//! Global cache for ephemeral indexes
//!
//! This module provides a thread-safe cache for storing ephemeral indexes
//! by their root path. This allows directory listing queries to reuse
//! existing indexes instead of spawning new indexer jobs.
//!
//! The cache is permanent in memory (no TTL or expiration). Entries persist
//! until the daemon restarts or they are explicitly removed. This ensures
//! UUIDs from ephemeral indexing can be preserved when regular indexing is enabled.

use crate::ops::indexing::EphemeralIndex;
use parking_lot::RwLock;
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
	time::Instant,
};
use tokio::sync::RwLock as TokioRwLock;

/// Cache entry wrapping an ephemeral index with metadata
struct CacheEntry {
	/// The ephemeral index
	index: Arc<TokioRwLock<EphemeralIndex>>,
	/// When this entry was created
	created_at: Instant,
	/// Whether an indexer job is currently running for this path
	indexing_in_progress: bool,
}

impl CacheEntry {
	fn new(index: Arc<TokioRwLock<EphemeralIndex>>) -> Self {
		Self {
			index,
			created_at: Instant::now(),
			indexing_in_progress: false,
		}
	}
}

/// Global cache for ephemeral indexes
///
/// Stores ephemeral indexes by their root path for reuse across queries.
/// Indexes persist in memory until the daemon restarts or they are explicitly removed.
pub struct EphemeralIndexCache {
	/// Map of root path to cache entry
	entries: RwLock<HashMap<PathBuf, CacheEntry>>,
}

impl EphemeralIndexCache {
	/// Create a new cache
	pub fn new() -> Self {
		Self {
			entries: RwLock::new(HashMap::new()),
		}
	}

	/// Get an existing index for a path, or None if not cached
	pub fn get(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let entries = self.entries.read();
		entries.get(path).map(|entry| entry.index.clone())
	}

	/// Get an existing index for a path (exact match only)
	///
	/// Returns the index if an index exists for this exact path.
	///
	/// Note: We only use exact matches because ephemeral indexing uses
	/// IndexScope::Current (single level), so an ancestor index doesn't
	/// contain the contents of subdirectories.
	pub fn get_for_path(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		self.get(path)
	}

	/// Check if indexing is in progress for a path
	pub fn is_indexing(&self, path: &Path) -> bool {
		let entries = self.entries.read();
		entries
			.get(path)
			.map(|e| e.indexing_in_progress)
			.unwrap_or(false)
	}

	/// Insert or update an index in the cache
	pub fn insert(&self, path: PathBuf, index: Arc<TokioRwLock<EphemeralIndex>>) {
		let mut entries = self.entries.write();
		entries.insert(path, CacheEntry::new(index));
	}

	/// Create a new index for a path and mark it as indexing in progress
	///
	/// Returns the index to be used by the indexer job.
	pub fn create_for_indexing(&self, path: PathBuf) -> Arc<TokioRwLock<EphemeralIndex>> {
		let mut entries = self.entries.write();

		// Check if entry already exists
		if let Some(entry) = entries.get_mut(&path) {
			entry.indexing_in_progress = true;
			return entry.index.clone();
		}

		// Create new entry
		let index = Arc::new(TokioRwLock::new(EphemeralIndex::new(path.clone())));
		let mut entry = CacheEntry::new(index.clone());
		entry.indexing_in_progress = true;
		entries.insert(path, entry);
		index
	}

	/// Mark indexing as complete for a path
	pub fn mark_indexing_complete(&self, path: &Path) {
		let mut entries = self.entries.write();
		if let Some(entry) = entries.get_mut(path) {
			entry.indexing_in_progress = false;
		}
	}

	/// Remove an index from the cache
	pub fn remove(&self, path: &Path) {
		let mut entries = self.entries.write();
		entries.remove(path);
	}

	/// Get the number of cached indexes
	pub fn len(&self) -> usize {
		self.entries.read().len()
	}

	/// Check if the cache is empty
	pub fn is_empty(&self) -> bool {
		self.entries.read().is_empty()
	}

	/// Get all cached root paths
	pub fn cached_paths(&self) -> Vec<PathBuf> {
		self.entries.read().keys().cloned().collect()
	}

	/// Get cache statistics
	pub fn stats(&self) -> EphemeralIndexCacheStats {
		let entries = self.entries.read();
		let total_entries = entries.len();
		let indexing_count = entries.values().filter(|e| e.indexing_in_progress).count();

		EphemeralIndexCacheStats {
			total_entries,
			indexing_count,
		}
	}

	/// Get the age of a cached index in seconds
	pub fn get_age(&self, path: &Path) -> Option<f64> {
		let entries = self.entries.read();
		entries
			.get(path)
			.map(|e| e.created_at.elapsed().as_secs_f64())
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
	pub total_entries: usize,
	pub indexing_count: usize,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_insert_and_get() {
		let cache = EphemeralIndexCache::new();
		let path = PathBuf::from("/test/path");
		let index = Arc::new(TokioRwLock::new(EphemeralIndex::new(path.clone())));

		cache.insert(path.clone(), index.clone());

		assert!(cache.get(&path).is_some());
		assert_eq!(cache.len(), 1);
	}

	#[test]
	fn test_get_nonexistent() {
		let cache = EphemeralIndexCache::new();
		assert!(cache.get(Path::new("/nonexistent")).is_none());
	}

	#[test]
	fn test_create_for_indexing() {
		let cache = EphemeralIndexCache::new();
		let path = PathBuf::from("/test/path");

		let _index = cache.create_for_indexing(path.clone());

		assert!(cache.is_indexing(&path));

		cache.mark_indexing_complete(&path);

		assert!(!cache.is_indexing(&path));
	}

	#[test]
	fn test_remove() {
		let cache = EphemeralIndexCache::new();
		let path = PathBuf::from("/test/path");
		let index = Arc::new(TokioRwLock::new(EphemeralIndex::new(path.clone())));

		cache.insert(path.clone(), index);
		assert_eq!(cache.len(), 1);

		cache.remove(&path);
		assert_eq!(cache.len(), 0);
	}

	#[test]
	fn test_get_for_path_exact_match_only() {
		let cache = EphemeralIndexCache::new();
		let root = PathBuf::from("/test");
		let child = PathBuf::from("/test/subdir/file.txt");
		let index = Arc::new(TokioRwLock::new(EphemeralIndex::new(root.clone())));

		cache.insert(root.clone(), index);

		// Should NOT find ancestor index - we only use exact matches
		// because ephemeral indexing is single-level (IndexScope::Current)
		assert!(cache.get_for_path(&child).is_none());

		// Should find exact match
		assert!(cache.get_for_path(&root).is_some());
	}

	#[test]
	fn test_cache_persists() {
		// Test that cache entries persist (no TTL expiration)
		let cache = EphemeralIndexCache::new();
		let path = PathBuf::from("/test/path");
		let index = Arc::new(TokioRwLock::new(EphemeralIndex::new(path.clone())));

		cache.insert(path.clone(), index);

		// Wait a bit
		std::thread::sleep(std::time::Duration::from_millis(100));

		// Should still be available (no expiration)
		assert!(cache.get(&path).is_some());
	}
}
