//! # Ephemeral Index Cache
//!
//! Thread-safe wrapper around a single global `EphemeralIndex`. All browsed
//! directories share one arena and string pool, keeping memory at ~50 bytes per
//! entry regardless of how many paths the user navigates. The cache tracks which
//! paths are indexed (queryable), in-progress (being scanned), or watched
//! (receiving live filesystem updates via `MemoryAdapter`).

use super::EphemeralIndex;
use parking_lot::RwLock;
use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
	time::Instant,
};
use tokio::sync::RwLock as TokioRwLock;

/// Global cache with a single unified ephemeral index
///
/// Instead of separate indexes per path, all entries live in one shared index.
/// This maximizes memory efficiency through shared string interning and arena.
pub struct EphemeralIndexCache {
	/// Single global index containing all browsed entries
	index: Arc<TokioRwLock<EphemeralIndex>>,

	/// Paths whose immediate children have been indexed (ready for queries)
	indexed_paths: RwLock<HashSet<PathBuf>>,

	/// Paths currently being indexed
	indexing_in_progress: RwLock<HashSet<PathBuf>>,

	/// Paths registered for filesystem watching (subset of indexed_paths)
	watched_paths: RwLock<HashSet<PathBuf>>,

	/// When the cache was created
	created_at: Instant,
}

impl EphemeralIndexCache {
	/// Create a new cache with an empty global index
	pub fn new() -> std::io::Result<Self> {
		Ok(Self {
			index: Arc::new(TokioRwLock::new(EphemeralIndex::new()?)),
			indexed_paths: RwLock::new(HashSet::new()),
			indexing_in_progress: RwLock::new(HashSet::new()),
			watched_paths: RwLock::new(HashSet::new()),
			created_at: Instant::now(),
		})
	}

	/// Get the global index if the given path has been indexed
	///
	/// Returns Some(index) if this path's contents are available,
	/// None if the path hasn't been browsed yet.
	///
	/// Only returns the index for exact path matches (for directory listing).
	/// For search, use `get_for_search()` which checks parent paths.
	pub fn get_for_path(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let indexed = self.indexed_paths.read();
		if indexed.contains(path) {
			Some(self.index.clone())
		} else {
			None
		}
	}

	/// Get the global index for searching within a path
	///
	/// Checks if the path OR any parent path is indexed, handling symlinks.
	/// Used by ephemeral search to find indexed content under parent directories.
	pub fn get_for_search(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let indexed = self.indexed_paths.read();

		// First check for exact match
		if indexed.contains(path) {
			return Some(self.index.clone());
		}

		// Canonicalize the search path to resolve symlinks (e.g., /Users -> /System/Volumes/Data/Users)
		let canonical_path = path.canonicalize().ok();

		// Check if path or its canonical form is under any indexed parent
		for indexed_path in indexed.iter() {
			// Try with original path
			if path.starts_with(indexed_path) {
				return Some(self.index.clone());
			}

			// Try with canonical path if available
			if let Some(ref canon) = canonical_path {
				if canon.starts_with(indexed_path) {
					return Some(self.index.clone());
				}
			}

			// Also try canonicalizing the indexed path (in case it has symlinks)
			if let Ok(canonical_indexed) = indexed_path.canonicalize() {
				if path.starts_with(&canonical_indexed) {
					return Some(self.index.clone());
				}

				if let Some(ref canon) = canonical_path {
					if canon.starts_with(&canonical_indexed) {
						return Some(self.index.clone());
					}
				}
			}
		}

		None
	}

	/// Get the global index unconditionally (for internal use)
	pub fn get_global_index(&self) -> Arc<TokioRwLock<EphemeralIndex>> {
		self.index.clone()
	}

	/// Check if a path has been fully indexed
	pub fn is_indexed(&self, path: &Path) -> bool {
		self.indexed_paths.read().contains(path)
	}

	/// Check if indexing is in progress for a path
	pub fn is_indexing(&self, path: &Path) -> bool {
		self.indexing_in_progress.read().contains(path)
	}

	/// Try to load a snapshot from disk, falling back to creating a new index
	///
	/// This method should be called before indexing. If a valid snapshot exists,
	/// it loads in 1-2 seconds instead of 10+ minutes of re-indexing.
	pub async fn try_load_snapshot_or_create(&self, path: &Path) -> anyhow::Result<bool> {
		// Check if already indexed
		if self.is_indexed(path) {
			return Ok(true);
		}

		// Try to load from snapshot
		if let Ok(snapshot_cache_dir) = super::snapshot::get_snapshot_cache_dir() {
			if let Ok(snapshot_path) = super::snapshot::snapshot_path_for(path, &snapshot_cache_dir)
			{
				if let Ok(Some(loaded_index)) = EphemeralIndex::load_snapshot(&snapshot_path) {
					// Replace the global index with the loaded one
					let mut index = self.index.write().await;
					*index = loaded_index;
					drop(index);

					// Mark as indexed
					let mut indexed = self.indexed_paths.write();
					indexed.insert(path.to_path_buf());

					tracing::info!("Loaded snapshot for path: {}", path.display());
					return Ok(true);
				}
			}
		}

		Ok(false)
	}

	/// Save the current index to a snapshot file
	pub async fn save_snapshot(&self, path: &Path) -> anyhow::Result<()> {
		if let Ok(snapshot_cache_dir) = super::snapshot::get_snapshot_cache_dir() {
			if let Ok(snapshot_path) = super::snapshot::snapshot_path_for(path, &snapshot_cache_dir)
			{
				let index = self.index.read().await;
				index.save_snapshot(&snapshot_path)?;
				tracing::info!("Saved snapshot for path: {}", path.display());
			}
		}
		Ok(())
	}

	/// Prepare the global index for indexing a new path
	///
	/// Marks the path as indexing-in-progress and returns the global index.
	/// The indexer job should add entries to this shared index.
	///
	/// If the path was previously indexed, clears its children first to
	/// prevent ghost entries from deleted files.
	pub fn create_for_indexing(&self, path: PathBuf) -> Arc<TokioRwLock<EphemeralIndex>> {
		let mut in_progress = self.indexing_in_progress.write();
		let mut indexed = self.indexed_paths.write();

		// If this path was previously indexed, remove it from indexed set
		// The actual clearing of stale entries happens asynchronously via clear_for_reindex
		indexed.remove(&path);
		in_progress.insert(path);

		self.index.clone()
	}

	/// Clear stale entries for a path before re-indexing (async version)
	///
	/// Removes files and unbrowsed subdirectories, preserving subdirectories
	/// that were explicitly navigated to. Verifies preserved directories still
	/// exist on the filesystem and removes deleted ones from tracking.
	pub async fn clear_for_reindex(&self, path: &Path) -> usize {
		let indexed = self.indexed_paths.read().clone();
		let mut index = self.index.write().await;
		let (cleared, deleted_browsed_dirs) = index.clear_directory_children(path, &indexed);

		if !deleted_browsed_dirs.is_empty() {
			let mut indexed_paths = self.indexed_paths.write();
			for deleted_path in deleted_browsed_dirs {
				indexed_paths.remove(&deleted_path);
			}
		}

		cleared
	}

	/// Mark indexing as complete for a path
	///
	/// Moves the path from "in progress" to "indexed" state.
	pub fn mark_indexing_complete(&self, path: &Path) {
		let mut in_progress = self.indexing_in_progress.write();
		let mut indexed = self.indexed_paths.write();

		in_progress.remove(path);
		indexed.insert(path.to_path_buf());
	}

	/// Remove a path from the indexed set (e.g., on invalidation)
	///
	/// Note: This doesn't remove entries from the index itself,
	/// just marks the path as needing re-indexing.
	pub fn invalidate_path(&self, path: &Path) {
		let mut indexed = self.indexed_paths.write();
		indexed.remove(path);
	}

	/// Get the number of indexed paths
	pub fn len(&self) -> usize {
		self.indexed_paths.read().len()
	}

	/// Check if no paths have been indexed
	pub fn is_empty(&self) -> bool {
		self.indexed_paths.read().is_empty()
	}

	/// Get all indexed paths
	pub fn indexed_paths(&self) -> Vec<PathBuf> {
		self.indexed_paths.read().iter().cloned().collect()
	}

	/// Get all paths currently being indexed
	pub fn paths_in_progress(&self) -> Vec<PathBuf> {
		self.indexing_in_progress.read().iter().cloned().collect()
	}

	/// Register a path for filesystem watching.
	///
	/// When registered, the watcher service will monitor this path for changes
	/// and update the ephemeral index via `MemoryAdapter`. The path
	/// must already be indexed.
	pub fn register_for_watching(&self, path: PathBuf) -> bool {
		let indexed = self.indexed_paths.read();
		if !indexed.contains(&path) {
			return false;
		}
		drop(indexed);

		let mut watched = self.watched_paths.write();
		watched.insert(path);
		true
	}

	/// Unregister a path from filesystem watching.
	pub fn unregister_from_watching(&self, path: &Path) {
		let mut watched = self.watched_paths.write();
		watched.remove(path);
	}

	/// Check if a path is registered for watching.
	pub fn is_watched(&self, path: &Path) -> bool {
		self.watched_paths.read().contains(path)
	}

	/// Get all watched paths.
	pub fn watched_paths(&self) -> Vec<PathBuf> {
		self.watched_paths.read().iter().cloned().collect()
	}

	/// Find the watched root path that contains the given path.
	///
	/// If the given path is under a watched directory, returns that directory.
	/// Used by the watcher to route events to the ephemeral handler.
	pub fn find_watched_root(&self, path: &Path) -> Option<PathBuf> {
		let watched = self.watched_paths.read();

		let mut best_match: Option<&PathBuf> = None;
		let mut best_len = 0;

		for watched_path in watched.iter() {
			if path.starts_with(watched_path) {
				let len = watched_path.as_os_str().len();
				if len > best_len {
					best_len = len;
					best_match = Some(watched_path);
				}
			}
		}

		best_match.cloned()
	}

	/// Check if any path in an event batch is under an ephemeral watched path.
	///
	/// Returns the watched root if found.
	pub fn find_watched_root_for_any<'a, I>(&self, paths: I) -> Option<PathBuf>
	where
		I: IntoIterator<Item = &'a Path>,
	{
		for path in paths {
			if let Some(root) = self.find_watched_root(path) {
				return Some(root);
			}
		}
		None
	}

	/// Clear the entire cache (all paths and entries)
	pub async fn clear_all(&self) -> usize {
		let cleared_paths = {
			let mut indexed = self.indexed_paths.write();
			let mut in_progress = self.indexing_in_progress.write();
			let mut watched = self.watched_paths.write();

			let count = indexed.len() + in_progress.len();

			indexed.clear();
			in_progress.clear();
			watched.clear();

			count
		};

		let mut index = self.index.write().await;
		*index = EphemeralIndex::new().expect("Failed to create new ephemeral index");

		cleared_paths
	}

	/// Get cache statistics
	pub fn stats(&self) -> EphemeralIndexCacheStats {
		let indexed = self.indexed_paths.read();
		let in_progress = self.indexing_in_progress.read();
		let watched = self.watched_paths.read();

		EphemeralIndexCacheStats {
			indexed_paths: indexed.len(),
			indexing_in_progress: in_progress.len(),
			watched_paths: watched.len(),
		}
	}

	/// Get how long the cache has existed
	pub fn age(&self) -> std::time::Duration {
		self.created_at.elapsed()
	}

	/// Legacy: Get age for a specific path (returns cache age since all share one index)
	pub fn get_age(&self, _path: &Path) -> Option<f64> {
		Some(self.created_at.elapsed().as_secs_f64())
	}

	// Legacy compatibility methods

	/// Legacy: Get an index by exact path (for backward compatibility)
	#[deprecated(note = "Use get_for_path instead")]
	pub fn get(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		self.get_for_path(path)
	}

	/// Legacy: Get all cached paths (returns indexed paths)
	#[deprecated(note = "Use indexed_paths instead")]
	pub fn cached_paths(&self) -> Vec<PathBuf> {
		self.indexed_paths()
	}

	/// Legacy: Insert (no-op, entries are added directly to global index)
	#[deprecated(note = "Entries should be added directly to the global index")]
	pub fn insert(&self, path: PathBuf, _index: Arc<TokioRwLock<EphemeralIndex>>) {
		let mut indexed = self.indexed_paths.write();
		indexed.insert(path);
	}

	/// Legacy: Remove (just invalidates the path)
	#[deprecated(note = "Use invalidate_path instead")]
	pub fn remove(&self, path: &Path) {
		self.invalidate_path(path);
	}
}

impl Default for EphemeralIndexCache {
	fn default() -> Self {
		Self::new().expect("Failed to create default EphemeralIndexCache")
	}
}

/// Statistics about the ephemeral index cache
#[derive(Debug, Clone)]
pub struct EphemeralIndexCacheStats {
	/// Number of paths that have been indexed
	pub indexed_paths: usize,
	/// Number of paths currently being indexed
	pub indexing_in_progress: usize,
	/// Number of paths registered for filesystem watching
	pub watched_paths: usize,
}

impl EphemeralIndexCacheStats {
	/// Legacy: total_entries now means indexed_paths
	pub fn total_entries(&self) -> usize {
		self.indexed_paths
	}

	/// Legacy: indexing_count now means indexing_in_progress
	pub fn indexing_count(&self) -> usize {
		self.indexing_in_progress
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_single_global_index() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");

		// Initially no paths are indexed
		assert!(cache.is_empty());
		assert!(cache.get_for_path(Path::new("/test")).is_none());
	}

	#[test]
	fn test_indexing_workflow() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");
		let path = PathBuf::from("/test/path");

		// Start indexing
		let _index = cache.create_for_indexing(path.clone());
		assert!(cache.is_indexing(&path));
		assert!(!cache.is_indexed(&path));

		// Complete indexing
		cache.mark_indexing_complete(&path);
		assert!(!cache.is_indexing(&path));
		assert!(cache.is_indexed(&path));

		// Now get_for_path returns the index
		assert!(cache.get_for_path(&path).is_some());
	}

	#[test]
	fn test_shared_index_across_paths() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");

		let path1 = PathBuf::from("/test/path1");
		let path2 = PathBuf::from("/test/path2");

		// Start indexing both paths
		let index1 = cache.create_for_indexing(path1.clone());
		let index2 = cache.create_for_indexing(path2.clone());

		// They should be the same index
		assert!(Arc::ptr_eq(&index1, &index2));

		// Complete both
		cache.mark_indexing_complete(&path1);
		cache.mark_indexing_complete(&path2);

		// Both paths now indexed
		assert!(cache.is_indexed(&path1));
		assert!(cache.is_indexed(&path2));
		assert_eq!(cache.len(), 2);
	}

	#[test]
	fn test_invalidate_path() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");
		let path = PathBuf::from("/test/path");

		// Index the path
		let _index = cache.create_for_indexing(path.clone());
		cache.mark_indexing_complete(&path);
		assert!(cache.is_indexed(&path));

		// Invalidate it
		cache.invalidate_path(&path);
		assert!(!cache.is_indexed(&path));

		// get_for_path now returns None
		assert!(cache.get_for_path(&path).is_none());
	}

	#[test]
	fn test_stats() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");

		let path1 = PathBuf::from("/ready");
		let path2 = PathBuf::from("/in_progress");

		// One indexed, one in progress
		let _index = cache.create_for_indexing(path1.clone());
		cache.mark_indexing_complete(&path1);

		let _index = cache.create_for_indexing(path2.clone());

		let stats = cache.stats();
		assert_eq!(stats.indexed_paths, 1);
		assert_eq!(stats.indexing_in_progress, 1);
	}

	#[test]
	fn test_watch_registration() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");
		let path = PathBuf::from("/test/watched");

		// Can't watch a path that's not indexed
		assert!(!cache.register_for_watching(path.clone()));
		assert!(!cache.is_watched(&path));

		// Index the path first
		let _index = cache.create_for_indexing(path.clone());
		cache.mark_indexing_complete(&path);

		// Now we can register for watching
		assert!(cache.register_for_watching(path.clone()));
		assert!(cache.is_watched(&path));

		// Stats should reflect watched path
		let stats = cache.stats();
		assert_eq!(stats.watched_paths, 1);

		// Unregister
		cache.unregister_from_watching(&path);
		assert!(!cache.is_watched(&path));
	}

	#[test]
	fn test_find_watched_root() {
		let cache = EphemeralIndexCache::new().expect("failed to create cache");

		let root = PathBuf::from("/mnt/nas");
		let child = PathBuf::from("/mnt/nas/documents/report.pdf");

		// Index and watch the root
		let _index = cache.create_for_indexing(root.clone());
		cache.mark_indexing_complete(&root);
		cache.register_for_watching(root.clone());

		// Child path should find the watched root
		assert_eq!(cache.find_watched_root(&child), Some(root.clone()));

		// Unrelated path should not find a root
		assert_eq!(cache.find_watched_root(Path::new("/other/path")), None);
	}
}
