//! The arena: one drive's map, held in memory.
//!
//! Every walk and watcher change lands here, and listings and search read from
//! here while it answers. It is rebuildable from the filesystem and restored from
//! a snapshot across launches; what outlives it belongs to the source stores.
//!
//! Memory usage is ~50 bytes per entry vs ~200 bytes with a naive `HashMap<PathBuf, Entry>`
//! approach. The optimization comes from:
//! - **NodeArena:** Contiguous slab allocation with pointer-sized entry IDs
//! - **NameCache:** String interning (one copy of "index.js" for thousands of node_modules files)
//! - **NameRegistry:** Trie-based prefix search without full-text indexing overhead
//!
//! Multiple directory trees can coexist in the same arena (e.g., browsing both
//! `/mnt/nas` and `/media/usb` simultaneously), sharing the string interning pool
//! for maximum deduplication.
//!
//! ## Slots, identity and compaction
//!
//! `EntryId` is an arena slot and never leaves this module: the public API
//! speaks paths and uuids, listings page by path, search pages by offset over
//! paths, and the watcher addresses entries by path. That is what makes slot
//! reuse safe. A removed entry's slot goes on the `NodeArena` free list and the
//! next insert takes it, so a folder that is cleared and refilled reuses the
//! slots it had. When a drive shrinks for real the free list grows instead;
//! once vacant slots outnumber live entries (and there are at least
//! `COMPACT_MIN_VACANT` of them) the arena is rebuilt densely from its live
//! graph, renumbering every internal reference together. Record uuids, rollups,
//! stubs and query results survive both; only the slot numbers change. The
//! restart snapshot always writes that dense projection, so it carries live
//! nodes rather than history.
//!
//! The design is heavily inspired by Cardinal's search-cache implementation,
//! particularly the memory-mapped arena storage, string interning, and snapshot
//! persistence patterns. See: https://github.com/cardisoft/cardinal
//!
//! ```text
//! Arena
//! ├── NodeArena: Vec<FileNode>        - Contiguous node storage
//! ├── NameCache: BTreeSet<Box<str>>   - String interning pool
//! ├── NameRegistry: BTreeMap          - Fast name lookups
//! └── path_index: HashMap<PathBuf, EntryId>  - Path to node mapping
//! ```

use crate::domain::ContentKind;
use crate::filetype::FileTypeRegistry;
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::state::{EntryKind, IndexerStats};

use super::types::{
	FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata, Rollup, RollupDelta,
};
use super::{EntryId, NameCache, NameRegistry, NodeArena};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Compaction runs once vacant slots outnumber live entries and there are at
/// least this many of them. The first half bounds allocation at twice the live
/// tree and keeps the pass amortized (a rebuild costs one walk of the live
/// graph, paid for by at least as many removals); the second keeps a small
/// arena from rebuilding over a few hundred bytes of slack.
pub const COMPACT_MIN_VACANT: usize = 4_096;

/// Memory-efficient index for browsing unmanaged paths.
pub struct Arena {
	arena: NodeArena,
	cache: Arc<NameCache>,
	registry: NameRegistry,
	path_index: HashMap<PathBuf, EntryId>,
	id_to_path: HashMap<EntryId, PathBuf>,
	entry_uuids: HashMap<EntryId, Uuid>,
	content_kinds: HashMap<EntryId, ContentKind>,
	/// Collection membership flags (see `collections`), derived from name +
	/// kind at add time and recomputed on restore. Only flagged entries
	/// carry a row.
	collection_flags: HashMap<EntryId, u32>,
	/// Directories the walk counted but did not keep. The entry stands for its
	/// whole subtree and has no children, so its rollups cannot be rebuilt from
	/// the tree and are held here instead.
	stubs: HashMap<EntryId, Rollup>,
	created_at: Instant,
	last_accessed: Instant,
	/// Set by every mutation, cleared when the partition is snapshotted.
	/// Entry count cannot stand in for this: a rename, or a delete balanced by
	/// an add, leaves the count identical while changing what must persist.
	dirty: bool,
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
	pub id_to_path_overhead: usize,
	pub id_to_path_entries: usize,
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
			+ self.id_to_path_overhead
			+ self.id_to_path_entries
			+ self.entry_uuids_overhead
			+ self.entry_uuids_entries
			+ self.content_kinds_overhead
			+ self.content_kinds_entries
	}

	pub fn path_index_total(&self) -> usize {
		self.path_index_overhead + self.path_index_entries
	}

	pub fn id_to_path_total(&self) -> usize {
		self.id_to_path_overhead + self.id_to_path_entries
	}

	pub fn entry_uuids_total(&self) -> usize {
		self.entry_uuids_overhead + self.entry_uuids_entries
	}

	pub fn content_kinds_total(&self) -> usize {
		self.content_kinds_overhead + self.content_kinds_entries
	}
}

impl std::fmt::Debug for Arena {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Arena")
			.field("entry_count", &self.arena.live())
			.field("allocated_slots", &self.arena.len())
			.field("interned_names", &self.cache.len())
			.field("path_count", &self.path_index.len())
			.finish()
	}
}

impl Arena {
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
			id_to_path: HashMap::new(),
			entry_uuids: HashMap::new(),
			content_kinds: HashMap::new(),
			collection_flags: HashMap::new(),
			stubs: HashMap::new(),
			created_at: now,
			last_accessed: now,
			dirty: false,
			stats: IndexerStats::default(),
		})
	}

	fn mark_dirty(&mut self) {
		self.dirty = true;
	}

	/// Whether anything has changed since the last snapshot.
	pub fn is_dirty(&self) -> bool {
		self.dirty
	}

	/// Called after a successful snapshot write.
	pub fn clear_dirty(&mut self) {
		self.dirty = false;
	}

	/// Drop every trace of one entry: its lookups, its name in the search
	/// registry, and the heap its arena node holds.
	///
	/// Returns the node's rollup contribution so the caller can decide how to
	/// settle ancestors — a caller removing a whole subtree adjusts once at the
	/// top rather than once per descendant.
	fn detach(&mut self, path: &Path, id: EntryId) -> Rollup {
		let rollup = self.arena.get(id).map(Rollup::of).unwrap_or_default();

		self.path_index.remove(path);
		self.id_to_path.remove(&id);
		self.entry_uuids.remove(&id);
		self.content_kinds.remove(&id);
		self.collection_flags.remove(&id);
		if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
			self.registry.remove(&name.to_lowercase(), id);
		}
		self.arena.vacate(id);
		self.stubs.remove(&id);

		rollup
	}

	/// Every path at or below `root`, deepest first.
	///
	/// Driven from `path_index` rather than by walking children, so an entry
	/// whose parent link was already severed is still collected — otherwise a
	/// second pass over the same subtree would find it resident and treat it as
	/// a live duplicate.
	fn descendant_paths(&self, root: &Path) -> Vec<PathBuf> {
		let mut paths: Vec<PathBuf> = self
			.path_index
			.keys()
			.filter(|candidate| candidate.starts_with(root))
			.cloned()
			.collect();
		paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
		paths
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

		// The parent is being enumerated for real, so whatever count stood in
		// for its contents is superseded by the entries now arriving.
		if let Some(parent_id) = parent_id {
			self.unsummarise(parent_id);
		}

		let name = self.cache.intern(
			path.file_name()
				.map(|s| s.to_string_lossy())
				.as_deref()
				.unwrap_or(&path.to_string_lossy()),
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
		self.id_to_path.insert(id, path.to_path_buf());
		let search_key = Self::search_key(&self.cache, name);
		self.registry.insert(search_key, id);

		self.mark_dirty();
		Ok(id)
	}

	/// The registry key for a name: Unicode lowercase, interned. Search
	/// lowercases its query, so candidate selection stays case-insensitive
	/// while arena nodes keep the original name for display.
	fn search_key<'cache>(cache: &'cache NameCache, name: &'cache str) -> &'cache str {
		let folded = name.to_lowercase();
		if folded == name {
			name
		} else {
			cache.intern(&folded)
		}
	}

	/// Adds an entry to the index, returning its content kind if successful.
	///
	/// Content kind is identified by file extension (no I/O needed), which is
	/// sufficient for a walk where speed is critical. Returns Ok(None)
	/// if the entry already exists (prevents duplicate entries when re-indexing
	/// a directory).
	/// Returns the content kind for newly inserted entries (None for a
	/// duplicate) together with the uuid that identifies the entry — the
	/// existing one when the path was already indexed, so identity is stable
	/// across rescans regardless of what the caller minted.
	pub fn add_entry(
		&mut self,
		path: PathBuf,
		uuid: Uuid,
		metadata: EntryMetadata,
	) -> std::io::Result<(Option<ContentKind>, Uuid)> {
		self.add_entry_with_registry(path, Some(uuid), metadata, &FileTypeRegistry::current())
	}

	fn add_entry_with_registry(
		&mut self,
		path: PathBuf,
		uuid: Option<Uuid>,
		metadata: EntryMetadata,
		registry: &FileTypeRegistry,
	) -> std::io::Result<(Option<ContentKind>, Uuid)> {
		if let Some(&existing_id) = self.path_index.get(&path) {
			// A repeat sighting of a known path is how a modification arrives:
			// the watcher's update handler routes through here. Adopt the new
			// size and timestamps and carry the difference up the ancestor
			// chain, so rollups stay true and the size/mtime validator that
			// keys the thumbnail and block caches actually changes when the
			// file does.
			let previous = self
				.arena
				.get(existing_id)
				.map(Rollup::of)
				.unwrap_or_default();
			// A directory keeps whatever its children have already rolled up;
			// only a file carries its own figures.
			let current = if metadata.kind == EntryKind::Directory {
				previous
			} else {
				Rollup::file(metadata.size)
			};

			if let Some(node) = self.arena.get_mut(existing_id) {
				node.meta = PackedMetadata::new(
					NodeState::Accessible,
					FileType::from(metadata.kind),
					metadata.size,
				)
				.with_hidden(metadata.is_hidden)
				.with_times(metadata.modified, metadata.created);
				node.subtree_bytes = current.bytes;
				node.file_count = current.files;
			}

			let parent = self.arena.get(existing_id).and_then(|node| node.parent());
			self.bump_ancestors(parent, RollupDelta::between(previous, current));

			self.mark_dirty();
			self.last_accessed = Instant::now();

			// Re-use the entry's identity; assign one now if it never had one
			// (volume indexing passes None to defer uuid creation).
			let existing_uuid = *self
				.entry_uuids
				.entry(existing_id)
				.or_insert_with(Uuid::now_v7);
			return Ok((None, existing_uuid));
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

		// The parent is being enumerated for real, so whatever count stood in
		// for its contents is superseded by the entries now arriving.
		if let Some(parent_id) = parent_id {
			self.unsummarise(parent_id);
		}

		let name = self.cache.intern(
			path.file_name()
				.map(|s| s.to_string_lossy())
				.as_deref()
				.unwrap_or("unknown"),
		);

		let file_type = FileType::from(metadata.kind);

		let meta = PackedMetadata::new(NodeState::Accessible, file_type, metadata.size)
			.with_hidden(metadata.is_hidden)
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
		self.id_to_path.insert(id, path.clone());
		let search_key = Self::search_key(&self.cache, name);
		self.registry.insert(search_key, id);

		// Non-directories contribute their size, and themselves, to every
		// ancestor's rollups.
		if metadata.kind != EntryKind::Directory {
			let rollup = Rollup::file(metadata.size);
			if let Some(node) = self.arena.get_mut(id) {
				node.subtree_bytes = rollup.bytes;
				node.file_count = rollup.files;
			}
			self.bump_ancestors(parent_id, rollup.added());
		}

		// Volume indexing passes None to defer uuid creation until something
		// actually references the entry; get_or_assign_uuid covers that later.
		let entry_uuid = uuid.unwrap_or_else(Uuid::now_v7);
		if uuid.is_some() {
			self.entry_uuids.insert(id, entry_uuid);
		}

		self.content_kinds.insert(id, content_kind);

		if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
			let flags = super::collections::classify(file_name, content_kind);
			if flags != 0 {
				self.collection_flags.insert(id, flags);
			}
		}

		self.mark_dirty();
		self.last_accessed = Instant::now();
		Ok((Some(content_kind), entry_uuid))
	}

	/// Add multiple entries in a batch (faster than individual add_entry calls)
	///
	/// Acquires write lock once for the entire batch instead of per-entry.
	/// Answers with the identity each entry ended up with, which is the one the
	/// arena holds rather than the one the caller offered: a path already
	/// present keeps the uuid it had, and anything announcing the change has to
	/// name the identity a query will resolve.
	pub fn add_entries_batch(
		&mut self,
		entries: Vec<(PathBuf, Option<Uuid>, EntryMetadata)>,
	) -> std::io::Result<Vec<(Option<ContentKind>, Uuid)>> {
		let mut results = Vec::with_capacity(entries.len());
		let registry = FileTypeRegistry::current();

		for (path, uuid, metadata) in entries {
			results.push(self.add_entry_with_registry(path, uuid, metadata, &registry)?);
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
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: node.meta.is_hidden(),
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
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: node.meta.is_hidden(),
		})
	}

	pub fn get_entry_uuid(&self, path: &PathBuf) -> Option<Uuid> {
		let entry_id = self.path_index.get(path)?;
		self.entry_uuids.get(entry_id).copied()
	}

	/// Get or assign a UUID for the given path (lazy generation).
	///
	/// Returns the cached UUID if one exists, otherwise assigns and caches a
	/// v7 — the same identity later persisted when the entry is promoted to a
	/// durable record. An unknown path yields a transient uuid the index never
	/// records; callers must not persist it.
	pub fn get_or_assign_uuid(&mut self, path: &PathBuf) -> Uuid {
		// Look up EntryId for this path
		let entry_id = match self.path_index.get(path) {
			Some(&id) => id,
			None => return Uuid::now_v7(),
		};

		// Check if UUID already exists for this EntryId
		if let Some(&uuid) = self.entry_uuids.get(&entry_id) {
			return uuid;
		}

		// Generate and cache new UUID
		let uuid = Uuid::now_v7();
		self.entry_uuids.insert(entry_id, uuid);
		uuid
	}

	/// Get the path for an entry by its UUID
	pub fn get_path_by_uuid(&self, uuid: Uuid) -> Option<PathBuf> {
		// Find the EntryId that has this UUID
		let entry_id = self
			.entry_uuids
			.iter()
			.find(|(_, &entry_uuid)| entry_uuid == uuid)
			.map(|(&id, _)| id)?;

		// Look up the path for this EntryId
		self.path_index
			.iter()
			.find(|(_, &id)| id == entry_id)
			.map(|(path, _)| path.clone())
	}

	pub fn get_content_kind(&self, path: &PathBuf) -> ContentKind {
		let entry_id = match self.path_index.get(path) {
			Some(&id) => id,
			None => return ContentKind::Unknown,
		};

		self.content_kinds
			.get(&entry_id)
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

	/// Snapshot the known files in a scope without reading the filesystem.
	/// Summarised and unvisited directories contribute no invented descendants.
	pub fn files_in_scope(&self, root: &Path, recursive: bool) -> Option<Vec<PathBuf>> {
		let root_id = *self.path_index.get(root)?;
		let mut pending = vec![root_id];
		let mut files = Vec::new();
		while let Some(id) = pending.pop() {
			let Some(node) = self.arena.get(id) else {
				continue;
			};
			match node.meta.file_type() {
				FileType::File => {
					if let Some(path) = self.reconstruct_path(id) {
						files.push(path);
					}
				}
				FileType::Directory if id == root_id || recursive => {
					pending.extend(node.children.iter().copied());
				}
				_ => {}
			}
		}
		files.sort_unstable();
		Some(files)
	}

	/// Every known entry beneath a directory at any depth, files and
	/// directories alike, without reading the filesystem.
	pub fn entries_beneath(&self, root: &Path) -> Vec<PathBuf> {
		let Some(&root_id) = self.path_index.get(root) else {
			return Vec::new();
		};
		let mut pending = vec![root_id];
		let mut entries = Vec::new();
		while let Some(id) = pending.pop() {
			let Some(node) = self.arena.get(id) else {
				continue;
			};
			if id != root_id {
				if let Some(path) = self.reconstruct_path(id) {
					entries.push(path);
				}
			}
			pending.extend(node.children.iter().copied());
		}
		entries
	}

	/// Clears entries before re-indexing, preserving explicitly browsed subdirectories.
	///
	/// A browse walks one level, so subdirectories that were explicitly
	/// navigated to (in `indexed_paths`) are preserved as separate branches.
	/// Unbrowsed subdirectories are refreshed with the parent.
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

		let mut removed = Rollup::default();
		for (_, id) in &children_to_remove {
			if let Some(node) = self.arena.get(*id) {
				removed += Rollup::of(node);
			}
		}
		self.bump_ancestors(Some(dir_id), removed.removed());

		// Detach each child with everything beneath it. Removing only the child
		// would leave its descendants resident in `path_index` with a severed
		// parent chain, where the next add for one of those paths finds a live
		// entry and takes the duplicate branch — inheriting stale metadata that
		// no longer describes anything.
		for (child_path, child_id) in &children_to_remove {
			for descendant in self.descendant_paths(child_path) {
				if descendant == *child_path {
					continue;
				}
				if let Some(&descendant_id) = self.path_index.get(&descendant) {
					self.detach(&descendant, descendant_id);
				}
			}
			self.detach(child_path, *child_id);
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
			self.mark_dirty();
			self.compact_if_inflated();
			tracing::debug!(
				"Cleared {} entries from {} (preserved browsed subdirs)",
				cleared,
				dir_path.display()
			);
		}

		(cleared, deleted_browsed_dirs)
	}

	fn reconstruct_path(&self, id: EntryId) -> Option<PathBuf> {
		self.id_to_path.get(&id).cloned()
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

	/// Slots allocated so far, vacant ones included. The live count is
	/// `path_index_count`; the two differ by what the free list holds.
	pub fn len(&self) -> usize {
		self.arena.len()
	}

	pub fn is_empty(&self) -> bool {
		self.arena.is_empty()
	}

	/// Slots on the free list, waiting for an insert or a compaction.
	pub fn vacant_slots(&self) -> usize {
		self.arena.vacant()
	}

	/// Dense renumbering of the live slots, in slot order: `map[old]` is the
	/// slot a live node lands on in a compact arena, `None` for a vacant one.
	fn live_projection(&self) -> Vec<Option<EntryId>> {
		let mut map = vec![None; self.arena.len()];
		for (next, (old, _)) in self.arena.iter().enumerate() {
			map[old.as_usize()] = Some(EntryId::from_usize(next));
		}
		map
	}

	/// Rebuild the arena densely from its live graph.
	///
	/// Every internal reference is renumbered together: parent links, child
	/// lists, the path maps, uuid bindings, content kinds, collection flags,
	/// summary stubs and the name registry. Nothing a caller holds changes,
	/// since nothing outside this module holds a slot number. The new arena is
	/// built in full before it replaces the old one, so a failure to map the
	/// backing file leaves the index as it was.
	pub fn compact(&mut self) -> std::io::Result<()> {
		let map = self.live_projection();
		let renumber = |id: EntryId| map.get(id.as_usize()).copied().flatten();

		let mut fresh = NodeArena::with_capacity(self.arena.live())?;
		for (_, node) in self.arena.iter() {
			let parent = node
				.parent()
				.and_then(renumber)
				.map(MaybeEntryId::some)
				.unwrap_or(MaybeEntryId::NONE);
			let mut moved = FileNode::new(NameRef::new(node.name(), parent), node.meta);
			moved.children = node.children.iter().copied().filter_map(renumber).collect();
			moved.subtree_bytes = node.subtree_bytes;
			moved.file_count = node.file_count;
			fresh.insert(moved)?;
		}
		self.arena = fresh;

		self.path_index.retain(|_, id| match renumber(*id) {
			Some(new) => {
				*id = new;
				true
			}
			None => false,
		});
		self.id_to_path = self
			.path_index
			.iter()
			.map(|(path, &id)| (id, path.clone()))
			.collect();
		remap_keys(&mut self.entry_uuids, renumber);
		remap_keys(&mut self.content_kinds, renumber);
		remap_keys(&mut self.collection_flags, renumber);
		remap_keys(&mut self.stubs, renumber);
		self.registry.remap(renumber);
		Ok(())
	}

	/// Compact when the free list has outgrown the live tree. Called after
	/// every removal; a failure keeps the inflated arena, which still answers.
	fn compact_if_inflated(&mut self) {
		let vacant = self.arena.vacant();
		if vacant < COMPACT_MIN_VACANT || vacant <= self.arena.live() {
			return;
		}
		let live = self.arena.live();
		match self.compact() {
			Ok(()) => tracing::debug!(live, vacant, "compacted arena"),
			Err(err) => tracing::warn!(%err, live, vacant, "arena compaction failed"),
		}
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
			// id_to_path: HashMap<EntryId, PathBuf> (reverse index)
			id_to_path_overhead: self.id_to_path.capacity(),
			id_to_path_entries: self.id_to_path.len()
				* (std::mem::size_of::<EntryId>() + std::mem::size_of::<PathBuf>() + avg_path_len),
			// entry_uuids: HashMap<EntryId, Uuid> - now using EntryId keys!
			entry_uuids_overhead: self.entry_uuids.capacity(),
			entry_uuids_entries: self.entry_uuids.len()
				* (std::mem::size_of::<EntryId>() + std::mem::size_of::<Uuid>()),
			// content_kinds: HashMap<EntryId, ContentKind> - now using EntryId keys!
			content_kinds_overhead: self.content_kinds.capacity(),
			content_kinds_entries: self.content_kinds.len()
				* (std::mem::size_of::<EntryId>() + std::mem::size_of::<ContentKind>()),
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

	/// Adjust every ancestor's subtree rollups by `delta`, starting at `start`
	/// and following parent links to the root.
	fn bump_ancestors(&mut self, start: Option<EntryId>, delta: RollupDelta) {
		if delta.is_zero() {
			return;
		}
		let mut cur = start;
		while let Some(id) = cur {
			let Some(node) = self.arena.get_mut(id) else {
				break;
			};
			node.subtree_bytes = delta.apply_bytes(node.subtree_bytes);
			node.file_count = delta.apply_files(node.file_count);
			cur = node.parent();
		}
	}

	/// Sum of file sizes under `path` (a file's own size for files).
	pub fn subtree_size(&self, path: &Path) -> Option<u64> {
		let id = *self.path_index.get(path)?;
		self.arena.get(id).map(|n| n.subtree_bytes)
	}

	/// Total file bytes across the index: the sum of root-level rollups,
	/// counted over live entries only (arena slots persist after removal).
	pub fn total_file_bytes(&self) -> u64 {
		self.path_index
			.values()
			.filter_map(|&id| {
				let node = self.arena.get(id)?;
				node.parent().is_none().then_some(node.subtree_bytes)
			})
			.sum()
	}

	/// Rebuild every subtree rollup from live entries. Used after bulk loads
	/// (snapshot restore), where incremental maintenance did not run.
	pub fn recompute_rollups(&mut self) {
		for i in 0..self.arena.len() {
			if let Some(node) = self.arena.get_mut(EntryId::from_usize(i)) {
				node.subtree_bytes = 0;
				node.file_count = 0;
			}
		}
		let live: Vec<EntryId> = self.path_index.values().copied().collect();
		for id in live {
			let Some(node) = self.arena.get(id) else {
				continue;
			};
			// A summarised directory was never walked, so its figures cannot be
			// rebuilt from children it does not have. They come from the count
			// taken when the walk turned back.
			let rollup = if let Some(&stub) = self.stubs.get(&id) {
				stub
			} else if node.is_directory() {
				continue;
			} else {
				Rollup::file(node.meta.size())
			};
			let parent = node.parent();
			if let Some(node) = self.arena.get_mut(id) {
				node.subtree_bytes = rollup.bytes;
				node.file_count = rollup.files;
			}
			self.bump_ancestors(parent, rollup.added());
		}
	}

	/// Record a directory the walk counted but did not keep.
	///
	/// The entry stays, its children do not, and `totals` is what its subtree
	/// held when the walk turned back. Ancestors are adjusted by the difference,
	/// so a directory that was partly walked before being summarised does not
	/// count twice.
	pub fn summarise(&mut self, path: &Path, totals: Rollup) {
		let Some(&id) = self.path_index.get(path) else {
			return;
		};
		let Some(node) = self.arena.get(id) else {
			return;
		};
		if !node.is_directory() {
			return;
		}

		let (previous, parent) = (Rollup::of(node), node.parent());
		if let Some(node) = self.arena.get_mut(id) {
			node.subtree_bytes = totals.bytes;
			node.file_count = totals.files;
		}
		self.bump_ancestors(parent, RollupDelta::between(previous, totals));

		self.stubs.insert(id, totals);
		*self.collection_flags.entry(id).or_default() |= super::collections::SUMMARISED;
		self.mark_dirty();
	}

	/// Drop a directory's summary because it is being enumerated for real.
	///
	/// Its totals go with it: what it holds is about to come from its children,
	/// and leaving the count in place would add the subtree to every ancestor
	/// twice.
	fn unsummarise(&mut self, id: EntryId) {
		let Some(totals) = self.stubs.remove(&id) else {
			return;
		};
		if let Some(flags) = self.collection_flags.get_mut(&id) {
			*flags &= !super::collections::SUMMARISED;
			if *flags == 0 {
				self.collection_flags.remove(&id);
			}
		}
		let parent = self.arena.get(id).and_then(|node| node.parent());
		if let Some(node) = self.arena.get_mut(id) {
			node.subtree_bytes = 0;
			node.file_count = 0;
		}
		self.bump_ancestors(parent, totals.removed());
	}

	/// Whether this directory stands for a subtree that was never kept.
	pub fn is_summarised(&self, path: &Path) -> bool {
		self.path_index
			.get(path)
			.is_some_and(|id| self.stubs.contains_key(id))
	}

	/// Number of files under `path` (1 for a file).
	pub fn subtree_file_count(&self, path: &Path) -> Option<u32> {
		let id = *self.path_index.get(path)?;
		self.arena.get(id).map(|node| node.file_count)
	}

	/// Every directory standing in for a subtree nobody kept.
	pub fn summarised_paths(&self) -> Vec<PathBuf> {
		self.stubs
			.keys()
			.filter_map(|id| self.id_to_path.get(id).cloned())
			.collect()
	}

	/// Rebuild collection flags from names and kinds already in the index.
	/// Runs on snapshot restore (flags are derived data and never persist),
	/// and is safe to re-run when classification heuristics change.
	pub fn recompute_collections(&mut self) {
		// Snapshots written before the arena kept the hidden bit have it
		// clear. Where the name alone decides, it is recomputed here; on
		// Windows the bit is what the walk read from the attribute, and no
		// snapshot there predates it.
		#[cfg(not(windows))]
		for (path, &id) in &self.path_index {
			if let Some(node) = self.arena.get_mut(id) {
				node.meta = node.meta.with_hidden(super::metadata::is_hidden_path(path));
			}
		}

		self.collection_flags.clear();
		// Flags are derived from name and kind, neither of which says anything
		// about a summarised directory, so its marker is restored from the
		// stub table instead.
		for &id in self.stubs.keys() {
			self.collection_flags
				.insert(id, super::collections::SUMMARISED);
		}
		for (path, &id) in &self.path_index {
			let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
				continue;
			};
			let kind = self
				.content_kinds
				.get(&id)
				.copied()
				.unwrap_or(ContentKind::Unknown);
			let flags = super::collections::classify(file_name, kind);
			if flags != 0 {
				self.collection_flags.insert(id, flags);
			}
		}
	}

	/// Most recently modified files, newest first, capped at `limit`.
	/// Directories and entries without a stored mtime are skipped.
	pub fn recent_files(&self, limit: usize) -> Vec<(PathBuf, EntryMetadata)> {
		let mut candidates: Vec<(std::time::SystemTime, &PathBuf)> = self
			.path_index
			.iter()
			.filter_map(|(path, &id)| {
				let node = self.arena.get(id)?;
				if node.is_directory() {
					return None;
				}
				let mtime = node.meta.mtime_as_system_time()?;
				Some((mtime, path))
			})
			.collect();
		candidates.sort_by(|a, b| b.0.cmp(&a.0));
		candidates.truncate(limit);
		candidates
			.into_iter()
			.filter_map(|(_, path)| Some((path.clone(), self.get_entry_ref(path)?)))
			.collect()
	}

	/// Paths of every entry whose flags intersect the mask.
	pub fn collection_paths(&self, mask: u32) -> Vec<PathBuf> {
		self.collection_flags
			.iter()
			.filter(|(_, flags)| *flags & mask != 0)
			.filter_map(|(id, _)| self.id_to_path.get(id).cloned())
			.collect()
	}

	pub fn collection_count(&self, mask: u32) -> usize {
		self.collection_flags
			.values()
			.filter(|flags| **flags & mask != 0)
			.count()
	}

	pub fn get_stats(&self) -> ArenaStats {
		ArenaStats {
			total_entries: self.arena.live(),
			allocated_slots: self.arena.len(),
			vacant_slots: self.arena.vacant(),
			capacity: self.arena.capacity(),
			unique_names: self.registry.unique_names(),
			interned_strings: self.cache.len(),
			memory_bytes: self.memory_usage(),
			total_file_bytes: self.total_file_bytes(),
			uuid_count: self.entry_uuids.len(),
		}
	}

	pub fn content_kinds_count(&self) -> usize {
		self.content_kinds.len()
	}

	pub fn path_index_count(&self) -> usize {
		self.path_index.len()
	}

	/// Count entries under a specific path prefix (including the prefix itself if it exists)
	pub fn count_entries_under_path(&self, prefix: &Path) -> usize {
		let prefix_str = prefix.to_string_lossy();
		self.path_index
			.keys()
			.filter(|path| {
				let path_str = path.to_string_lossy();
				// Match paths that are under the prefix (including the prefix itself)
				path_str.starts_with(prefix_str.as_ref())
					&& (path_str.len() == prefix_str.len()
						|| path_str.as_bytes().get(prefix_str.len()) == Some(&b'/')
						|| path_str.as_bytes().get(prefix_str.len()) == Some(&b'\\'))
			})
			.count()
	}

	/// Check if an entry exists at the given path.
	pub fn has_entry(&self, path: &Path) -> bool {
		self.path_index.contains_key(path)
	}

	/// Remove an entry at the given path.
	///
	/// Returns true if the entry was removed, false if it didn't exist.
	/// A directory goes with everything beneath it: a descendant left resident
	/// would keep a parent link to a slot that the next insert may reuse for
	/// an unrelated node, and from then on its bytes would roll up into the
	/// wrong directory. A rename is `rename`, which keeps the subtree.
	pub fn remove_entry(&mut self, path: &Path) -> bool {
		let Some(id) = self.path_index.get(path).copied() else {
			return false;
		};

		if self.arena.get(id).is_some_and(|node| node.is_directory()) {
			return self.remove_directory_tree(path) > 0;
		}

		let parent = self.arena.get(id).and_then(|node| node.parent());

		// Drop the parent's link before the node goes, so no child list keeps an
		// id whose slot has been vacated.
		if let Some(parent_id) = parent {
			if let Some(parent_node) = self.arena.get_mut(parent_id) {
				parent_node.children.retain(|child_id| *child_id != id);
			}
		}

		let removed = self.detach(path, id);
		self.bump_ancestors(parent, removed.removed());

		self.mark_dirty();
		self.compact_if_inflated();
		true
	}

	/// Move an entry, with everything beneath it, to a new path.
	///
	/// A directory takes its subtree's addresses with it, as the store's
	/// `rename_tree` does, so a renamed folder keeps listing its files and
	/// every descendant keeps its uuid, kind and rollup. Identity, size and
	/// times for the moved entry itself come from `to`. Anything already at
	/// the destination is removed first, as a rename over it on disk would.
	/// Returns the entry's uuid, the one it had or `uuid` if it had none, or
	/// `None` when `from` is not indexed.
	pub fn rename(&mut self, from: &Path, to: EntryMetadata, uuid: Uuid) -> Option<Uuid> {
		if !self.path_index.contains_key(from) {
			return None;
		}
		if from == to.path {
			let id = self.path_index[from];
			return Some(*self.entry_uuids.entry(id).or_insert(uuid));
		}
		// Clearing the destination may compact, so look the slot up after it.
		if self.path_index.contains_key(&to.path) {
			self.remove_entry(&to.path);
		}
		let id = self.path_index.get(from).copied()?;

		let (rollup, old_parent, is_directory) = {
			let node = self.arena.get(id)?;
			(Rollup::of(node), node.parent(), node.is_directory())
		};
		if let Some(parent_node) = old_parent.and_then(|parent| self.arena.get_mut(parent)) {
			parent_node.children.retain(|child| *child != id);
		}
		self.bump_ancestors(old_parent, rollup.removed());
		if let Some(name) = from.file_name().and_then(|name| name.to_str()) {
			self.registry.remove(&name.to_lowercase(), id);
		}

		let new_parent = match to.path.parent() {
			Some(parent) if !parent.as_os_str().is_empty() => match self.ensure_directory(parent) {
				Ok(parent_id) => Some(parent_id),
				Err(err) => {
					tracing::warn!(%err, "could not build the destination's ancestry");
					None
				}
			},
			_ => None,
		};
		if let Some(parent_id) = new_parent {
			self.unsummarise(parent_id);
			if let Some(parent_node) = self.arena.get_mut(parent_id) {
				parent_node.add_child(id);
			}
		}

		let name = self.cache.intern(
			to.path
				.file_name()
				.map(|s| s.to_string_lossy())
				.as_deref()
				.unwrap_or("unknown"),
		);
		let search_key = Self::search_key(&self.cache, name);
		self.registry.insert(search_key, id);
		let parent_ref = new_parent
			.map(MaybeEntryId::some)
			.unwrap_or(MaybeEntryId::NONE);
		let rollup = if is_directory {
			rollup
		} else {
			Rollup::file(to.size)
		};
		if let Some(node) = self.arena.get_mut(id) {
			node.name_ref = NameRef::new(name, parent_ref);
			node.meta =
				PackedMetadata::new(NodeState::Accessible, FileType::from(to.kind), to.size)
					.with_hidden(to.is_hidden)
					.with_times(to.modified, to.created);
			node.subtree_bytes = rollup.bytes;
			node.file_count = rollup.files;
		}
		self.bump_ancestors(new_parent, rollup.added());

		for old_path in self.descendant_paths(from) {
			let Some(descendant) = self.path_index.remove(&old_path) else {
				continue;
			};
			// `join("")` appends a separator, so the entry's own path is taken
			// as given rather than built from an empty remainder.
			let new_path = match old_path.strip_prefix(from) {
				Ok(rest) if rest.as_os_str().is_empty() => to.path.clone(),
				Ok(rest) => to.path.join(rest),
				Err(_) => old_path,
			};
			self.id_to_path.insert(descendant, new_path.clone());
			self.path_index.insert(new_path, descendant);
		}

		// The name decides kind and collection flags; the subtree keeps its own.
		if !is_directory {
			let kind = FileTypeRegistry::current().identify_by_extension(&to.path);
			self.content_kinds.insert(id, kind);
			let flags = to
				.path
				.file_name()
				.and_then(|n| n.to_str())
				.map(|file_name| super::collections::classify(file_name, kind))
				.unwrap_or(0);
			if flags == 0 {
				self.collection_flags.remove(&id);
			} else {
				self.collection_flags.insert(id, flags);
			}
		}

		self.mark_dirty();
		self.last_accessed = Instant::now();
		Some(*self.entry_uuids.entry(id).or_insert(uuid))
	}

	/// Remove a directory and all its descendants.
	///
	/// Returns the number of entries removed.
	pub fn remove_directory_tree(&mut self, path: &Path) -> usize {
		// First, get the entry ID for the root directory to remove from parent
		let root_id = self.path_index.get(path).copied();

		if let Some(node) = root_id.and_then(|id| self.arena.get(id)) {
			let (removed, parent) = (Rollup::of(node), node.parent());
			self.bump_ancestors(parent, removed.removed());
		}

		let keys_to_remove = self.descendant_paths(path);

		let count = keys_to_remove.len();
		for key in keys_to_remove {
			if let Some(entry_id) = self.path_index.get(&key).copied() {
				self.detach(&key, entry_id);
			}
		}
		if count > 0 {
			self.mark_dirty();
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

		self.compact_if_inflated();
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
					uid: None,
					gid: None,
					link_target: None,
					is_hidden: node.meta.is_hidden(),
				};
				result.insert(path.clone(), metadata);
			}
		}

		result
	}

	/// Save this index to a snapshot file for fast restoration.
	///
	/// Snapshots are compressed with zstd, written atomically, and carry the
	/// owning source id and root path so a restore can verify what it holds.
	pub fn save_snapshot(
		&self,
		snapshot_path: &Path,
		source_id: Uuid,
		root_path: &Path,
	) -> anyhow::Result<()> {
		super::snapshot::save_snapshot_impl(self, snapshot_path, source_id, root_path)
	}

	/// Load an index from a snapshot file.
	///
	/// Returns None if the snapshot doesn't exist or is incompatible; an
	/// incompatible file is removed so the source reindexes cleanly.
	pub fn load_snapshot(
		snapshot_path: &Path,
	) -> anyhow::Result<Option<(Self, super::snapshot::SnapshotMeta)>> {
		super::snapshot::load_snapshot_impl(snapshot_path)
	}

	/// The dense renumbering a snapshot is written through, so the artifact
	/// carries live nodes rather than every slot ever allocated.
	pub(super) fn snapshot_projection(&self) -> Vec<Option<EntryId>> {
		self.live_projection()
	}

	/// Internal accessor for snapshot serialization
	pub(super) fn snapshot_data(
		&self,
	) -> (
		&NodeArena,
		&Arc<NameCache>,
		&NameRegistry,
		&HashMap<PathBuf, EntryId>,
		&HashMap<EntryId, Uuid>,
		&HashMap<EntryId, ContentKind>,
		&IndexerStats,
		&HashMap<EntryId, Rollup>,
	) {
		(
			&self.arena,
			&self.cache,
			&self.registry,
			&self.path_index,
			&self.entry_uuids,
			&self.content_kinds,
			&self.stats,
			&self.stubs,
		)
	}

	/// Internal constructor for snapshot deserialization
	pub(super) fn from_snapshot_parts(
		arena: NodeArena,
		cache: Arc<NameCache>,
		registry: NameRegistry,
		path_index: HashMap<PathBuf, EntryId>,
		entry_uuids: HashMap<EntryId, Uuid>,
		content_kinds: HashMap<EntryId, ContentKind>,
		stats: IndexerStats,
		stubs: HashMap<EntryId, Rollup>,
	) -> Self {
		let now = Instant::now();
		// Rebuild reverse index from path_index
		let id_to_path: HashMap<EntryId, PathBuf> = path_index
			.iter()
			.map(|(path, &id)| (id, path.clone()))
			.collect();
		let mut index = Self {
			arena,
			cache,
			registry,
			path_index,
			id_to_path,
			entry_uuids,
			content_kinds,
			collection_flags: HashMap::new(),
			stubs,
			created_at: now,
			last_accessed: now,
			dirty: false,
			stats,
		};
		index.recompute_rollups();
		index.recompute_collections();
		index
	}
}

impl Default for Arena {
	fn default() -> Self {
		Self::new().expect("Failed to create default Arena")
	}
}

/// Renumber a map keyed by slot through `renumber`, dropping entries for
/// slots it no longer knows.
fn remap_keys<V>(map: &mut HashMap<EntryId, V>, renumber: impl Fn(EntryId) -> Option<EntryId>) {
	*map = std::mem::take(map)
		.into_iter()
		.filter_map(|(id, value)| renumber(id).map(|new| (new, value)))
		.collect();
}

/// Statistics about an arena
#[derive(Debug, Clone)]
pub struct ArenaStats {
	/// Live entries.
	pub total_entries: usize,
	/// Slots handed out, vacant ones included.
	pub allocated_slots: usize,
	/// Slots on the free list.
	pub vacant_slots: usize,
	/// Slots the backing mapping can hold before it grows.
	pub capacity: usize,
	pub unique_names: usize,
	pub interned_strings: usize,
	pub memory_bytes: usize,
	pub total_file_bytes: u64,
	pub uuid_count: usize,
}

#[cfg(test)]
mod rollup_tests {
	use super::*;

	fn meta(path: &Path, kind: EntryKind, size: u64) -> EntryMetadata {
		EntryMetadata {
			kind,
			path: path.to_path_buf(),
			size,
			modified: None,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		}
	}

	#[test]
	fn rollups_track_adds_and_removes_up_the_chain() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let a = root.join("a");
		let f1 = a.join("one.bin");
		let f2 = a.join("two.bin");
		let f3 = root.join("top.bin");

		index
			.add_entry(f1.clone(), Uuid::now_v7(), meta(&f1, EntryKind::File, 100))
			.unwrap();
		index
			.add_entry(f2.clone(), Uuid::now_v7(), meta(&f2, EntryKind::File, 50))
			.unwrap();
		index
			.add_entry(f3.clone(), Uuid::now_v7(), meta(&f3, EntryKind::File, 7))
			.unwrap();

		assert_eq!(index.subtree_size(&a), Some(150));
		assert_eq!(index.subtree_size(&root), Some(157));
		assert_eq!(index.subtree_size(&f1), Some(100));
		assert_eq!(index.total_file_bytes(), 157);

		// Duplicate adds do not double-count.
		index
			.add_entry(f1.clone(), Uuid::now_v7(), meta(&f1, EntryKind::File, 100))
			.unwrap();
		assert_eq!(index.subtree_size(&root), Some(157));

		index.remove_entry(&f2);
		assert_eq!(index.subtree_size(&a), Some(100));
		assert_eq!(index.subtree_size(&root), Some(107));

		index.remove_directory_tree(&a);
		assert_eq!(index.subtree_size(&root), Some(7));
		assert_eq!(index.total_file_bytes(), 7);
	}

	#[test]
	fn file_counts_ride_alongside_bytes() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let a = root.join("a");
		let f1 = a.join("one.bin");
		let f2 = a.join("two.bin");
		let empty = a.join("empty.bin");

		for (path, size) in [(&f1, 100), (&f2, 50), (&empty, 0)] {
			index
				.add_entry(
					path.clone(),
					Uuid::now_v7(),
					meta(path, EntryKind::File, size),
				)
				.unwrap();
		}

		// A zero-byte file still counts as a file.
		assert_eq!(index.subtree_file_count(&a), Some(3));
		assert_eq!(index.subtree_file_count(&root), Some(3));
		assert_eq!(index.subtree_file_count(&f1), Some(1));

		index.remove_entry(&f2);
		assert_eq!(index.subtree_file_count(&a), Some(2));

		index.remove_directory_tree(&a);
		assert_eq!(index.subtree_file_count(&root), Some(0));
	}

	#[test]
	fn a_summarised_directory_reports_a_subtree_it_does_not_hold() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let library = root.join("Library");
		let kept = root.join("keep.bin");

		index
			.add_entry(
				kept.clone(),
				Uuid::now_v7(),
				meta(&kept, EntryKind::File, 10),
			)
			.unwrap();
		index
			.add_entry(
				library.clone(),
				Uuid::now_v7(),
				meta(&library, EntryKind::Directory, 0),
			)
			.unwrap();

		index.summarise(
			&library,
			Rollup {
				bytes: 1_000,
				files: 40,
			},
		);

		assert!(index.is_summarised(&library));
		assert_eq!(index.subtree_size(&library), Some(1_000));
		assert_eq!(index.subtree_file_count(&library), Some(40));
		assert_eq!(index.subtree_size(&root), Some(1_010));
		assert_eq!(index.subtree_file_count(&root), Some(41));
		assert!(index
			.list_directory(&library)
			.unwrap_or_default()
			.is_empty());
	}

	#[test]
	fn enumerating_a_summarised_directory_replaces_its_count() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let dir = root.join("Applications");
		let app = dir.join("Thing.app");

		index
			.add_entry(
				dir.clone(),
				Uuid::now_v7(),
				meta(&dir, EntryKind::Directory, 0),
			)
			.unwrap();
		index.summarise(
			&dir,
			Rollup {
				bytes: 500,
				files: 5,
			},
		);

		// Drilling in lists the directory for real, and what it holds is now
		// what its children say rather than what the count said.
		index
			.add_entry(
				app.clone(),
				Uuid::now_v7(),
				meta(&app, EntryKind::File, 300),
			)
			.unwrap();

		assert!(!index.is_summarised(&dir));
		assert_eq!(index.subtree_size(&dir), Some(300));
		assert_eq!(index.subtree_size(&root), Some(300));
		assert_eq!(index.subtree_file_count(&root), Some(1));
	}

	#[test]
	fn recounting_a_summary_settles_ancestors_once() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let dir = root.join("Library");

		index
			.add_entry(
				dir.clone(),
				Uuid::now_v7(),
				meta(&dir, EntryKind::Directory, 0),
			)
			.unwrap();
		index.summarise(
			&dir,
			Rollup {
				bytes: 900,
				files: 9,
			},
		);
		index.summarise(
			&dir,
			Rollup {
				bytes: 100,
				files: 1,
			},
		);

		assert_eq!(index.subtree_size(&root), Some(100));
		assert_eq!(index.subtree_file_count(&root), Some(1));
	}

	#[test]
	fn repeat_add_adopts_new_size_and_times() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let file = root.join("clip.mov");

		let uuid = Uuid::now_v7();
		index
			.add_entry(file.clone(), uuid, meta(&file, EntryKind::File, 100))
			.unwrap();

		let grown = EntryMetadata {
			modified: Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)),
			..meta(&file, EntryKind::File, 400)
		};
		let (kind, same_uuid) = index
			.add_entry(file.clone(), Uuid::now_v7(), grown)
			.unwrap();

		// The second sighting is not a new entry, and identity is preserved.
		assert!(kind.is_none());
		assert_eq!(same_uuid, uuid);

		// The size the caches key on has to move with the file.
		let seen = index.get_entry(&file).expect("entry present");
		assert_eq!(seen.size, 400);
		assert!(seen.modified.is_some());

		// Ancestors carry the difference, not the original.
		assert_eq!(index.subtree_size(&file), Some(400));
		assert_eq!(index.subtree_size(&root), Some(400));
		assert_eq!(index.total_file_bytes(), 400);
	}

	#[test]
	fn clearing_a_directory_takes_its_whole_subtree() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let branch = root.join("branch");
		let deep = branch.join("inner").join("leaf.bin");
		let kept = root.join("kept.bin");

		index
			.add_entry(
				deep.clone(),
				Uuid::now_v7(),
				meta(&deep, EntryKind::File, 64),
			)
			.unwrap();
		index
			.add_entry(
				kept.clone(),
				Uuid::now_v7(),
				meta(&kept, EntryKind::File, 8),
			)
			.unwrap();

		let before = index.arena.len();
		index.clear_directory_children(&root, &std::collections::HashSet::new());

		// Nothing under the cleared branch may still resolve: a descendant left
		// behind would be found by the next add and treated as a live duplicate.
		assert!(index.get_entry(&deep).is_none());
		assert!(index.get_entry(&branch.join("inner")).is_none());
		assert!(index.find_by_name("leaf.bin").is_empty());
		assert_eq!(index.subtree_size(&root), Some(0));

		// Slots are released and reused, so re-adding a cleared path is a fresh
		// entry that costs no new allocation.
		let reused = index
			.add_entry(
				deep.clone(),
				Uuid::now_v7(),
				meta(&deep, EntryKind::File, 64),
			)
			.unwrap();
		assert!(
			reused.0.is_some(),
			"re-adding a cleared path is a fresh entry"
		);
		assert_eq!(index.subtree_size(&root), Some(64));
		assert_eq!(index.arena.len(), before);
	}

	/// R8 "Repeated subtree clear and refill".
	///
	/// A hundred clear-and-refill cycles over one branch leave the same live
	/// tree, so the allocation behind it must stay bounded by that tree and
	/// a snapshot must carry live nodes rather than historical slots. Entries
	/// outside the churn keep their identities throughout.
	#[test]
	fn repeated_clear_and_refill_keeps_allocation_bounded() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let branch = root.join("branch");
		let kept = root.join("kept.bin");
		let kept_uuid = Uuid::now_v7();
		index
			.add_entry(kept.clone(), kept_uuid, meta(&kept, EntryKind::File, 8))
			.unwrap();

		let fill = |index: &mut Arena| {
			for i in 0..50 {
				let path = branch.join(format!("file-{i}.bin"));
				index
					.add_entry(
						path.clone(),
						Uuid::now_v7(),
						meta(&path, EntryKind::File, 1),
					)
					.unwrap();
			}
		};
		fill(&mut index);
		let live = index.path_index_count();
		let allocated_after_one_fill = index.len();

		for _ in 0..100 {
			index.remove_directory_tree(&branch);
			fill(&mut index);
		}

		assert_eq!(index.path_index_count(), live, "the live tree is unchanged");
		assert_eq!(
			index.get_entry_uuid(&kept),
			Some(kept_uuid),
			"an entry outside the churn keeps its identity"
		);
		assert_eq!(index.find_by_name("file-7.bin").len(), 1);
		assert!(
			index.len() <= allocated_after_one_fill * 2,
			"allocation follows the live tree, not its history: {} slots for {} live paths",
			index.len(),
			live
		);

		let dir = tempfile::tempdir().unwrap();
		let snapshot = dir.path().join("arena.snapshot");
		index
			.save_snapshot(&snapshot, Uuid::now_v7(), &root)
			.unwrap();
		let (restored, _) = Arena::load_snapshot(&snapshot).unwrap().expect("snapshot");
		assert_eq!(restored.path_index_count(), live);
		assert!(
			restored.len() <= allocated_after_one_fill * 2,
			"the snapshot reconstructs live nodes, not every slot ever allocated: {}",
			restored.len()
		);
	}

	/// Every live node must be reachable the way the tree says: its parent
	/// lists it, its children are live and point back, and its path map entry
	/// agrees. Compaction renumbers all of these together, and this is the
	/// check that it did.
	fn assert_consistent(index: &Arena) {
		assert_eq!(index.path_index.len(), index.id_to_path.len());
		for (path, &id) in &index.path_index {
			let node = index.arena.get(id).expect("live path has a node");
			assert_eq!(index.id_to_path.get(&id), Some(path));
			if let Some(name) = path.file_name() {
				assert_eq!(name.to_string_lossy(), node.name(), "{}", path.display());
			}
			if let Some(parent) = node.parent() {
				let parent_node = index.arena.get(parent).expect("parent is live");
				assert!(parent_node.children.contains(&id));
				assert_eq!(
					index.id_to_path.get(&parent).map(PathBuf::as_path),
					path.parent()
				);
			}
			for &child in &node.children {
				let child_node = index.arena.get(child).expect("child is live");
				assert_eq!(child_node.parent(), Some(id));
			}
		}
		for id in index.entry_uuids.keys().chain(index.content_kinds.keys()) {
			assert!(index.id_to_path.contains_key(id));
		}
	}

	fn fill_files(index: &mut Arena, dir: &Path, count: usize) {
		let entries = (0..count)
			.map(|i| {
				let path = dir
					.join(format!("d{}", i % 7))
					.join(format!("file-{i}.bin"));
				let metadata = meta(&path, EntryKind::File, i as u64 + 1);
				(path, Some(Uuid::now_v7()), metadata)
			})
			.collect();
		index.add_entries_batch(entries).unwrap();
	}

	/// R4: a drive that shrinks for real leaves vacant slots behind that no
	/// refill will take. Once they outnumber the live tree the arena rebuilds
	/// itself, and nothing a caller can observe changes.
	#[test]
	fn a_shrunken_tree_compacts_once_vacancy_outgrows_it() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let total = COMPACT_MIN_VACANT * 2 + 500;
		fill_files(&mut index, &root, total);
		let keep = root.join("d3").join("file-3.bin");
		let keep_uuid = index.get_entry_uuid(&keep).unwrap();
		let before = index.len();

		for i in 0..total {
			let path = root
				.join(format!("d{}", i % 7))
				.join(format!("file-{i}.bin"));
			if path != keep {
				index.remove_entry(&path);
			}
		}

		let live = index.path_index_count();
		assert_eq!(live, 10, "/, /vol, seven buckets and the kept file");
		// Removals after the pass leave vacancies below the floor, so the
		// bound is the live tree plus that floor, not the slots ever used.
		assert!(
			index.vacant_slots() < COMPACT_MIN_VACANT && index.len() < live + COMPACT_MIN_VACANT,
			"the free list was folded away: {} slots ({} vacant) for {live} live paths (was {before})",
			index.len(),
			index.vacant_slots()
		);
		assert!(index.len() < before / 10);
		assert_eq!(index.get_entry_uuid(&keep), Some(keep_uuid));
		assert_eq!(index.subtree_size(&root), Some(4));
		assert_eq!(index.find_by_name("file-3.bin"), vec![keep.clone()]);
		assert_eq!(
			index.list_directory(&root.join("d3")),
			Some(vec![keep.clone()])
		);
		assert_consistent(&index);

		// The compacted arena keeps working as a tree: new entries land under
		// renumbered parents and roll up to them.
		let again = root.join("d0").join("late.bin");
		index
			.add_entry(
				again.clone(),
				Uuid::now_v7(),
				meta(&again, EntryKind::File, 10),
			)
			.unwrap();
		assert_eq!(index.subtree_size(&root.join("d0")), Some(10));
		assert_eq!(index.subtree_size(&root), Some(14));
		assert_consistent(&index);
	}

	/// R4: whatever the session's slot history, the restart snapshot carries
	/// the live tree and nothing else, and what it restores answers the same.
	#[test]
	fn a_snapshot_after_churn_carries_only_live_nodes() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let branch = root.join("branch");
		let summarised = root.join("Library");
		index
			.add_entry(
				summarised.clone(),
				Uuid::now_v7(),
				meta(&summarised, EntryKind::Directory, 0),
			)
			.unwrap();
		index.summarise(
			&summarised,
			Rollup {
				bytes: 900,
				files: 9,
			},
		);
		for cycle in 0..20 {
			index.remove_directory_tree(&branch);
			fill_files(&mut index, &branch, 60 + cycle);
		}
		// Leave vacant slots behind, below the compaction trigger, so the
		// writer has history to leave out.
		for i in 0..40 {
			index.remove_entry(
				&branch
					.join(format!("d{}", i % 7))
					.join(format!("file-{i}.bin")),
			);
		}
		assert!(index.vacant_slots() > 0, "the test needs an inflated arena");

		let live = index.path_index_count();
		let uuids: Vec<(PathBuf, Uuid)> = index
			.path_index
			.keys()
			.filter_map(|path| Some((path.clone(), index.get_entry_uuid(path)?)))
			.collect();
		assert!(uuids.len() > 30);
		let sizes: Vec<(PathBuf, u64, EntryKind)> = index
			.entries()
			.into_iter()
			.map(|(path, entry)| (path, entry.size, entry.kind))
			.collect();

		let dir = tempfile::tempdir().unwrap();
		let snapshot = dir.path().join("arena.snapshot");
		index
			.save_snapshot(&snapshot, Uuid::now_v7(), &root)
			.unwrap();
		let (restored, _) = Arena::load_snapshot(&snapshot).unwrap().expect("snapshot");

		assert_eq!(restored.len(), live, "one slot per live node");
		assert_eq!(restored.vacant_slots(), 0);
		for (path, size, kind) in sizes {
			let entry = restored.get_entry_ref(&path).expect("restored entry");
			assert_eq!((entry.size, entry.kind), (size, kind), "{}", path.display());
		}
		for (path, uuid) in uuids {
			assert_eq!(
				restored.get_entry_uuid(&path),
				Some(uuid),
				"{}",
				path.display()
			);
		}
		assert_eq!(restored.subtree_size(&root), index.subtree_size(&root));
		assert_eq!(
			restored.subtree_file_count(&branch),
			index.subtree_file_count(&branch)
		);
		assert!(restored.is_summarised(&summarised));
		assert_eq!(restored.subtree_size(&summarised), Some(900));
		assert_eq!(
			restored.find_by_name("file-50.bin"),
			index.find_by_name("file-50.bin")
		);
		assert_consistent(&restored);
	}

	/// Listings and search page by path (the explorer's cursor is a directory
	/// and a name, search pages by offset over sorted paths), so a cursor taken
	/// before a compaction names the same place afterwards. Slot numbers never
	/// reach a caller, which is the rule that lets compaction renumber freely.
	#[test]
	fn a_listing_cursor_taken_before_compaction_still_pages() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		fill_files(&mut index, &root, 200);
		for i in (0..200).step_by(3) {
			index.remove_entry(
				&root
					.join(format!("d{}", i % 7))
					.join(format!("file-{i}.bin")),
			);
		}

		let page_size = 25;
		let all_before = index.files_in_scope(&root, true).unwrap();
		let first_page: Vec<PathBuf> = all_before.iter().take(page_size).cloned().collect();
		let cursor = first_page.last().cloned().unwrap();
		let expected_rest: Vec<PathBuf> = all_before
			.iter()
			.filter(|path| **path > cursor)
			.cloned()
			.collect();
		let bucket = root.join("d1");
		let mut bucket_before = index.list_directory(&bucket).unwrap();
		bucket_before.sort();

		assert!(index.vacant_slots() > 0);
		index.compact().unwrap();
		assert_eq!(index.vacant_slots(), 0);
		assert_consistent(&index);

		let all_after = index.files_in_scope(&root, true).unwrap();
		assert_eq!(all_after, all_before);
		let rest_after: Vec<PathBuf> = all_after
			.iter()
			.filter(|path| **path > cursor)
			.cloned()
			.collect();
		assert_eq!(rest_after, expected_rest);
		assert_eq!(rest_after.len(), all_before.len() - page_size);

		let mut bucket_after = index.list_directory(&bucket).unwrap();
		bucket_after.sort();
		assert_eq!(bucket_after, bucket_before);
		assert_eq!(
			index.find_by_prefix("file-1").len(),
			index.find_by_prefix("file-1").len()
		);
		for path in &all_after {
			assert!(index.get_entry_ref(path).is_some(), "{}", path.display());
		}
	}

	/// A directory rename arrives as `remove_entry(from)` then `add_entry(to)`.
	/// The old children must leave with their parent: with slot reuse, a
	/// child still holding the vacated parent's slot would find an unrelated
	/// node there and roll its bytes up the wrong chain.
	#[test]
	fn removing_a_directory_takes_its_children_so_no_parent_link_dangles() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let from = root.join("old");
		let child = from.join("inner").join("leaf.bin");
		let to = root.join("new");
		let sibling = root.join("sibling");
		let sibling_file = sibling.join("other.bin");

		index
			.add_entry(
				child.clone(),
				Uuid::now_v7(),
				meta(&child, EntryKind::File, 100),
			)
			.unwrap();
		let dir_uuid = index.get_or_assign_uuid(&from);

		assert!(index.remove_entry(&from));
		index
			.add_entry(to.clone(), dir_uuid, meta(&to, EntryKind::Directory, 0))
			.unwrap();
		index
			.add_entry(
				sibling_file.clone(),
				Uuid::now_v7(),
				meta(&sibling_file, EntryKind::File, 7),
			)
			.unwrap();

		assert!(index.get_entry_ref(&child).is_none());
		assert!(index.get_entry_ref(&from.join("inner")).is_none());
		assert!(index.find_by_name("leaf.bin").is_empty());
		assert_eq!(index.get_entry_uuid(&to), Some(dir_uuid));
		assert_eq!(index.subtree_size(&to), Some(0));
		assert_eq!(index.subtree_size(&sibling), Some(7));
		assert_eq!(index.subtree_size(&root), Some(7));
		assert_consistent(&index);

		// Re-observing the old leaf path builds it fresh under its own chain.
		index
			.add_entry(
				child.clone(),
				Uuid::now_v7(),
				meta(&child, EntryKind::File, 100),
			)
			.unwrap();
		assert_eq!(index.subtree_size(&from), Some(100));
		assert_eq!(index.subtree_size(&to), Some(0));
		assert_eq!(index.subtree_size(&sibling), Some(7));
		assert_eq!(index.subtree_size(&root), Some(107));
		index.recompute_rollups();
		assert_eq!(index.subtree_size(&root), Some(107));
		assert_consistent(&index);
	}

	/// A renamed directory keeps its subtree, its identities and its rollups
	/// under the new name, as the store's `rename_tree` does.
	#[test]
	fn renaming_a_directory_carries_its_subtree() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let from = root.join("old");
		let deep = from.join("inner").join("leaf.bin");
		let shallow = from.join("top.bin");
		let to = root.join("archive").join("new");

		index
			.add_entry(
				deep.clone(),
				Uuid::now_v7(),
				meta(&deep, EntryKind::File, 100),
			)
			.unwrap();
		index
			.add_entry(
				shallow.clone(),
				Uuid::now_v7(),
				meta(&shallow, EntryKind::File, 5),
			)
			.unwrap();
		let dir_uuid = index.get_or_assign_uuid(&from);
		let leaf_uuid = index.get_entry_uuid(&deep).unwrap();
		let slots = index.len();

		let kept = index
			.rename(&from, meta(&to, EntryKind::Directory, 0), Uuid::now_v7())
			.unwrap();
		assert_eq!(kept, dir_uuid);

		assert!(index.get_entry_ref(&from).is_none());
		assert!(index.get_entry_ref(&deep).is_none());
		let moved_leaf = to.join("inner").join("leaf.bin");
		assert_eq!(index.get_entry_uuid(&moved_leaf), Some(leaf_uuid));
		assert_eq!(index.get_entry_uuid(&to), Some(dir_uuid));
		assert_eq!(index.get_entry_ref(&moved_leaf).unwrap().size, 100);
		let mut listed = index.list_directory(&to).unwrap();
		listed.sort();
		assert_eq!(listed, vec![to.join("inner"), to.join("top.bin")]);
		// Paths leave the arena as strings too, so compare spellings and not
		// just components: a trailing separator would fail every open.
		assert_eq!(
			index.find_by_name("new")[0].as_os_str(),
			to.as_os_str(),
			"the moved entry's own path is spelled exactly as given"
		);
		assert_eq!(
			index.find_by_name("leaf.bin")[0].as_os_str(),
			moved_leaf.as_os_str()
		);
		assert_eq!(index.find_by_name("leaf.bin"), vec![moved_leaf.clone()]);
		assert_eq!(index.find_by_name("new"), vec![to.clone()]);
		assert!(index.find_by_name("old").is_empty());
		assert_eq!(index.subtree_size(&to), Some(105));
		assert_eq!(index.subtree_file_count(&root.join("archive")), Some(2));
		assert_eq!(index.subtree_size(&root), Some(105));
		assert_eq!(index.len(), slots + 1, "only the new ancestor is allocated");
		assert_consistent(&index);

		index.recompute_rollups();
		assert_eq!(index.subtree_size(&root), Some(105));

		let dir = tempfile::tempdir().unwrap();
		let snapshot = dir.path().join("arena.snapshot");
		index
			.save_snapshot(&snapshot, Uuid::now_v7(), &root)
			.unwrap();
		let (restored, _) = Arena::load_snapshot(&snapshot).unwrap().expect("snapshot");
		assert_eq!(restored.get_entry_uuid(&moved_leaf), Some(leaf_uuid));
		assert_eq!(restored.subtree_size(&to), Some(105));
		assert_consistent(&restored);
	}

	/// A file rename keeps identity, adopts the new name's kind, and replaces
	/// whatever the destination held.
	#[test]
	fn renaming_a_file_rebinds_name_kind_and_destination() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let from = root.join("clip.txt");
		let to = root.join("clip.mp4");
		let uuid = Uuid::now_v7();
		index
			.add_entry(from.clone(), uuid, meta(&from, EntryKind::File, 10))
			.unwrap();
		index
			.add_entry(to.clone(), Uuid::now_v7(), meta(&to, EntryKind::File, 3))
			.unwrap();
		assert_eq!(index.subtree_size(&root), Some(13));

		let kept = index
			.rename(&from, meta(&to, EntryKind::File, 12), Uuid::now_v7())
			.unwrap();
		assert_eq!(kept, uuid);
		assert_eq!(index.get_entry_uuid(&to), Some(uuid));
		assert!(index.get_entry_ref(&from).is_none());
		assert_eq!(index.subtree_size(&root), Some(12));
		assert_eq!(index.get_content_kind(&to), ContentKind::Video);
		assert_eq!(index.find_by_name("clip.mp4"), vec![to.clone()]);
		assert_eq!(
			index.find_by_name("clip.mp4")[0].as_os_str(),
			to.as_os_str()
		);
		assert!(index.find_by_name("clip.txt").is_empty());
		assert_consistent(&index);
	}

	#[test]
	fn removal_clears_the_name_registry() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let file = root.join("only.bin");

		index
			.add_entry(
				file.clone(),
				Uuid::now_v7(),
				meta(&file, EntryKind::File, 1),
			)
			.unwrap();
		assert_eq!(index.find_by_name("only.bin").len(), 1);

		index.remove_entry(&file);
		assert!(
			index.find_by_name("only.bin").is_empty(),
			"a deleted name must not survive in the search registry, which is serialized into the snapshot"
		);
	}

	/// Search lowercases its query before hitting the registry, so a name with
	/// uppercase letters is only findable if registry keys are folded too. The
	/// arena node keeps the original casing for display.
	#[test]
	fn mixed_case_names_match_lowercase_queries() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let file = root.join("Dangerous Woman.mp3");

		index
			.add_entry(
				file.clone(),
				Uuid::now_v7(),
				meta(&file, EntryKind::File, 1),
			)
			.unwrap();

		assert_eq!(
			index.find_by_name("dangerous woman.mp3"),
			vec![file.clone()]
		);
		assert_eq!(index.find_by_prefix("danger"), vec![file.clone()]);
		assert_eq!(index.find_containing("woman"), vec![file.clone()]);

		// The reported path preserves the on-disk casing.
		assert!(index.get_entry(&file).is_some());

		index.remove_entry(&file);
		assert!(
			index.find_by_prefix("danger").is_empty(),
			"removal folds the name the same way insertion did"
		);
	}

	#[test]
	fn mutations_mark_the_index_dirty() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		let before = root.join("before.bin");
		let after = root.join("after.bin");

		index
			.add_entry(
				before.clone(),
				Uuid::now_v7(),
				meta(&before, EntryKind::File, 10),
			)
			.unwrap();
		index.clear_dirty();
		assert!(!index.is_dirty());

		// A rename leaves the entry count identical, which is why count cannot
		// stand in for "something changed".
		let uuid = index.get_entry_uuid(&before).unwrap();
		index.remove_entry(&before);
		index
			.add_entry(after.clone(), uuid, meta(&after, EntryKind::File, 10))
			.unwrap();

		assert!(index.is_dirty(), "a rename has to reach the snapshot");
	}

	#[test]
	fn recompute_matches_incremental() {
		let mut index = Arena::new().unwrap();
		let root = PathBuf::from("/vol");
		for i in 0..20u64 {
			let p = root.join(format!("d{}", i % 4)).join(format!("f{i}.bin"));
			index
				.add_entry(p.clone(), Uuid::now_v7(), meta(&p, EntryKind::File, i + 1))
				.unwrap();
		}
		let incremental = index.subtree_size(&root);
		index.recompute_rollups();
		assert_eq!(index.subtree_size(&root), incremental);
		assert_eq!(index.total_file_bytes(), (1..=20).sum::<u64>());
	}
}
