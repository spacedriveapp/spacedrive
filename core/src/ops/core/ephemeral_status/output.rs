//! Ephemeral index cache status output types

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Status of the unified ephemeral index cache
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralCacheStatus {
	/// Number of paths that have been indexed
	pub indexed_paths_count: usize,
	/// Number of paths currently being indexed
	pub indexing_in_progress_count: usize,
	/// Unified index statistics (shared arena and string interning)
	pub index_stats: UnifiedIndexStats,
	/// List of indexed paths (directories whose contents are ready)
	pub indexed_paths: Vec<IndexedPathInfo>,
	/// List of paths currently being indexed
	pub paths_in_progress: Vec<PathBuf>,

	// Legacy fields for backward compatibility
	#[serde(skip_serializing_if = "Option::is_none")]
	pub total_indexes: Option<usize>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub indexing_in_progress: Option<usize>,
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub indexes: Vec<EphemeralIndexInfo>,
}

/// Statistics for the unified ephemeral index
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UnifiedIndexStats {
	/// Total entries in the shared arena
	pub total_entries: usize,
	/// Number of entries indexed by path
	pub path_index_count: usize,
	/// Number of unique interned names (shared across all paths)
	pub unique_names: usize,
	/// Number of interned strings in shared cache
	pub interned_strings: usize,
	/// Number of content kinds stored
	pub content_kinds: usize,
	/// Estimated memory usage in bytes
	pub memory_bytes: usize,
	/// Age of the cache in seconds
	pub age_seconds: f64,
	/// Seconds since last access
	pub idle_seconds: f64,
}

/// Information about an indexed path
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexedPathInfo {
	/// The directory path that was indexed
	pub path: PathBuf,
	/// Number of direct children in this directory
	pub child_count: usize,
}

/// Legacy: Information about a single ephemeral index (for backward compatibility)
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

/// Output from resetting the ephemeral cache
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralCacheResetOutput {
	/// Number of paths that were cleared from the cache
	pub cleared_paths: usize,
	/// Message describing the result
	pub message: String,
}
