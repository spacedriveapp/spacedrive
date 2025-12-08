//! Indexer job implementation and ephemeral index storage.
//!
//! This module contains the main `IndexerJob` struct that orchestrates the multi-phase
//! indexing pipeline, as well as the `EphemeralIndex` used for browsing unmanaged paths
//! without database writes. The job supports both persistent indexing (for managed locations)
//! and ephemeral indexing (for external drives, network shares, and temporary browsing).

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

/// How deeply to index files, from metadata-only to full processing.
///
/// IndexMode controls the trade-off between indexing speed and feature completeness.
/// Shallow mode is fast enough for ephemeral browsing, while Deep mode enables
/// duplicate detection, thumbnail generation, and full-text search at the cost of
/// significantly longer indexing time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub enum IndexMode {
	/// Location exists but is not indexed
	None,
	/// Just filesystem metadata (fastest)
	Shallow,
	/// Generate content identities via BLAKE3 hashing (enables duplicate detection)
	Content,
	/// Full indexing with thumbnails and text extraction (slowest)
	Deep,
}

/// Whether to index just one directory level or recurse through subdirectories.
///
/// Current scope is used for UI navigation where users expand folders on-demand,
/// while Recursive scope is used for full location indexing. Current scope with
/// persistent storage enables progressive indexing where the UI drives which
/// directories get indexed based on user interaction.
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

/// Whether to write indexing results to the database or keep them in memory.
///
/// Ephemeral persistence allows users to browse external drives and network shares
/// without adding them as managed locations. The in-memory index survives for the
/// session duration and provides the same API surface as persistent entries, enabling
/// features like search and navigation to work identically for both modes. If an
/// ephemeral path is later promoted to a managed location, UUIDs are preserved to
/// maintain continuity for user metadata.
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

/// Configuration for an indexer job, supporting both persistent and ephemeral indexing.
///
/// Persistent jobs require a location_id to identify which managed location they're
/// indexing. Ephemeral jobs (browsing unmanaged paths) use location_id = None and
/// store results in memory instead of the database.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerJobConfig {
	pub location_id: Option<Uuid>,
	pub path: SdPath,
	pub mode: IndexMode,
	pub scope: IndexScope,
	pub persistence: IndexPersistence,
	pub max_depth: Option<u32>,
	#[serde(default)]
	pub rule_toggles: super::rules::RuleToggles,
}

impl IndexerJobConfig {
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

	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self {
			location_id: None,
			path,
			mode: IndexMode::Shallow,
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

/// Memory-efficient index for browsing paths outside managed locations.
///
/// Ephemeral indexing lets users navigate unmanaged directories (network shares,
/// external drives) without adding them as permanent locations. Instead of writing
/// to the database, entries live in this memory-only structure until the session
/// ends or the path is promoted to a managed location.
///
/// Memory usage is ~50 bytes per entry vs ~200 bytes with a naive `HashMap<PathBuf, Entry>`
/// approach. The optimization comes from:
/// - **NodeArena:** Contiguous slab allocation with pointer-sized entry IDs
/// - **NameCache:** String interning (one copy of "index.js" for thousands of node_modules files)
/// - **NameRegistry:** Trie-based prefix search without full-text indexing overhead
///
/// Multiple directory trees can coexist in the same index (e.g., browsing both
/// `/mnt/nas` and `/media/usb` simultaneously), sharing the string interning pool
/// for maximum deduplication.
pub struct EphemeralIndex {
	arena: super::ephemeral::NodeArena,
	cache: std::sync::Arc<super::ephemeral::NameCache>,
	registry: super::ephemeral::NameRegistry,
	path_index: HashMap<PathBuf, super::ephemeral::EntryId>,
	entry_uuids: HashMap<PathBuf, Uuid>,
	content_kinds: HashMap<PathBuf, crate::domain::ContentKind>,
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
	pub fn new() -> std::io::Result<Self> {
		use super::ephemeral::{NameCache, NameRegistry, NodeArena};

		let cache = std::sync::Arc::new(NameCache::new());
		let arena = NodeArena::new()?;
		let registry = NameRegistry::new();

		let now = std::time::Instant::now();

		Ok(Self {
			arena,
			cache,
			registry,
			path_index: HashMap::new(),
			entry_uuids: HashMap::new(),
			content_kinds: HashMap::new(),
			created_at: now,
			last_accessed: now,
			stats: IndexerStats::default(),
		})
	}

	/// Ensures a directory exists, creating all missing ancestors recursively.
	///
	/// This method guarantees that `list_directory()` works immediately after
	/// `add_entry()` without a separate tree-building pass. Parent directories
	/// are created from root to leaf, so the full ancestor chain exists before
	/// any child is added.
	pub fn ensure_directory(&mut self, path: &Path) -> std::io::Result<super::ephemeral::EntryId> {
		use super::ephemeral::{
			FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata,
		};
		use super::state::EntryKind;

		if let Some(&id) = self.path_index.get(path) {
			return Ok(id);
		}

		let parent_id = if let Some(parent_path) = path.parent() {
			if parent_path.as_os_str().is_empty() {
				None
			} else {
				Some(self.ensure_directory(parent_path)?)
			}
		} else {
			None
		};

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

		let id = self.arena.insert(node)?;

		// Add to parent's children
		if let Some(parent_id) = parent_id {
			if let Some(parent) = self.arena.get_mut(parent_id) {
				parent.add_child(id);
			}
		}

		self.path_index.insert(path.to_path_buf(), id);
		self.registry.insert(name, id);

		let uuid = uuid::Uuid::new_v4();
		self.entry_uuids.insert(path.to_path_buf(), uuid);

		Ok(id)
	}

	/// Adds an entry to the index, returning its content kind if successful.
	///
	/// Content kind is identified by file extension (no I/O needed), which is
	/// sufficient for ephemeral browsing where speed is critical. Returns Ok(None)
	/// if the entry already exists (prevents duplicate entries when re-indexing
	/// a directory).
	pub fn add_entry(
		&mut self,
		path: PathBuf,
		uuid: Uuid,
		metadata: EntryMetadata,
	) -> std::io::Result<Option<crate::domain::ContentKind>> {
		use super::ephemeral::{
			FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata,
		};
		use crate::domain::ContentKind;
		use crate::filetype::FileTypeRegistry;

		if self.path_index.contains_key(&path) {
			tracing::trace!("Skipping duplicate entry: {}", path.display());
			return Ok(None);
		}

		// Ensure parent directories exist before adding this entry, building the ancestor
		// chain from root to leaf. The &mut borrow happens before name interning to avoid
		// holding the cache lock while recursing.
		let parent_id = if let Some(parent_path) = path.parent() {
			if parent_path.as_os_str().is_empty() {
				None
			} else if let Some(&existing_id) = self.path_index.get(parent_path) {
				Some(existing_id)
			} else {
				Some(self.ensure_directory(parent_path)?)
			}
		} else {
			None
		};

		let name = self.cache.intern(
			path.file_name()
				.map(|s| s.to_string_lossy())
				.as_deref()
				.unwrap_or("unknown"),
		);

		let file_type = FileType::from(metadata.kind);

		let meta = PackedMetadata::new(NodeState::Accessible, file_type, metadata.size)
			.with_times(metadata.modified, metadata.created);

		let parent_ref = parent_id
			.map(MaybeEntryId::some)
			.unwrap_or(MaybeEntryId::NONE);
		let node = FileNode::new(NameRef::new(name, parent_ref), meta);

		let id = self.arena.insert(node)?;

		// Add to parent's children
		if let Some(parent_id) = parent_id {
			if let Some(parent) = self.arena.get_mut(parent_id) {
				parent.add_child(id);
			}
		}

		let content_kind = if metadata.kind == super::state::EntryKind::File {
			let registry = FileTypeRegistry::default();
			registry.identify_by_extension(&path)
		} else if metadata.kind == super::state::EntryKind::Directory {
			ContentKind::Unknown
		} else {
			ContentKind::Unknown
		};

		self.path_index.insert(path.clone(), id);
		self.registry.insert(name, id);
		self.entry_uuids.insert(path.clone(), uuid);
		self.content_kinds.insert(path, content_kind);

		self.last_accessed = std::time::Instant::now();
		Ok(Some(content_kind))
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

	pub fn get_content_kind(&self, path: &PathBuf) -> crate::domain::ContentKind {
		self.content_kinds
			.get(path)
			.copied()
			.unwrap_or(crate::domain::ContentKind::Unknown)
	}

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

	/// Clears entries before re-indexing, preserving explicitly browsed subdirectories.
	///
	/// Since ephemeral indexing is shallow, subdirectories that were explicitly
	/// navigated to (in `indexed_paths`) should be preserved as separate index
	/// branches. Unbrowsed subdirectories are refreshed with the parent.
	///
	/// Returns (cleared_count, deleted_browsed_dirs) where deleted_browsed_dirs
	/// contains paths that were in indexed_paths but no longer exist on disk.
	pub fn clear_directory_children(
		&mut self,
		dir_path: &Path,
		indexed_paths: &std::collections::HashSet<std::path::PathBuf>,
	) -> (usize, Vec<std::path::PathBuf>) {
		let dir_id = match self.path_index.get(dir_path) {
			Some(&id) => id,
			None => return (0, Vec::new()),
		};

		let dir_node = match self.arena.get(dir_id) {
			Some(node) => node,
			None => return (0, Vec::new()),
		};

		let mut deleted_browsed_dirs = Vec::new();

		// Collect children to remove
		let mut children_to_remove: Vec<(PathBuf, super::ephemeral::EntryId)> = dir_node
			.children
			.iter()
			.filter_map(|&child_id| {
				let child_node = self.arena.get(child_id)?;
				let child_path = self.reconstruct_path(child_id)?;

				// Preserve subdirectories that were explicitly browsed AND still exist
				if child_node.is_directory() && indexed_paths.contains(&child_path) {
					// Verify the directory still exists on the filesystem
					if std::fs::metadata(&child_path).is_ok() {
						return None; // Preserve - still exists and was browsed
					}
					// Directory was deleted - track for removal from indexed_paths
					tracing::debug!(
						"Removing deleted browsed directory: {}",
						child_path.display()
					);
					deleted_browsed_dirs.push(child_path.clone());
				}

				// Remove everything else (files, unbrowsed directories, deleted directories)
				Some((child_path, child_id))
			})
			.collect();

		let cleared = children_to_remove.len();

		// Remove from indexes
		for (child_path, _) in &children_to_remove {
			self.path_index.remove(child_path);
			self.entry_uuids.remove(child_path);
			self.content_kinds.remove(child_path);
		}

		// Update parent's children list
		if let Some(dir_node) = self.arena.get_mut(dir_id) {
			let removed_ids: std::collections::HashSet<_> =
				children_to_remove.iter().map(|(_, id)| id).collect();

			dir_node
				.children
				.retain(|child_id| !removed_ids.contains(child_id));
		}

		if cleared > 0 {
			tracing::debug!(
				"Cleared {} entries from {} (preserved browsed subdirs)",
				cleared,
				dir_path.display()
			);
		}

		(cleared, deleted_browsed_dirs)
	}

	fn reconstruct_path(&self, id: super::ephemeral::EntryId) -> Option<PathBuf> {
		let mut segments = Vec::new();
		let mut current = id;

		while let Some(node) = self.arena.get(current) {
			segments.push(node.name().to_owned());
			if let Some(parent) = node.parent() {
				current = parent;
			} else {
				break;
			}
		}

		if segments.is_empty() {
			return None;
		}

		let mut path = PathBuf::from("/");
		for segment in segments.into_iter().rev() {
			path.push(segment);
		}
		Some(path)
	}

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

	pub fn len(&self) -> usize {
		self.arena.len()
	}

	pub fn is_empty(&self) -> bool {
		self.arena.is_empty()
	}

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

	pub fn get_stats(&self) -> EphemeralIndexStats {
		EphemeralIndexStats {
			total_entries: self.arena.len(),
			unique_names: self.registry.unique_names(),
			interned_strings: self.cache.len(),
			memory_bytes: self.memory_usage(),
		}
	}

	pub fn content_kinds_count(&self) -> usize {
		self.content_kinds.len()
	}

	pub fn path_index_count(&self) -> usize {
		self.path_index.len()
	}

	/// Reconstructs paths for all entries and returns them as a HashMap.
	///
	/// For large indexes, this can be expensive since it walks the tree to rebuild
	/// every path. Prefer using `list_directory()` or `find_by_name()` for targeted
	/// queries when possible.
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
		Self::new().expect("Failed to create default EphemeralIndex")
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

/// Orchestrates multi-phase file indexing for both persistent and ephemeral modes.
///
/// The job executes as a state machine progressing through Discovery, Processing,
/// Aggregation, and ContentIdentification phases. State is automatically serialized
/// between phases, allowing the job to survive app restarts and resume from the last
/// completed phase. Ephemeral jobs (browsing unmanaged paths) skip aggregation and
/// content identification, storing results in memory via `EphemeralIndex`.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IndexerJob {
	pub config: IndexerJobConfig,
	state: Option<IndexerState>,
	#[serde(skip)]
	ephemeral_index: Option<Arc<RwLock<EphemeralIndex>>>,
	#[serde(skip)]
	timer: Option<PhaseTimer>,
	#[serde(skip)]
	db_operations: (u64, u64),
	#[serde(skip)]
	batch_info: (u64, usize),
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

impl IndexerJob {
	async fn run_job_phases(&mut self, ctx: &JobContext<'_>) -> JobResult<IndexerOutput> {
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

		// For cloud volumes, we use the path component from the SdPath (e.g., "/" or "folder/")
		// since discovery operates through the volume backend (not direct filesystem access).
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

		if state.dirs_to_walk.is_empty() {
			state.dirs_to_walk.push_back(root_path.to_path_buf());
		}

		loop {
			ctx.check_interrupt().await?;

			let current_phase = state.phase.clone();
			warn!("DEBUG: IndexerJob entering phase: {:?}", current_phase);
			match current_phase {
				Phase::Discovery => {
					let cloud_url_base =
						if let Some((service, identifier, _)) = self.config.path.as_cloud() {
							Some(format!("{}://{}/", service.scheme(), identifier))
						} else {
							None
						};

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

					self.batch_info.0 = state.entry_batches.len() as u64;
					self.batch_info.1 = state.entry_batches.iter().map(|b| b.len()).sum();

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

						self.db_operations.1 += state.entry_batches.len() as u64 * 100;
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
						ctx.log("Skipping aggregation and content phases for ephemeral job (content kind identified by extension)");
						state.phase = Phase::Complete;
						continue;
					}

					if let Some(timer) = &mut self.timer {
						timer.start_content();
					}
				}

				Phase::ContentIdentification => {
					if self.config.mode >= IndexMode::Content {
						if self.config.is_ephemeral() {
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

			warn!(
				"DEBUG: IndexerJob completed phase: {:?}, next phase will be: {:?}",
				current_phase, state.phase
			);
		}

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
			action_context: None,
		};
		ctx.progress(Progress::generic(final_progress.to_generic_progress()));

		let metrics = if let Some(timer) = &self.timer {
			IndexerMetrics::calculate(&state.stats, timer, self.db_operations, self.batch_info)
		} else {
			IndexerMetrics::default()
		};

		ctx.log(&metrics.format_summary());

		if self.config.mode == IndexMode::Deep && !self.config.is_ephemeral() {
			use crate::ops::media::thumbnail::{ThumbnailJob, ThumbnailJobConfig};

			ctx.log("Deep mode enabled - dispatching thumbnail generation job");

			let thumbnail_config = ThumbnailJobConfig::default();
			let thumbnail_job = ThumbnailJob::new(thumbnail_config);

			match ctx.library().jobs().dispatch(thumbnail_job).await {
				Ok(_handle) => {
					ctx.log("Successfully dispatched thumbnail generation job");
				}
				Err(e) => {
					ctx.log(format!("Warning: Failed to dispatch thumbnail job: {}", e));
				}
			}
		}

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
}

// JobHandler trait implementation
#[async_trait::async_trait]
impl JobHandler for IndexerJob {
	type Output = IndexerOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if self.timer.is_none() {
			self.timer = Some(PhaseTimer::new());
		}

		if self.config.is_ephemeral() && self.ephemeral_index.is_none() {
			let index = EphemeralIndex::new()
				.map_err(|e| JobError::Other(format!("Failed to create ephemeral index: {}", e)))?;
			self.ephemeral_index = Some(Arc::new(RwLock::new(index)));
			ctx.log("Initialized ephemeral index for non-persistent job");
		}

		let result = self.run_job_phases(&ctx).await;

		// Mark ephemeral indexing complete even on failure to prevent the indexing
		// flag from being stuck forever. Without this, a failed ephemeral job would
		// block all future indexing attempts for that path until app restart.
		if self.config.is_ephemeral() {
			if let Some(local_path) = self.config.path.as_local_path() {
				ctx.library()
					.core_context()
					.ephemeral_cache()
					.mark_indexing_complete(local_path);
				match &result {
					Ok(_) => ctx.log(format!(
						"Marked ephemeral indexing complete for: {}",
						local_path.display()
					)),
					Err(e) => ctx.log(format!(
						"Marked ephemeral indexing complete (job failed: {}) for: {}",
						e,
						local_path.display()
					)),
				}
			}
		}

		result
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
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

			self.timer = Some(PhaseTimer::new());
		} else {
			warn!("DEBUG: IndexerJob has no state during resume - creating new state!");
			self.state = Some(IndexerState::new(&self.config.path));
		}
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing indexer job");
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
		self.state.is_some()
	}
}

impl IndexerJob {
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

	pub fn from_location(location_id: Uuid, root_path: SdPath, mode: IndexMode) -> Self {
		Self::new(IndexerJobConfig::new(location_id, root_path, mode))
	}

	pub fn shallow(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Shallow)
	}

	pub fn with_content(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Content)
	}

	pub fn deep(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Deep)
	}

	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self::new(IndexerJobConfig::ui_navigation(location_id, path))
	}

	/// Sets the ephemeral index storage that the job will use.
	///
	/// This must be called before dispatching ephemeral jobs. It allows external code
	/// (like the ephemeral cache manager) to maintain a reference to the same storage
	/// the job uses, enabling direct access to indexing results without job-to-caller
	/// communication overhead.
	pub fn set_ephemeral_index(&mut self, index: Arc<RwLock<EphemeralIndex>>) {
		self.ephemeral_index = Some(index);
	}

	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self::new(IndexerJobConfig::ephemeral_browse(path, scope))
	}

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

	async fn run_ephemeral_processing_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		ephemeral_index: Arc<RwLock<EphemeralIndex>>,
		root_path: &Path,
		_volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	) -> JobResult<()> {
		use super::persistence::PersistenceFactory;

		ctx.log("Starting ephemeral processing");

		let event_bus = Some(ctx.library().event_bus().clone());

		let persistence = PersistenceFactory::ephemeral(
			ephemeral_index.clone(),
			event_bus,
			root_path.to_path_buf(),
		);

		while let Some(batch) = state.entry_batches.pop() {
			for entry in batch {
				let _entry_id = persistence.store_entry(&entry, None, root_path).await?;
			}
		}

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
