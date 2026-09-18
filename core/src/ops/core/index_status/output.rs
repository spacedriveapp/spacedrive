//! Volume index status output types

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// A registered source and its live state
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexSourceInfo {
	pub id: uuid::Uuid,
	pub root: PathBuf,
	/// The volume this source sits on, when it sits on one Spacedrive tracks.
	pub volume_uuid: Option<uuid::Uuid>,
	/// The root exists on disk right now
	pub attached: bool,
	/// A snapshot restore has populated this source's index this session
	pub restored: bool,
	pub last_seen_secs: u64,
	/// Entry count at last snapshot — present without restoring the source
	pub entry_count: Option<u64>,
	/// Total file bytes at last snapshot
	pub total_bytes: Option<u64>,
	/// The source's directory in the daemon's per-source layout
	pub directory: Option<PathBuf>,
	/// The source's thumbnail cache file within that directory
	pub thumbs_path: Option<PathBuf>,
}

/// Status of the volume index
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexStatus {
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
	/// Roots armed for filesystem watching.
	///
	/// Separate from `indexed_paths` because the two come apart: an index
	/// restored from a snapshot is browsable without anything watching it, and
	/// that reads from the outside exactly like a watcher that is running and a
	/// UI that never updates.
	#[serde(default)]
	pub watched_paths: Vec<PathBuf>,
	/// Registered sources (volumes, drives, explicit roots) with their
	/// attachment state — detached sources remain browsable from snapshots
	#[serde(default)]
	pub sources: Vec<IndexSourceInfo>,
}

/// Statistics across every partition of the volume index
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
	/// Number of UUIDs generated (lazy assignment)
	pub uuid_count: usize,
	/// Estimated memory usage in bytes
	pub memory_bytes: usize,
	/// Total size of all indexed files in bytes
	pub total_file_bytes: u64,
	/// Age of the cache in seconds
	pub age_seconds: f64,
	/// Seconds since last access
	pub idle_seconds: f64,
	/// Detailed memory breakdown (optional, expensive to compute)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub memory_breakdown: Option<MemoryBreakdownStats>,
}

/// Detailed breakdown of memory usage by component
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MemoryBreakdownStats {
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

/// Information about an indexed path
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexedPathInfo {
	/// The directory path that was indexed
	pub path: PathBuf,
	/// Number of direct children in this directory
	pub child_count: usize,
}

/// Output from resetting the volume index
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexResetOutput {
	/// Number of paths that were cleared from the index
	pub cleared_paths: usize,
	/// Message describing the result
	pub message: String,
}
