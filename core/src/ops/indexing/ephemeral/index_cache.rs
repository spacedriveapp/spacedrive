//! Global cache for ephemeral indexes
//!
//! This module provides a thread-safe cache with a SINGLE global ephemeral index.
//! All browsed directories share the same arena and string interning pool,
//! providing efficient memory usage through deduplication.
//!
//! Key benefits of unified index:
//! - String interning shared across all paths (common names like .git, README.md)
//! - Single arena for all entries (~50 bytes per entry vs ~200 with HashMap)
//! - Hierarchical structure preserved for efficient directory listings
//!
//! The cache tracks which paths have been indexed (ready) vs are currently
//! being indexed (in progress).

use crate::ops::indexing::EphemeralIndex;
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

	/// When the cache was created
	created_at: Instant,
}

impl EphemeralIndexCache {
	/// Create a new cache with an empty global index
	pub fn new() -> Self {
		Self {
			index: Arc::new(TokioRwLock::new(EphemeralIndex::new())),
			indexed_paths: RwLock::new(HashSet::new()),
			indexing_in_progress: RwLock::new(HashSet::new()),
			created_at: Instant::now(),
		}
	}

	/// Get the global index if the given path has been indexed
	///
	/// Returns Some(index) if this path's contents are available,
	/// None if the path hasn't been browsed yet.
	pub fn get_for_path(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let indexed = self.indexed_paths.read();
		if indexed.contains(path) {
			Some(self.index.clone())
		} else {
			None
		}
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
	/// Call this after create_for_indexing to remove old children entries.
	/// This prevents ghost entries when files are deleted between index runs.
	pub async fn clear_for_reindex(&self, path: &Path) -> usize {
		let mut index = self.index.write().await;
		index.clear_directory_children(path)
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

	/// Get cache statistics
	pub fn stats(&self) -> EphemeralIndexCacheStats {
		let indexed = self.indexed_paths.read();
		let in_progress = self.indexing_in_progress.read();

		EphemeralIndexCacheStats {
			indexed_paths: indexed.len(),
			indexing_in_progress: in_progress.len(),
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
		// Mark the path as indexed
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
		Self::new()
	}
}

/// Statistics about the ephemeral index cache
#[derive(Debug, Clone)]
pub struct EphemeralIndexCacheStats {
	/// Number of paths that have been indexed
	pub indexed_paths: usize,
	/// Number of paths currently being indexed
	pub indexing_in_progress: usize,
	// Legacy field names for compatibility
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
		let cache = EphemeralIndexCache::new();

		// Initially no paths are indexed
		assert!(cache.is_empty());
		assert!(cache.get_for_path(Path::new("/test")).is_none());
	}

	#[test]
	fn test_indexing_workflow() {
		let cache = EphemeralIndexCache::new();
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
		let cache = EphemeralIndexCache::new();

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
		let cache = EphemeralIndexCache::new();
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
		let cache = EphemeralIndexCache::new();

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
}
