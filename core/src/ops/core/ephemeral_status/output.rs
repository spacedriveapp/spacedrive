//! Ephemeral index cache status output types

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Status of the entire ephemeral index cache
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralCacheStatus {
	/// Total number of cached indexes
	pub total_indexes: usize,
	/// Number of indexes currently being populated
	pub indexing_in_progress: usize,
	/// Details for each cached index
	pub indexes: Vec<EphemeralIndexInfo>,
}

/// Information about a single ephemeral index
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralIndexInfo {
	/// Root path this index covers
	pub root_path: PathBuf,
	/// Whether indexing is currently in progress
	pub indexing_in_progress: bool,
	/// Total entries in the arena
	pub total_entries: usize,
	/// Number of entries indexed by path
	pub path_index_count: usize,
	/// Number of unique interned names
	pub unique_names: usize,
	/// Number of interned strings in cache
	pub interned_strings: usize,
	/// Number of content kinds stored
	pub content_kinds: usize,
	/// Estimated memory usage in bytes
	pub memory_bytes: usize,
	/// Age of the index in seconds
	pub age_seconds: f64,
	/// Seconds since last access
	pub idle_seconds: f64,
	/// Indexer job statistics (files/dirs/bytes counted)
	pub job_stats: JobStats,
}

/// Statistics from the indexer job
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobStats {
	/// Number of files indexed
	pub files: u64,
	/// Number of directories indexed
	pub dirs: u64,
	/// Number of symlinks indexed
	pub symlinks: u64,
	/// Total bytes indexed
	pub bytes: u64,
}
