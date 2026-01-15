//! Memory-efficient index for browsing paths outside managed locations.
//!
//! Ephemeral indexing lets users navigate unmanaged directories (network shares,
//! external drives) without adding them as permanent locations. Instead of writing
//! to the database, entries live in this memory-only structure until the session
//! ends or the path is promoted to a managed location.
//!
//! Memory usage is ~50 bytes per entry vs ~200 bytes with a naive `HashMap<PathBuf, Entry>`
//! approach. The optimization comes from:
//! - **NodeArena:** Contiguous slab allocation with pointer-sized entry IDs
//! - **NameCache:** String interning (one copy of "index.js" for thousands of node_modules files)
//! - **NameRegistry:** Trie-based prefix search without full-text indexing overhead
//!
//! Multiple directory trees can coexist in the same index (e.g., browsing both
//! `/mnt/nas` and `/media/usb` simultaneously), sharing the string interning pool
//! for maximum deduplication.

use crate::domain::ContentKind;
use crate::filetype::FileTypeRegistry;
use crate::ops::indexing::database_storage::EntryMetadata;
use crate::ops::indexing::state::{EntryKind, IndexerStats};

use super::types::{FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata};
use super::{EntryId, NameCache, NameRegistry, NodeArena};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Memory-efficient index for browsing unmanaged paths.
pub struct EphemeralIndex {
	arena: NodeArena,
	cache: Arc<NameCache>,
	registry: NameRegistry,
	path_index: HashMap<PathBuf, EntryId>,
	entry_uuids: HashMap<PathBuf, Uuid>,
	content_kinds: HashMap<PathBuf, ContentKind>,
	created_at: Instant,
	last_accessed: Instant,
	pub stats: IndexerStats,
}

/// Detailed memory breakdown by component
#[derive(Debug, Clone)]
pub struct MemoryBreakdown {
	pub arena: usize,
	pub cache: usize,
	pub registry: usize,
	pub path_index_overhead: usize,
	pub path_index_entries: usize,
	pub entry_uuids_overhead: usize,
	pub entry_uuids_entries: usize,
	pub content_kinds_overhead: usize,
	pub content_kinds_entries: usize,
}

impl MemoryBreakdown {
	pub fn total(&self) -> usize {
		self.arena
			+ self.cache
			+ self.registry
			+ self.path_index_overhead
			+ self.path_index_entries
			+ self.entry_uuids_overhead
			+ self.entry_uuids_entries
			+ self.content_kinds_overhead
			+ self.content_kinds_entries
	}

	pub fn path_index_total(&self) -> usize {
		self.path_index_overhead + self.path_index_entries
	}

	pub fn entry_uuids_total(&self) -> usize {
		self.entry_uuids_overhead + self.entry_uuids_entries
	}

	pub fn content_kinds_total(&self) -> usize {
		self.content_kinds_overhead + self.content_kinds_entries
	}
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
		let cache = Arc::new(NameCache::new());
		let arena = NodeArena::new()?;
		let registry = NameRegistry::new();

		let now = Instant::now();

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
	pub fn ensure_directory(&mut self, path: &Path) -> std::io::Result<EntryId> {
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
	) -> std::io::Result<Option<ContentKind>> {
		let registry = FileTypeRegistry::default();
		self.add_entry_with_registry(path, Some(uuid), metadata, &registry)
	}

	fn add_entry_with_registry(
		&mut self,
		path: PathBuf,
		uuid: Option<Uuid>,
		metadata: EntryMetadata,
		registry: &FileTypeRegistry,
	) -> std::io::Result<Option<ContentKind>> {
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

		let content_kind = if metadata.kind == EntryKind::File {
			registry.identify_by_extension(&path)
		} else if metadata.kind == EntryKind::Directory {
			ContentKind::Unknown
		} else {
			ContentKind::Unknown
		};

		self.path_index.insert(path.clone(), id);
		self.registry.insert(name, id);

		// Only store UUID if provided (volume indexing passes None to skip UUID generation)
		if let Some(uuid) = uuid {
			self.entry_uuids.insert(path.clone(), uuid);
		}

		self.content_kinds.insert(path, content_kind);

		self.last_accessed = Instant::now();
		Ok(Some(content_kind))
	}

	/// Add multiple entries in a batch (faster than individual add_entry calls)
	///
	/// Acquires write lock once for the entire batch instead of per-entry.
	pub fn add_entries_batch(
		&mut self,
		entries: Vec<(PathBuf, Option<Uuid>, EntryMetadata)>,
	) -> std::io::Result<Vec<Option<ContentKind>>> {
		let mut results = Vec::with_capacity(entries.len());

		// Create registry once for entire batch instead of per-file
		let registry = FileTypeRegistry::default();

		for (path, uuid, metadata) in entries {
			let result = self.add_entry_with_registry(path, uuid, metadata, &registry)?;
			results.push(result);
		}

		Ok(results)
	}

	pub fn get_entry(&mut self, path: &PathBuf) -> Option<EntryMetadata> {
		let id = self.path_index.get(path)?;
		let node = self.arena.get(*id)?;

		self.last_accessed = Instant::now();

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

	/// Get or assign a UUID for the given path (lazy generation).
	///
	/// Returns cached UUID if exists, otherwise generates a new random UUID
	/// and caches it. UUIDs are random (v4) for global uniqueness across devices,
	/// avoiding collisions when syncing ephemeral indexes that are later upgraded
	/// to persistent indexes.
	pub fn get_or_assign_uuid(&mut self, path: &PathBuf) -> Uuid {
		if let Some(&uuid) = self.entry_uuids.get(path) {
			return uuid;
		}

		let uuid = Uuid::new_v4();
		self.entry_uuids.insert(path.clone(), uuid);
		uuid
	}

	/// Get the path for an entry by its UUID
	pub fn get_path_by_uuid(&self, uuid: Uuid) -> Option<PathBuf> {
		self.entry_uuids
			.iter()
			.find(|(_, &entry_uuid)| entry_uuid == uuid)
			.map(|(path, _)| path.clone())
	}

	pub fn get_content_kind(&self, path: &PathBuf) -> ContentKind {
		self.content_kinds
			.get(path)
			.copied()
			.unwrap_or(ContentKind::Unknown)
	}

	pub fn list_directory(&self, path: &Path) -> Option<Vec<PathBuf>> {
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
		indexed_paths: &std::collections::HashSet<PathBuf>,
	) -> (usize, Vec<PathBuf>) {
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
		let mut children_to_remove: Vec<(PathBuf, EntryId)> = dir_node
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

	fn reconstruct_path(&self, id: EntryId) -> Option<PathBuf> {
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

	pub fn find_containing(&self, substring: &str) -> Vec<PathBuf> {
		self.registry
			.find_containing(substring)
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
		self.detailed_memory_breakdown().total()
	}

	/// Get a detailed breakdown of memory usage by component
	pub fn detailed_memory_breakdown(&self) -> MemoryBreakdown {
		// Estimate average path length from a sample
		let avg_path_len = self.estimate_avg_path_length();

		MemoryBreakdown {
			arena: self.arena.memory_usage(),
			cache: self.cache.memory_usage(),
			registry: self.registry.memory_usage(),
			// path_index: HashMap<PathBuf, EntryId>
			path_index_overhead: self.path_index.capacity(),
			path_index_entries: self.path_index.len()
				* (std::mem::size_of::<PathBuf>() + std::mem::size_of::<EntryId>() + avg_path_len),
			// entry_uuids: HashMap<PathBuf, Uuid>
			entry_uuids_overhead: self.entry_uuids.capacity(),
			entry_uuids_entries: self.entry_uuids.len()
				* (std::mem::size_of::<PathBuf>() + std::mem::size_of::<Uuid>() + avg_path_len),
			// content_kinds: HashMap<PathBuf, ContentKind>
			content_kinds_overhead: self.content_kinds.capacity(),
			content_kinds_entries: self.content_kinds.len()
				* (std::mem::size_of::<PathBuf>()
					+ std::mem::size_of::<ContentKind>() + avg_path_len),
		}
	}

	/// Estimate average path length by sampling entries
	fn estimate_avg_path_length(&self) -> usize {
		if self.path_index.is_empty() {
			return 80; // default estimate
		}

		// Sample up to 1000 paths to estimate average length
		let sample_size = self.path_index.len().min(1000);
		let total_len: usize = self
			.path_index
			.keys()
			.take(sample_size)
			.map(|p| p.as_os_str().len())
			.sum();

		total_len / sample_size
	}

	pub fn get_stats(&self) -> EphemeralIndexStats {
		EphemeralIndexStats {
			total_entries: self.arena.len(),
			unique_names: self.registry.unique_names(),
			interned_strings: self.cache.len(),
			memory_bytes: self.memory_usage(),
			total_file_bytes: self.stats.bytes,
			uuid_count: self.entry_uuids.len(),
		}
	}

	pub fn content_kinds_count(&self) -> usize {
		self.content_kinds.len()
	}

	pub fn path_index_count(&self) -> usize {
		self.path_index.len()
	}

	/// Check if an entry exists at the given path.
	pub fn has_entry(&self, path: &Path) -> bool {
		self.path_index.contains_key(path)
	}

	/// Remove an entry at the given path.
	///
	/// Returns true if the entry was removed, false if it didn't exist.
	/// For directories, this only removes the directory entry itself, not its children.
	/// Use `remove_directory_tree` to remove a directory and all its descendants.
	pub fn remove_entry(&mut self, path: &Path) -> bool {
		// Get the entry ID before removing from path_index
		let entry_id = self.path_index.remove(path);
		self.entry_uuids.remove(path);
		self.content_kinds.remove(path);

		// Also remove from parent's children list in arena
		if let Some(id) = entry_id {
			// Get the parent's entry ID
			if let Some(parent_path) = path.parent() {
				if let Some(&parent_id) = self.path_index.get(parent_path) {
					if let Some(parent_node) = self.arena.get_mut(parent_id) {
						parent_node.children.retain(|child_id| *child_id != id);
					}
				}
			}
		}

		entry_id.is_some()
	}

	/// Remove a directory and all its descendants.
	///
	/// Returns the number of entries removed.
	pub fn remove_directory_tree(&mut self, path: &Path) -> usize {
		// First, get the entry ID for the root directory to remove from parent
		let root_id = self.path_index.get(path).copied();

		let prefix = path.to_string_lossy().to_string();
		let keys_to_remove: Vec<_> = self
			.path_index
			.keys()
			.filter(|k| {
				let k_str = k.to_string_lossy();
				k_str == prefix || k_str.starts_with(&format!("{}/", prefix))
			})
			.cloned()
			.collect();

		let count = keys_to_remove.len();
		for key in keys_to_remove {
			self.path_index.remove(&key);
			self.entry_uuids.remove(&key);
			self.content_kinds.remove(&key);
		}

		// Remove root directory from parent's children list
		if let Some(id) = root_id {
			if let Some(parent_path) = path.parent() {
				if let Some(&parent_id) = self.path_index.get(parent_path) {
					if let Some(parent_node) = self.arena.get_mut(parent_id) {
						parent_node.children.retain(|child_id| *child_id != id);
					}
				}
			}
		}

		count
	}

	/// Reconstructs paths for all entries and returns them as a HashMap.
	///
	/// For large indexes, this can be expensive since it walks the tree to rebuild
	/// every path. Prefer using `list_directory()` or `find_by_name()` for targeted
	/// queries when possible.
	pub fn entries(&self) -> HashMap<PathBuf, EntryMetadata> {
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

	/// Save this index to a snapshot file for fast restoration
	///
	/// Snapshots are compressed with zstd and written atomically.
	pub fn save_snapshot(&self, snapshot_path: &Path) -> anyhow::Result<()> {
		super::snapshot::save_snapshot_impl(self, snapshot_path)
	}

	/// Load an index from a snapshot file
	///
	/// Returns None if the snapshot doesn't exist or is incompatible.
	pub fn load_snapshot(snapshot_path: &Path) -> anyhow::Result<Option<Self>> {
		super::snapshot::load_snapshot_impl(snapshot_path)
	}

	/// Internal accessor for snapshot serialization
	pub(super) fn snapshot_data(
		&self,
	) -> (
		&NodeArena,
		&Arc<NameCache>,
		&NameRegistry,
		&HashMap<PathBuf, EntryId>,
		&HashMap<PathBuf, Uuid>,
		&HashMap<PathBuf, ContentKind>,
		&IndexerStats,
	) {
		(
			&self.arena,
			&self.cache,
			&self.registry,
			&self.path_index,
			&self.entry_uuids,
			&self.content_kinds,
			&self.stats,
		)
	}

	/// Internal constructor for snapshot deserialization
	pub(super) fn from_snapshot_parts(
		arena: NodeArena,
		cache: Arc<NameCache>,
		registry: NameRegistry,
		path_index: HashMap<PathBuf, EntryId>,
		entry_uuids: HashMap<PathBuf, Uuid>,
		content_kinds: HashMap<PathBuf, ContentKind>,
		stats: IndexerStats,
	) -> Self {
		let now = Instant::now();
		Self {
			arena,
			cache,
			registry,
			path_index,
			entry_uuids,
			content_kinds,
			created_at: now,
			last_accessed: now,
			stats,
		}
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
	pub total_file_bytes: u64,
	pub uuid_count: usize,
}
