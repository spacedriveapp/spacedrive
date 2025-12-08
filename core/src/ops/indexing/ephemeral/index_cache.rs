//! Global cache for ephemeral indexes
//!
//! This module provides a thread-safe cache for storing ephemeral indexes
//! by their root path. This allows directory listing queries to reuse
//! existing indexes instead of spawning new indexer jobs.

use crate::ops::indexing::EphemeralIndex;
use parking_lot::RwLock;
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::sync::RwLock as TokioRwLock;

/// Default TTL for ephemeral indexes (5 minutes)
const DEFAULT_TTL: Duration = Duration::from_secs(5 * 60);

/// Maximum idle time before an index is considered stale (2 minutes)
const MAX_IDLE_TIME: Duration = Duration::from_secs(2 * 60);

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

	fn is_stale(&self, ttl: Duration) -> bool {
		self.created_at.elapsed() > ttl
	}
}

/// Global cache for ephemeral indexes
///
/// Stores ephemeral indexes by their root path for reuse across queries.
/// Indexes are automatically evicted based on TTL and idle time.
pub struct EphemeralIndexCache {
	/// Map of root path to cache entry
	entries: RwLock<HashMap<PathBuf, CacheEntry>>,
	/// Time-to-live for cache entries
	ttl: Duration,
}

impl EphemeralIndexCache {
	/// Create a new cache with default TTL
	pub fn new() -> Self {
		Self {
			entries: RwLock::new(HashMap::new()),
			ttl: DEFAULT_TTL,
		}
	}

	/// Create a new cache with custom TTL
	pub fn with_ttl(ttl: Duration) -> Self {
		Self {
			entries: RwLock::new(HashMap::new()),
			ttl,
		}
	}

	/// Get an existing index for a path, or None if not cached or stale
	///
	/// Also checks if the index is still being populated (indexing in progress).
	pub fn get(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let entries = self.entries.read();
		if let Some(entry) = entries.get(path) {
			// Check if stale
			if entry.is_stale(self.ttl) {
				return None;
			}
			Some(entry.index.clone())
		} else {
			None
		}
	}

	/// Get an existing index for a path (exact match only)
	///
	/// Returns the index if:
	/// 1. An index exists for this exact path
	/// 2. The index is not stale
	///
	/// Note: We only use exact matches because ephemeral indexing uses
	/// IndexScope::Current (single level), so an ancestor index doesn't
	/// contain the contents of subdirectories.
	pub fn get_for_path(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let entries = self.entries.read();

		// Only exact match - ancestor indexes don't contain subdirectory contents
		// because ephemeral indexing uses IndexScope::Current (single level)
		if let Some(entry) = entries.get(path) {
			if !entry.is_stale(self.ttl) {
				return Some(entry.index.clone());
			}
		}

		None
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
	///
	/// This also refreshes the entry's `created_at` timestamp so it's no longer
	/// considered stale. This is important because `create_for_indexing()` may
	/// have reused an existing stale entry, and without this refresh the entry
	/// would remain stale even after being freshly populated.
	pub fn mark_indexing_complete(&self, path: &Path) {
		let mut entries = self.entries.write();
		if let Some(entry) = entries.get_mut(path) {
			entry.indexing_in_progress = false;
			// Reset created_at so the freshly-populated index is no longer stale
			entry.created_at = Instant::now();
		}
	}

	/// Remove an index from the cache
	pub fn remove(&self, path: &Path) {
		let mut entries = self.entries.write();
		entries.remove(path);
	}

	/// Remove stale entries from the cache
	pub fn evict_stale(&self) {
		let mut entries = self.entries.write();
		entries.retain(|_, entry| !entry.is_stale(self.ttl));
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
		let stale_count = entries.values().filter(|e| e.is_stale(self.ttl)).count();

		EphemeralIndexCacheStats {
			total_entries,
			indexing_count,
			stale_count,
		}
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
	pub stale_count: usize,
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

		let index = cache.create_for_indexing(path.clone());

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
	fn test_stale_detection() {
		let cache = EphemeralIndexCache::with_ttl(Duration::from_millis(1));
		let path = PathBuf::from("/test/path");
		let index = Arc::new(TokioRwLock::new(EphemeralIndex::new(path.clone())));

		cache.insert(path.clone(), index);

		// Wait for TTL to expire
		std::thread::sleep(Duration::from_millis(10));

		// Should be stale now
		assert!(cache.get(&path).is_none());
	}
}
