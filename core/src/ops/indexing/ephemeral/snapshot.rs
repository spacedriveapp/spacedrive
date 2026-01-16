//! Snapshot persistence for ephemeral indexes
//!
//! Saves ephemeral indexes to disk for fast restoration between sessions.
//! Instead of re-indexing millions of files every time (10+ minutes), indexes
//! load from snapshots in 1-2 seconds.
//!
//! ## Format
//!
//! Snapshots use zero-copy binary serialization (postcard) with zstd compression:
//! - **Serialization**: postcard (no schema needed, just derives)
//! - **Compression**: zstd level 6 with multithreading
//! - **Typical size**: ~50-100MB for 1M+ files (70-80% compression)
//!
//! ## Atomic Writes
//!
//! Files are written to `.tmp` first, then atomically renamed to prevent corruption.

use super::{EntryId, EphemeralIndex, NameCache, NameRegistry};
use crate::domain::ContentKind;
use crate::ops::indexing::state::IndexerStats;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	fs::{self, File},
	io::{BufReader, BufWriter},
	path::{Path, PathBuf},
	sync::Arc,
	thread::available_parallelism,
	time::Instant,
};
use uuid::Uuid;

/// Current snapshot format version
const SNAPSHOT_VERSION: u32 = 1;

/// Serializable snapshot of an ephemeral index
#[derive(Serialize, Deserialize)]
pub struct IndexSnapshot {
	/// Format version for compatibility checking
	pub version: u32,
	/// Root path that was indexed
	pub root_path: PathBuf,
	/// When the snapshot was created
	pub created_at_secs: u64,
	/// Path to node ID mappings
	pub path_index: HashMap<PathBuf, EntryId>,
	/// File UUIDs
	pub entry_uuids: HashMap<PathBuf, Uuid>,
	/// Content kind cache
	pub content_kinds: HashMap<PathBuf, ContentKind>,
	/// Indexer statistics
	pub stats: IndexerStats,
	/// Name cache (string interning pool)
	pub name_cache_strings: Vec<String>,
	/// Name registry (name → entry ID mappings)
	pub name_registry_map: Vec<(String, Vec<EntryId>)>,
	/// Arena entries (serialized without pointers)
	pub arena_entries: Vec<(usize, SerializableFileNode)>,
}

/// Serializable version of FileNode without raw pointers
#[derive(Serialize, Deserialize)]
struct SerializableFileNode {
	/// Name string (instead of pointer)
	name: String,
	/// Parent ID
	parent: super::types::MaybeEntryId,
	/// Children
	children: smallvec::SmallVec<[EntryId; 0]>,
	/// Metadata
	meta: super::types::PackedMetadata,
}

/// Internal implementation for saving snapshots (called from index.rs)
pub(super) fn save_snapshot_impl(
	index: &super::EphemeralIndex,
	snapshot_path: &Path,
) -> Result<()> {
	let start = Instant::now();

	// Create snapshot directory if needed
	if let Some(parent) = snapshot_path.parent() {
		fs::create_dir_all(parent).context("Failed to create snapshot directory")?;
	}

	// Get snapshot data from index
	let (arena, cache, registry, path_index, entry_uuids, content_kinds, stats) =
		index.snapshot_data();

	// Serialize name cache
	let name_cache_strings: Vec<String> = cache.iter().collect();

	// Serialize name registry
	let name_registry_map = registry.export_map();

	// Serialize arena entries (convert FileNode to SerializableFileNode)
	let arena_entries: Vec<(usize, SerializableFileNode)> = arena
		.iter()
		.map(|(id, node)| {
			(
				id.as_usize(),
				SerializableFileNode {
					name: node.name().to_string(),
					parent: node.parent().into(),
					children: node.children.clone(),
					meta: node.meta,
				},
			)
		})
		.collect();

	let snapshot = IndexSnapshot {
		version: SNAPSHOT_VERSION,
		root_path: PathBuf::new(), // Populated by caller
		created_at_secs: std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_secs(),
		path_index: path_index.clone(),
		entry_uuids: entry_uuids.clone(),
		content_kinds: content_kinds.clone(),
		stats: stats.clone(),
		name_cache_strings,
		name_registry_map,
		arena_entries,
	};

	// Write to temporary file first
	let tmp_path = snapshot_path.with_extension("tmp");
	{
		let file = File::create(&tmp_path).context("Failed to create temporary snapshot file")?;

		// Create zstd encoder with multithreading
		let mut encoder = zstd::Encoder::new(file, 6).context("Failed to create zstd encoder")?;
		encoder
			.multithread(available_parallelism().map(|x| x.get() as u32).unwrap_or(4))
			.context("Failed to enable zstd multithreading")?;

		let writer = BufWriter::new(encoder.auto_finish());

		// Serialize with postcard
		postcard::to_io(&snapshot, writer).context("Failed to serialize snapshot")?;
	}

	// Atomically rename temporary file
	fs::rename(&tmp_path, snapshot_path).context("Failed to rename snapshot file")?;

	let file_size = fs::metadata(snapshot_path)
		.context("Failed to read snapshot file size")?
		.len();

	tracing::info!(
		"Saved snapshot: {} entries, {} MB, took {:?}",
		arena.len(),
		file_size / 1024 / 1024,
		start.elapsed()
	);

	Ok(())
}

/// Internal implementation for loading snapshots (called from index.rs)
pub(super) fn load_snapshot_impl(snapshot_path: &Path) -> Result<Option<super::EphemeralIndex>> {
	if !snapshot_path.exists() {
		return Ok(None);
	}

	let start = Instant::now();

	// Open and decompress
	let file = File::open(snapshot_path).context("Failed to open snapshot file")?;
	let decoder = zstd::Decoder::new(file).context("Failed to create zstd decoder")?;
	let reader = BufReader::new(decoder);

	// Deserialize with postcard
	let mut buffer = vec![0u8; 4 * 1024];
	let snapshot: IndexSnapshot = postcard::from_io((reader, &mut buffer))
		.context("Failed to deserialize snapshot")?
		.0;

	// Version check
	if snapshot.version != SNAPSHOT_VERSION {
		tracing::warn!(
			"Snapshot version mismatch: expected {}, got {}",
			SNAPSHOT_VERSION,
			snapshot.version
		);
		return Ok(None);
	}

	// Reconstruct index
	let cache = Arc::new(NameCache::new());

	// Rebuild name cache
	for name in &snapshot.name_cache_strings {
		cache.intern(name);
	}

	// Rebuild name registry
	let mut registry = NameRegistry::new();
	for (name, ids) in &snapshot.name_registry_map {
		let interned = cache.intern(name);
		for &id in ids {
			registry.insert(interned, id);
		}
	}

	// Rebuild arena (convert SerializableFileNode back to FileNode)
	let mut arena = super::NodeArena::new()?;
	for (expected_idx, serializable_node) in snapshot.arena_entries {
		// Intern the name and create NameRef
		let interned_name = cache.intern(&serializable_node.name);
		let name_ref = super::types::NameRef::new(interned_name, serializable_node.parent);

		// Reconstruct FileNode
		let mut file_node = super::types::FileNode::new(name_ref, serializable_node.meta);
		file_node.children = serializable_node.children;

		let actual_idx = arena.insert(file_node)?;
		if actual_idx.as_usize() != expected_idx {
			anyhow::bail!(
				"Arena index mismatch: expected {}, got {}",
				expected_idx,
				actual_idx.as_usize()
			);
		}
	}

	// Reconstruct index using constructor
	let index = super::EphemeralIndex::from_snapshot_parts(
		arena,
		cache,
		registry,
		snapshot.path_index,
		snapshot.entry_uuids,
		snapshot.content_kinds,
		snapshot.stats,
	);

	tracing::info!(
		"Loaded snapshot: {} entries, took {:?}",
		index.snapshot_data().0.len(),
		start.elapsed()
	);

	Ok(Some(index))
}

/// Get the snapshot file path for a given root path
///
/// Uses a hash of the canonical path to avoid filesystem-unsafe characters.
pub fn snapshot_path_for(root: &Path, cache_dir: &Path) -> Result<PathBuf> {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};

	// Canonicalize to handle symlinks
	let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

	// Hash the path
	let mut hasher = DefaultHasher::new();
	canonical.hash(&mut hasher);
	let hash = hasher.finish();

	// Create filename
	let filename = format!("{:016x}.snapshot", hash);
	Ok(cache_dir.join(filename))
}

/// Get the ephemeral snapshot cache directory
///
/// Returns `~/Library/Application Support/spacedrive/cache/volume-index/` on macOS,
/// similar paths on other platforms.
pub fn get_snapshot_cache_dir() -> Result<PathBuf> {
	let data_dir = crate::config::default_data_dir().context("Failed to get data directory")?;
	let cache_dir = data_dir.join("cache").join("volume-index");
	fs::create_dir_all(&cache_dir).context("Failed to create snapshot cache directory")?;
	Ok(cache_dir)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_snapshot_path_hash() {
		let cache_dir = PathBuf::from("/tmp/cache");

		// Same path should give same hash
		let path1 = PathBuf::from("/Users/test");
		let snapshot1 = snapshot_path_for(&path1, &cache_dir).unwrap();
		let snapshot2 = snapshot_path_for(&path1, &cache_dir).unwrap();
		assert_eq!(snapshot1, snapshot2);

		// Different paths should give different hashes
		let path2 = PathBuf::from("/Users/other");
		let snapshot3 = snapshot_path_for(&path2, &cache_dir).unwrap();
		assert_ne!(snapshot1, snapshot3);
	}
}
