//! Main indexer job implementation

use crate::{
	domain::addressing::SdPath,
	infra::db::entities,
	infra::job::{prelude::*, traits::DynJob},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use super::{
	entry::EntryMetadata,
	metrics::{IndexerMetrics, PhaseTimer},
	phases,
	state::{IndexError, IndexPhase, IndexerProgress, IndexerState, IndexerStats, Phase},
	PathResolver,
};

/// Indexing mode determines the depth of indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub enum IndexMode {
	/// Location exists but is not indexed
	None,
	/// Just filesystem metadata (fastest)
	Shallow,
	/// Generate content identities (moderate)
	Content,
	/// Full indexing with thumbnails and text extraction (slowest)
	Deep,
}

/// Indexing scope determines how much of the directory tree to process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum IndexScope {
	/// Index only the current directory (single level)
	Current,
	/// Index recursively through all subdirectories
	Recursive,
}

impl Default for IndexScope {
	fn default() -> Self {
		IndexScope::Recursive
	}
}

impl From<&str> for IndexScope {
	fn from(s: &str) -> Self {
		match s.to_lowercase().as_str() {
			"current" => IndexScope::Current,
			"recursive" => IndexScope::Recursive,
			_ => IndexScope::Recursive,
		}
	}
}

impl std::fmt::Display for IndexScope {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IndexScope::Current => write!(f, "current"),
			IndexScope::Recursive => write!(f, "recursive"),
		}
	}
}

/// Determines whether indexing results are persisted to database or kept in memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum IndexPersistence {
	/// Write all results to database (normal operation)
	Persistent,
	/// Keep results in memory only (for unmanaged paths)
	Ephemeral,
}

impl Default for IndexPersistence {
	fn default() -> Self {
		IndexPersistence::Persistent
	}
}

/// Enhanced configuration for indexer jobs
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerJobConfig {
	pub location_id: Option<Uuid>, // None for ephemeral indexing
	pub path: SdPath,
	pub mode: IndexMode,
	pub scope: IndexScope,
	pub persistence: IndexPersistence,
	pub max_depth: Option<u32>, // Override for Current scope or depth limiting
	#[serde(default)]
	pub rule_toggles: super::rules::RuleToggles,
}

impl IndexerJobConfig {
	/// Create a new configuration for persistent recursive indexing (traditional)
	pub fn new(location_id: Uuid, path: SdPath, mode: IndexMode) -> Self {
		Self {
			location_id: Some(location_id),
			path,
			mode,
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		}
	}

	/// Create configuration for UI directory navigation (quick current scan)
	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self {
			location_id: Some(location_id),
			path,
			mode: IndexMode::Shallow,
			scope: IndexScope::Current,
			persistence: IndexPersistence::Persistent,
			max_depth: Some(1),
			rule_toggles: Default::default(),
		}
	}

	/// Create configuration for ephemeral path browsing (outside managed locations)
	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self {
			location_id: None,
			path,
			mode: IndexMode::Shallow, // Ephemeral jobs identify content kind by extension, no hashing needed
			scope,
			persistence: IndexPersistence::Ephemeral,
			max_depth: if scope == IndexScope::Current {
				Some(1)
			} else {
				None
			},
			rule_toggles: Default::default(),
		}
	}

	/// Check if this is an ephemeral (non-persistent) job
	pub fn is_ephemeral(&self) -> bool {
		self.persistence == IndexPersistence::Ephemeral
	}

	/// Check if this is a current scope (single level) job
	pub fn is_current_scope(&self) -> bool {
		self.scope == IndexScope::Current
	}
}

/// In-memory storage for ephemeral indexing results
///
/// This implementation uses efficient data structures for memory optimization:
/// - NodeArena: Contiguous storage for file nodes (~48 bytes per node)
/// - NameCache: String interning for common filenames (shared across all entries)
/// - NameRegistry: Fast name-based lookups
///
/// All browsed paths share a single index, maximizing string deduplication
/// and memory efficiency. Parent-child relationships are established based
/// on path hierarchy.
///
/// Memory usage: ~50 bytes per entry vs ~200 bytes with HashMap
pub struct EphemeralIndex {
	/// Efficient tree storage
	arena: super::ephemeral::NodeArena,

	/// String interning (shared across all paths)
	cache: std::sync::Arc<super::ephemeral::NameCache>,

	/// Fast name lookups
	registry: super::ephemeral::NameRegistry,

	/// Path → EntryId mapping (for lookups by path)
	path_index: HashMap<PathBuf, super::ephemeral::EntryId>,

	/// UUID mapping (for API compatibility)
	entry_uuids: HashMap<PathBuf, Uuid>,

	/// Content kinds by path (fast extension-based identification)
	content_kinds: HashMap<PathBuf, crate::domain::ContentKind>,

	/// Metadata
	created_at: std::time::Instant,
	last_accessed: std::time::Instant,
	pub stats: IndexerStats,
}

impl std::fmt::Debug for EphemeralIndex {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("EphemeralIndex")
			.field("entry_count", &self.arena.len())
			.field("interned_names", &self.cache.len())
			.field("path_count", &self.path_index.len())
			.finish()
	}
}

impl EphemeralIndex {
	/// Create a new empty ephemeral index
	///
	/// The index stores entries with their full paths and builds parent-child
	/// relationships based on path hierarchy. Multiple directory trees can
	/// coexist in the same index, sharing the arena and string interning pool.
	pub fn new() -> Self {
		use super::ephemeral::{NameCache, NameRegistry, NodeArena};

		let cache = std::sync::Arc::new(NameCache::new());
		let arena = NodeArena::new();
		let registry = NameRegistry::new();

		let now = std::time::Instant::now();

		Self {
			arena,
			cache,
			registry,
			path_index: HashMap::new(),
			entry_uuids: HashMap::new(),
			content_kinds: HashMap::new(),
			created_at: now,
			last_accessed: now,
			stats: IndexerStats::default(),
		}
	}

	/// Ensure a directory exists in the index, creating ancestor chain if needed
	///
	/// Returns the EntryId of the directory.
	pub fn ensure_directory(&mut self, path: &Path) -> super::ephemeral::EntryId {
		use super::ephemeral::{
			FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata,
		};
		use super::state::EntryKind;

		// Already exists?
		if let Some(&id) = self.path_index.get(path) {
			return id;
		}

		// Ensure parent exists first (recursive)
		let parent_id = if let Some(parent_path) = path.parent() {
			if parent_path.as_os_str().is_empty() {
				None
			} else {
				Some(self.ensure_directory(parent_path))
			}
		} else {
			None
		};

		// Create this directory
		let name = self.cache.intern(
			path.file_name()
				.map(|s| s.to_string_lossy())
				.as_deref()
				.unwrap_or("/"),
		);

		let parent_ref = parent_id
			.map(MaybeEntryId::some)
			.unwrap_or(MaybeEntryId::NONE);
		let meta = PackedMetadata::new(NodeState::Accessible, FileType::Directory, 0);
		let node = FileNode::new(NameRef::new(name, parent_ref), meta);

		let id = self.arena.insert(node);

		// Add to parent's children
		if let Some(parent_id) = parent_id {
			if let Some(parent) = self.arena.get_mut(parent_id) {
				parent.add_child(id);
			}
		}

		// Index by path and name
		self.path_index.insert(path.to_path_buf(), id);
		self.registry.insert(name, id);

		// Generate UUID for directory
		let uuid = uuid::Uuid::new_v4();
		self.entry_uuids.insert(path.to_path_buf(), uuid);

		id
	}

	/// Add an entry to the index. Returns Some(content_kind) if added, None if duplicate.
	pub fn add_entry(
		&mut self,
		path: PathBuf,
		uuid: Uuid,
		metadata: EntryMetadata,
	) -> Option<crate::domain::ContentKind> {
		use super::ephemeral::{
			FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata,
		};
		use crate::domain::ContentKind;
		use crate::filetype::FileTypeRegistry;

		// Check if entry already exists for this path - skip if so to prevent duplicates
		if self.path_index.contains_key(&path) {
			tracing::trace!("Skipping duplicate entry: {}", path.display());
			return None;
		}

		// Ensure parent directory exists in the index FIRST (requires &mut self)
		// This must happen before interning the name to avoid borrow conflicts
		let parent_id = if let Some(parent_path) = path.parent() {
			if parent_path.as_os_str().is_empty() {
				// Root of filesystem, no parent
				None
			} else if let Some(&existing_id) = self.path_index.get(parent_path) {
				// Parent already exists
				Some(existing_id)
			} else {
				// Parent doesn't exist - ensure it (and ancestors) are created
				Some(self.ensure_directory(parent_path))
			}
		} else {
			None
		};

		// Now intern the filename (borrows self.cache immutably)
		let name = self.cache.intern(
			path.file_name()
				.map(|s| s.to_string_lossy())
				.as_deref()
				.unwrap_or("unknown"),
		);

		// Create metadata
		let file_type = FileType::from(metadata.kind);

		let meta = PackedMetadata::new(NodeState::Accessible, file_type, metadata.size)
			.with_times(metadata.modified, metadata.created);

		// Create node
		let parent_ref = parent_id
			.map(MaybeEntryId::some)
			.unwrap_or(MaybeEntryId::NONE);
		let node = FileNode::new(NameRef::new(name, parent_ref), meta);

		let id = self.arena.insert(node);

		// Add to parent's children
		if let Some(parent_id) = parent_id {
			if let Some(parent) = self.arena.get_mut(parent_id) {
				parent.add_child(id);
			}
		}

		// Detect content kind by extension (fast, no I/O)
		let content_kind = if metadata.kind == super::state::EntryKind::File {
			let registry = FileTypeRegistry::default();
			registry.identify_by_extension(&path)
		} else if metadata.kind == super::state::EntryKind::Directory {
			ContentKind::Unknown // Directories don't have content kind
		} else {
			ContentKind::Unknown
		};

		// Index by path and name
		self.path_index.insert(path.clone(), id);
		self.registry.insert(name, id);
		self.entry_uuids.insert(path.clone(), uuid);
		self.content_kinds.insert(path, content_kind);

		self.last_accessed = std::time::Instant::now();
		Some(content_kind)
	}

	pub fn get_entry(&mut self, path: &PathBuf) -> Option<EntryMetadata> {
		use super::state::EntryKind;

		let id = self.path_index.get(path)?;
		let node = self.arena.get(*id)?;

		self.last_accessed = std::time::Instant::now();

		Some(EntryMetadata {
			path: path.clone(),
			kind: EntryKind::from(node.meta.file_type()),
			size: node.meta.size(),
			modified: node.meta.mtime_as_system_time(),
			accessed: None,
			created: node.meta.ctime_as_system_time(),
			inode: None,
			permissions: None,
			is_hidden: path
				.file_name()
				.and_then(|n| n.to_str())
				.map(|n| n.starts_with('.'))
				.unwrap_or(false),
		})
	}

	/// Get entry reference for read-only access (doesn't update last_accessed)
	pub fn get_entry_ref(&self, path: &PathBuf) -> Option<EntryMetadata> {
		use super::state::EntryKind;

		let id = self.path_index.get(path)?;
		let node = self.arena.get(*id)?;

		Some(EntryMetadata {
			path: path.clone(),
			kind: EntryKind::from(node.meta.file_type()),
			size: node.meta.size(),
			modified: node.meta.mtime_as_system_time(),
			accessed: None,
			created: node.meta.ctime_as_system_time(),
			inode: None,
			permissions: None,
			is_hidden: path
				.file_name()
				.and_then(|n| n.to_str())
				.map(|n| n.starts_with('.'))
				.unwrap_or(false),
		})
	}

	pub fn get_entry_uuid(&self, path: &PathBuf) -> Option<Uuid> {
		self.entry_uuids.get(path).copied()
	}

	/// Get the content kind for an entry (identified by extension)
	pub fn get_content_kind(&self, path: &PathBuf) -> crate::domain::ContentKind {
		self.content_kinds
			.get(path)
			.copied()
			.unwrap_or(crate::domain::ContentKind::Unknown)
	}

	/// List directory children
	pub fn list_directory(&self, path: &std::path::Path) -> Option<Vec<PathBuf>> {
		let id = self.path_index.get(path)?;
		let node = self.arena.get(*id)?;

		Some(
			node.children
				.iter()
				.filter_map(|&child_id| self.reconstruct_path(child_id))
				.collect(),
		)
	}

	/// Clear all direct children of a directory (for re-indexing)
	///
	/// This removes entries for the immediate children of the given directory,
	/// preventing ghost entries when files are deleted between index runs.
	/// Note: Does not recursively clear subdirectories.
	pub fn clear_directory_children(&mut self, dir_path: &Path) -> usize {
		// Get the directory's children paths first
		let children_paths: Vec<PathBuf> = if let Some(dir_id) = self.path_index.get(dir_path) {
			if let Some(dir_node) = self.arena.get(*dir_id) {
				dir_node
					.children
					.iter()
					.filter_map(|&child_id| self.reconstruct_path(child_id))
					.collect()
			} else {
				return 0;
			}
		} else {
			return 0;
		};

		let mut cleared = 0;

		// Remove each child from indexes (arena nodes are left as orphans - acceptable for ephemeral)
		for child_path in &children_paths {
			if self.path_index.remove(child_path).is_some() {
				cleared += 1;
			}
			self.entry_uuids.remove(child_path);
			self.content_kinds.remove(child_path);
		}

		// Clear the parent's children list
		if let Some(dir_id) = self.path_index.get(dir_path) {
			if let Some(dir_node) = self.arena.get_mut(*dir_id) {
				dir_node.children.clear();
			}
		}

		cleared
	}

	/// Reconstruct full path for a node
	fn reconstruct_path(&self, id: super::ephemeral::EntryId) -> Option<PathBuf> {
		let mut segments = Vec::new();
		let mut current = id;

		// Walk up the tree collecting path segments
		while let Some(node) = self.arena.get(current) {
			segments.push(node.name().to_owned());
			if let Some(parent) = node.parent() {
				current = parent;
			} else {
				// Reached a root node (no parent)
				break;
			}
		}

		if segments.is_empty() {
			return None;
		}

		// Build absolute path from segments (root to leaf)
		let mut path = PathBuf::from("/");
		for segment in segments.into_iter().rev() {
			path.push(segment);
		}
		Some(path)
	}

	/// Find all entries with the given filename
	pub fn find_by_name(&self, name: &str) -> Vec<PathBuf> {
		self.registry
			.get(name)
			.map(|ids| {
				ids.iter()
					.filter_map(|&id| self.reconstruct_path(id))
					.collect()
			})
			.unwrap_or_default()
	}

	/// Find all entries with names starting with the given prefix
	pub fn find_by_prefix(&self, prefix: &str) -> Vec<PathBuf> {
		self.registry
			.find_prefix(prefix)
			.iter()
			.filter_map(|&id| self.reconstruct_path(id))
			.collect()
	}

	pub fn age(&self) -> Duration {
		self.created_at.elapsed()
	}

	pub fn idle_time(&self) -> Duration {
		self.last_accessed.elapsed()
	}

	/// Get the total number of entries
	pub fn len(&self) -> usize {
		self.arena.len()
	}

	/// Check if the index is empty
	pub fn is_empty(&self) -> bool {
		self.arena.is_empty()
	}

	/// Get approximate memory usage in bytes
	pub fn memory_usage(&self) -> usize {
		self.arena.memory_usage()
			+ self.cache.memory_usage()
			+ self.registry.memory_usage()
			+ self.path_index.capacity()
				* (std::mem::size_of::<PathBuf>()
					+ std::mem::size_of::<super::ephemeral::EntryId>())
			+ self.entry_uuids.capacity()
				* (std::mem::size_of::<PathBuf>() + std::mem::size_of::<Uuid>())
	}

	/// Get statistics about the index
	pub fn get_stats(&self) -> EphemeralIndexStats {
		EphemeralIndexStats {
			total_entries: self.arena.len(),
			unique_names: self.registry.unique_names(),
			interned_strings: self.cache.len(),
			memory_bytes: self.memory_usage(),
		}
	}

	/// Get the number of content kinds stored
	pub fn content_kinds_count(&self) -> usize {
		self.content_kinds.len()
	}

	/// Get the number of entries in the path index
	pub fn path_index_count(&self) -> usize {
		self.path_index.len()
	}

	/// Get all entries as a HashMap (for backward compatibility)
	///
	/// This method reconstructs paths for all entries. For large indexes,
	/// consider using iterators or specific queries instead.
	pub fn entries(&self) -> HashMap<PathBuf, EntryMetadata> {
		use super::state::EntryKind;

		let mut result = HashMap::with_capacity(self.path_index.len());

		for (path, &id) in &self.path_index {
			if let Some(node) = self.arena.get(id) {
				let metadata = EntryMetadata {
					path: path.clone(),
					kind: EntryKind::from(node.meta.file_type()),
					size: node.meta.size(),
					modified: node.meta.mtime_as_system_time(),
					accessed: None,
					created: node.meta.ctime_as_system_time(),
					inode: None,
					permissions: None,
					is_hidden: path
						.file_name()
						.and_then(|n| n.to_str())
						.map(|n| n.starts_with('.'))
						.unwrap_or(false),
				};
				result.insert(path.clone(), metadata);
			}
		}

		result
	}
}

impl Default for EphemeralIndex {
	fn default() -> Self {
		Self::new()
	}
}

/// Statistics about an ephemeral index
#[derive(Debug, Clone)]
pub struct EphemeralIndexStats {
	pub total_entries: usize,
	pub unique_names: usize,
	pub interned_strings: usize,
	pub memory_bytes: usize,
}

/// Indexer job - discovers and indexes files in a location
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IndexerJob {
	pub config: IndexerJobConfig,

	// Resumable state
	state: Option<IndexerState>,

	// Ephemeral storage for non-persistent jobs
	#[serde(skip)]
	ephemeral_index: Option<Arc<RwLock<EphemeralIndex>>>,

	// Performance tracking
	#[serde(skip)]
	timer: Option<PhaseTimer>,
	#[serde(skip)]
	db_operations: (u64, u64), // (reads, writes)
	#[serde(skip)]
	batch_info: (u64, usize), // (count, total_size)
}

impl Job for IndexerJob {
	const NAME: &'static str = "indexer";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Index files in a location");
}

impl DynJob for IndexerJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

impl JobProgress for IndexerProgress {}

#[async_trait::async_trait]
impl JobHandler for IndexerJob {
	type Output = IndexerOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// Initialize timer
		if self.timer.is_none() {
			self.timer = Some(PhaseTimer::new());
		}

		// Initialize ephemeral index if needed
		if self.config.is_ephemeral() && self.ephemeral_index.is_none() {
			self.ephemeral_index = Some(Arc::new(RwLock::new(EphemeralIndex::new())));
			ctx.log("Initialized ephemeral index for non-persistent job");
		}

		// Initialize or restore state
		// Ensure state is always created early to avoid serialization issues
		if self.state.is_none() {
			ctx.log(format!(
				"Starting new indexer job (scope: {}, persistence: {:?})",
				self.config.scope, self.config.persistence
			));
			info!("INDEXER_STATE: Job starting with NO saved state - creating new state");
			self.state = Some(IndexerState::new(&self.config.path));
		} else {
			ctx.log("Resuming indexer from saved state");
			let state = self.state.as_ref().unwrap();
			info!("INDEXER_STATE: Job resuming with saved state - phase: {:?}, entry_batches: {}, entries_for_content: {}, seen_paths: {}",
				state.phase,
				state.entry_batches.len(),
				state.entries_for_content.len(),
				state.seen_paths.len());
			warn!(
				"DEBUG: Resumed state - phase: {:?}, entry_batches: {}, entries_for_content: {}",
				state.phase,
				state.entry_batches.len(),
				state.entries_for_content.len()
			);
		}

		let state = self.state.as_mut().unwrap();

		// Get root path ONCE for the entire job
		// For cloud volumes, we use the path component from the SdPath (e.g., "/" or "folder/")
		// since discovery operates through the volume backend (not direct filesystem access)
		let root_path_buf = if let Some(p) = self.config.path.as_local_path() {
			p.to_path_buf()
		} else if let Some(cloud_path) = self.config.path.cloud_path() {
			// Cloud path - use the path component within the cloud volume
			// The actual I/O will go through the volume backend
			PathBuf::from(cloud_path)
		} else if !self.config.is_ephemeral() {
			let loc_uuid = self
				.config
				.location_id
				.ok_or_else(|| JobError::execution("Missing location id".to_string()))?;
			let db = ctx.library().db();
			let location = entities::location::Entity::find()
				.filter(entities::location::Column::Uuid.eq(loc_uuid))
				.one(db.conn())
				.await
				.map_err(|e| JobError::execution(e.to_string()))?
				.ok_or_else(|| JobError::execution("Location not found".to_string()))?;
			let entry_id = location
				.entry_id
				.ok_or_else(|| JobError::execution("Location has no entry_id".to_string()))?;
			let path_str = PathResolver::get_directory_path(db.conn(), entry_id)
				.await
				.map_err(|e| JobError::execution(e.to_string()))?;
			std::path::PathBuf::from(path_str)
		} else {
			return Err(JobError::execution(
				"Location root path is not local".to_string(),
			));
		};
		let root_path = root_path_buf.as_path();

		// Get volume backend for the entire job
		let volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>> =
			if let Some(vm) = ctx.volume_manager() {
				match vm
					.resolve_volume_for_sdpath(&self.config.path, ctx.library())
					.await
				{
					Ok(Some(mut volume)) => {
						ctx.log(format!(
							"Using volume backend: {} for path: {}",
							volume.name, self.config.path
						));
						Some(vm.backend_for_volume(&mut volume))
					}
					Ok(None) => {
						// For cloud paths, we MUST have a volume - can't fall back to local
						if self.config.path.is_cloud() {
							ctx.log(format!(
								"Cloud volume not found for path: {}",
								self.config.path
							));
							return Err(JobError::execution(format!(
								"Cloud volume not found for path: {}. The cloud volume may not be registered yet.",
								self.config.path
							)));
						}

						// For local paths, we can fall back to LocalBackend
						ctx.log(format!(
							"No volume found for path: {}, will use LocalBackend fallback",
							self.config.path
						));
						None
					}
					Err(e) => {
						ctx.log(format!("Failed to resolve volume: {}", e));
						return Err(JobError::execution(format!(
							"Failed to resolve volume: {}",
							e
						)));
					}
				}
			} else {
				ctx.log("No volume manager available, will use LocalBackend fallback");
				None
			};

		// Seed discovery queue if it wasn't initialized due to device-id timing
		if state.dirs_to_walk.is_empty() {
			state.dirs_to_walk.push_back(root_path.to_path_buf());
		}

		// Main state machine loop
		loop {
			ctx.check_interrupt().await?;

			let current_phase = state.phase.clone();
			warn!("DEBUG: IndexerJob entering phase: {:?}", current_phase);
			match current_phase {
				Phase::Discovery => {
					// For cloud volumes, construct the base URL for building absolute paths
					let cloud_url_base =
						if let Some((service, identifier, _)) = self.config.path.as_cloud() {
							Some(format!("{}://{}/", service.scheme(), identifier))
						} else {
							None
						};

					// Use scope-aware discovery
					if self.config.is_current_scope() {
						Self::run_current_scope_discovery_static(state, &ctx, root_path).await?;
					} else {
						phases::run_discovery_phase(
							state,
							&ctx,
							root_path,
							self.config.rule_toggles.clone(),
							volume_backend.as_ref(),
							cloud_url_base,
						)
						.await?;
					}

					// Track batch info
					self.batch_info.0 = state.entry_batches.len() as u64;
					self.batch_info.1 = state.entry_batches.iter().map(|b| b.len()).sum();

					// Start processing timer
					if let Some(timer) = &mut self.timer {
						timer.start_processing();
					}
				}

				Phase::Processing => {
					warn!("DEBUG: IndexerJob starting Processing phase");
					if self.config.is_ephemeral() {
						let ephemeral_index = self.ephemeral_index.clone().ok_or_else(|| {
							JobError::execution("Ephemeral index not initialized".to_string())
						})?;
						Self::run_ephemeral_processing_static(
							state,
							&ctx,
							ephemeral_index,
							root_path,
							volume_backend.as_ref(),
						)
						.await?;
					} else {
						phases::run_processing_phase(
							self.config
								.location_id
								.expect("Location ID required for persistent jobs"),
							state,
							&ctx,
							self.config.mode,
							root_path,
							volume_backend.as_ref(),
						)
						.await?;

						// Update DB operation counts
						self.db_operations.1 += state.entry_batches.len() as u64 * 100; // Estimate
					}
				}

				Phase::Aggregation => {
					if !self.config.is_ephemeral() {
						phases::run_aggregation_phase(
							self.config
								.location_id
								.expect("Location ID required for persistent jobs"),
							state,
							&ctx,
						)
						.await?;
					} else {
						// Skip aggregation and content phases for ephemeral jobs
						// Content kind is already identified by extension during add_entry
						ctx.log("Skipping aggregation and content phases for ephemeral job (content kind identified by extension)");
						state.phase = Phase::Complete;
						continue;
					}

					// Start content timer
					if let Some(timer) = &mut self.timer {
						timer.start_content();
					}
				}

				Phase::ContentIdentification => {
					if self.config.mode >= IndexMode::Content {
						if self.config.is_ephemeral() {
							// Skip content phase for ephemeral jobs - content kind already identified
							ctx.log("Skipping content identification for ephemeral job");
							state.phase = Phase::Complete;
							continue;
						} else {
							let library_id = ctx.library().id();
							phases::run_content_phase(
								state,
								&ctx,
								library_id,
								volume_backend.as_ref(),
							)
							.await?;
							self.db_operations.1 += state.entries_for_content.len() as u64;
						}
					} else {
						ctx.log("Skipping content identification phase (mode=Shallow)");
						state.phase = Phase::Complete;
					}
				}

				Phase::Complete => break,
			}

			// State is automatically saved during job serialization on shutdown
			warn!(
				"DEBUG: IndexerJob completed phase: {:?}, next phase will be: {:?}",
				current_phase, state.phase
			);
		}

		// Send final progress update
		let final_progress = IndexerProgress {
			phase: IndexPhase::Finalizing {
				processed: 0,
				total: 0,
			},
			current_path: "Completed".to_string(),
			total_found: state.stats,
			processing_rate: 0.0,
			estimated_remaining: None,
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None, // TODO: Pass action context from job state
		};
		ctx.progress(Progress::generic(final_progress.to_generic_progress()));

		// Calculate final metrics
		let metrics = if let Some(timer) = &self.timer {
			IndexerMetrics::calculate(&state.stats, timer, self.db_operations, self.batch_info)
		} else {
			IndexerMetrics::default()
		};

		// Log summary
		ctx.log(&metrics.format_summary());

		// If Deep mode, dispatch thumbnail generation job after indexing completes
		if self.config.mode == IndexMode::Deep && !self.config.is_ephemeral() {
			use crate::ops::media::thumbnail::{ThumbnailJob, ThumbnailJobConfig};

			ctx.log("Deep mode enabled - dispatching thumbnail generation job");

			// Dispatch thumbnail job for all entries in this location
			let thumbnail_config = ThumbnailJobConfig::default();
			let thumbnail_job = ThumbnailJob::new(thumbnail_config);

			match ctx.library().jobs().dispatch(thumbnail_job).await {
				Ok(_handle) => {
					ctx.log("Successfully dispatched thumbnail generation job");
				}
				Err(e) => {
					ctx.log(format!("Warning: Failed to dispatch thumbnail job: {}", e));
					// Don't fail the indexing job if thumbnail dispatch fails
				}
			}
		}

		// Mark ephemeral indexing as complete in the cache
		if self.config.is_ephemeral() {
			if let Some(local_path) = self.config.path.as_local_path() {
				ctx.library()
					.core_context()
					.ephemeral_cache()
					.mark_indexing_complete(local_path);
				ctx.log(format!(
					"Marked ephemeral indexing complete for: {}",
					local_path.display()
				));
			}
		}

		// Generate final output
		Ok(IndexerOutput {
			location_id: self.config.location_id,
			stats: state.stats,
			duration: state.started_at.elapsed(),
			errors: state.errors.clone(),
			metrics: Some(metrics),
			ephemeral_results: if self.config.is_ephemeral() {
				self.ephemeral_index.clone()
			} else {
				None
			},
		})
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
		// State is already loaded from serialization
		warn!("DEBUG: IndexerJob on_resume called");
		if let Some(state) = &self.state {
			warn!(
				"DEBUG: IndexerJob has state, resuming in {:?} phase",
				state.phase
			);
			ctx.log(format!("Resuming indexer in {:?} phase", state.phase));
			ctx.log(format!(
				"Progress: {} files, {} dirs, {} errors so far",
				state.stats.files, state.stats.dirs, state.stats.errors
			));

			// Reinitialize timer for resumed job
			self.timer = Some(PhaseTimer::new());
		} else {
			warn!("DEBUG: IndexerJob has no state during resume - creating new state!");
			// If state is missing, create it now (this shouldn't happen in normal operation)
			self.state = Some(IndexerState::new(&self.config.path));
		}
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing indexer job - state will be preserved");
		Ok(())
	}

	async fn on_cancel(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Cancelling indexer job");
		if let Some(state) = &self.state {
			ctx.log(format!(
				"Final stats: {} files, {} dirs indexed before cancellation",
				state.stats.files, state.stats.dirs
			));
		}
		Ok(())
	}

	fn is_resuming(&self) -> bool {
		// If we have existing state, we're resuming
		self.state.is_some()
	}
}

impl IndexerJob {
	/// Create a new indexer job with enhanced configuration
	pub fn new(config: IndexerJobConfig) -> Self {
		Self {
			config,
			state: None,
			ephemeral_index: None,
			timer: None,
			db_operations: (0, 0),
			batch_info: (0, 0),
		}
	}

	/// Create a traditional persistent recursive indexer job
	pub fn from_location(location_id: Uuid, root_path: SdPath, mode: IndexMode) -> Self {
		Self::new(IndexerJobConfig::new(location_id, root_path, mode))
	}

	/// Create a shallow indexer job (metadata only)
	pub fn shallow(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Shallow)
	}

	/// Create a content indexer job (with CAS IDs)
	pub fn with_content(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Content)
	}

	/// Create a deep indexer job (full processing)
	pub fn deep(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Deep)
	}

	/// Create a UI navigation job (current scope, quick scan)
	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self::new(IndexerJobConfig::ui_navigation(location_id, path))
	}

	/// Set the ephemeral index storage (must be called before dispatching for ephemeral jobs)
	/// This allows external code to maintain a reference to the same storage the job uses
	pub fn set_ephemeral_index(&mut self, index: Arc<RwLock<EphemeralIndex>>) {
		self.ephemeral_index = Some(index);
	}

	/// Create an ephemeral browsing job (no database writes)
	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self::new(IndexerJobConfig::ephemeral_browse(path, scope))
	}

	/// Run current scope discovery (single level only)
	async fn run_current_scope_discovery_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		root_path: &std::path::Path,
	) -> JobResult<()> {
		use super::entry::EntryProcessor;
		use super::state::{DirEntry, EntryKind};
		use tokio::fs;

		ctx.log("Starting current scope discovery (single level)");

		let mut entries = fs::read_dir(root_path)
			.await
			.map_err(|e| JobError::execution(format!("Failed to read directory: {}", e)))?;

		while let Some(entry) = entries
			.next_entry()
			.await
			.map_err(|e| JobError::execution(format!("Failed to read directory entry: {}", e)))?
		{
			let path = entry.path();
			let metadata = entry
				.metadata()
				.await
				.map_err(|e| JobError::execution(format!("Failed to read metadata: {}", e)))?;

			let entry_kind = if metadata.is_dir() {
				EntryKind::Directory
			} else if metadata.is_symlink() {
				EntryKind::Symlink
			} else {
				EntryKind::File
			};

			let dir_entry = DirEntry {
				path: path.clone(),
				kind: entry_kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				inode: EntryProcessor::get_inode(&metadata),
			};

			state.pending_entries.push(dir_entry);
			state.items_since_last_update += 1;

			// Update stats
			match entry_kind {
				EntryKind::File => state.stats.files += 1,
				EntryKind::Directory => state.stats.dirs += 1,
				EntryKind::Symlink => state.stats.symlinks += 1,
			}
		}

		// Create single batch and move to processing
		if !state.pending_entries.is_empty() {
			let batch = state.create_batch();
			state.entry_batches.push(batch);
		}

		state.phase = Phase::Processing;
		ctx.log(format!(
			"Current scope discovery complete: {} entries found",
			state.stats.files + state.stats.dirs
		));

		Ok(())
	}

	/// Run ephemeral processing (store in memory instead of database)
	async fn run_ephemeral_processing_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		ephemeral_index: Arc<RwLock<EphemeralIndex>>,
		root_path: &Path,
		_volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	) -> JobResult<()> {
		use super::persistence::PersistenceFactory;

		ctx.log("Starting ephemeral processing");

		// Get event bus from library
		let event_bus = Some(ctx.library().event_bus().clone());

		// Create ephemeral persistence layer (emits events as entries are stored)
		let persistence = PersistenceFactory::ephemeral(
			ephemeral_index.clone(),
			event_bus,
			root_path.to_path_buf(),
		);

		// Process all batches through persistence layer
		while let Some(batch) = state.entry_batches.pop() {
			for entry in batch {
				// Store entry (this will emit ResourceChanged events)
				// Content kind is identified by extension during add_entry, no hashing needed
				let _entry_id = persistence.store_entry(&entry, None, root_path).await?;
			}
		}

		// Skip content identification for ephemeral jobs - go directly to complete
		state.phase = Phase::Complete;

		ctx.log("Ephemeral processing complete");
		Ok(())
	}
}

/// Job output with comprehensive results
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerOutput {
	pub location_id: Option<Uuid>,
	pub stats: IndexerStats,
	pub duration: Duration,
	pub errors: Vec<IndexError>,
	pub metrics: Option<IndexerMetrics>,
	#[serde(skip)]
	pub ephemeral_results: Option<Arc<RwLock<EphemeralIndex>>>,
}

impl From<IndexerOutput> for JobOutput {
	fn from(output: IndexerOutput) -> Self {
		JobOutput::Indexed {
			stats: output.stats,
			metrics: output.metrics.unwrap_or_default(),
		}
	}
}
