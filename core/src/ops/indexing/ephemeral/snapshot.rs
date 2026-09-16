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

/// Current snapshot format version. Version 3 carries summarised directories,
/// whose rollups cannot be rebuilt from a tree they have no children in.
/// Version 2 keys snapshots by source id,
/// carries a real root path, and holds one source's partition rather than a
/// dump of a shared global index.
const SNAPSHOT_VERSION: u32 = 3;

/// Metadata read back alongside a restored index, used for staleness checks
/// and for reattaching the snapshot to its source.
#[derive(Debug, Clone)]
pub struct SnapshotMeta {
	pub source_id: Uuid,
	pub root_path: PathBuf,
	pub created_at_secs: u64,
}

/// Serializable snapshot of an ephemeral index
#[derive(Serialize, Deserialize)]
pub struct IndexSnapshot {
	/// Format version for compatibility checking
	pub version: u32,
	/// The source this partition belongs to
	pub source_id: Uuid,
	/// Root path that was indexed
	pub root_path: PathBuf,
	/// When the snapshot was created
	pub created_at_secs: u64,
	/// Path to node ID mappings
	pub path_index: HashMap<PathBuf, EntryId>,
	/// File UUIDs (keyed by EntryId for memory efficiency)
	pub entry_uuids: HashMap<EntryId, Uuid>,
	/// Content kind cache (keyed by EntryId for memory efficiency)
	pub content_kinds: HashMap<EntryId, ContentKind>,
	/// Indexer statistics
	pub stats: IndexerStats,
	/// Name cache (string interning pool)
	pub name_cache_strings: Vec<String>,
	/// Name registry (name → entry ID mappings)
	pub name_registry_map: Vec<(String, Vec<EntryId>)>,
	/// Arena entries (serialized without pointers)
	pub arena_entries: Vec<(usize, SerializableFileNode)>,
	/// Directories counted but not kept, and the totals standing in for them.
	/// Every other rollup is recomputed from the tree on load; these have no
	/// tree beneath them to recompute from.
	pub stubs: Vec<(EntryId, u64, u32)>,
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
	source_id: Uuid,
	root_path: &Path,
) -> Result<()> {
	let start = Instant::now();

	// Create snapshot directory if needed
	if let Some(parent) = snapshot_path.parent() {
		fs::create_dir_all(parent).context("Failed to create snapshot directory")?;
	}

	// Get snapshot data from index
	let (arena, cache, registry, path_index, entry_uuids, content_kinds, stats, stubs) =
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
		source_id,
		root_path: root_path.to_path_buf(),
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
		stubs: stubs
			.iter()
			.map(|(&id, rollup)| (id, rollup.bytes, rollup.files))
			.collect(),
	};

	// Write to a uniquely named temporary file first. The name must be unique
	// per writer: concurrent saves sharing one tmp path interleave their
	// writes, and the atomic rename then publishes a corrupt snapshot.
	let tmp_path = snapshot_path.with_extension(format!("tmp.{}", Uuid::now_v7().simple()));
	{
		let file = File::create(&tmp_path).context("Failed to create temporary snapshot file")?;

		// Create zstd encoder with multithreading
		let mut encoder = zstd::Encoder::new(file, 6).context("Failed to create zstd encoder")?;
		encoder
			.multithread(available_parallelism().map(|x| x.get() as u32).unwrap_or(4))
			.context("Failed to enable zstd multithreading")?;

		{
			let mut writer = BufWriter::new(&mut encoder);
			postcard::to_io(&snapshot, &mut writer).context("Failed to serialize snapshot")?;
			use std::io::Write;
			writer.flush().context("Failed to flush snapshot")?;
		}

		// The rename below only orders metadata; without syncing the data a
		// reboot can persist the rename while the file's blocks are still in
		// the page cache, leaving a truncated snapshot at the final name.
		let file = encoder.finish().context("Failed to finish zstd stream")?;
		file.sync_all().context("Failed to sync snapshot data")?;
	}

	// Atomically rename temporary file
	fs::rename(&tmp_path, snapshot_path).context("Failed to rename snapshot file")?;

	// Sync the directory so the rename itself survives a crash.
	if let Some(parent) = snapshot_path.parent() {
		if let Ok(dir) = File::open(parent) {
			let _ = dir.sync_all();
		}
	}

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
pub(super) fn load_snapshot_impl(
	snapshot_path: &Path,
) -> Result<Option<(super::EphemeralIndex, SnapshotMeta)>> {
	if !snapshot_path.exists() {
		return Ok(None);
	}

	let start = Instant::now();

	// Open and decompress into memory.
	//
	// `postcard::from_io` needs its scratch buffer to hold whatever element it
	// is currently decoding, and this read used a fixed 4 KiB. That is enough
	// for a handful of entries and not for a real index, so every snapshot past
	// a few hundred entries failed to deserialize, was deleted as unreadable,
	// and left the source with nothing to restore. The next browse then wrote
	// its own handful of entries in its place.
	//
	// Reading the whole stream first removes the size limit entirely. The cost
	// is the decompressed snapshot held once, which is the same order as the
	// arena it is about to become.
	let file = File::open(snapshot_path).context("Failed to open snapshot file")?;
	let decoder = zstd::Decoder::new(file).context("Failed to create zstd decoder")?;
	let mut reader = BufReader::new(decoder);
	let mut bytes = Vec::new();
	use std::io::Read;
	if let Err(err) = reader.read_to_end(&mut bytes) {
		tracing::warn!(
			"Unreadable snapshot {} ({err}); removing",
			snapshot_path.display()
		);
		let _ = fs::remove_file(snapshot_path);
		return Ok(None);
	}

	// A snapshot from an older format version fails either here (layout
	// changed) or at the version check below; both cases remove the file so the
	// source reindexes cleanly instead of retrying a dead artifact on every
	// launch.
	let snapshot: IndexSnapshot = match postcard::from_bytes(&bytes) {
		Ok(snapshot) => snapshot,
		Err(err) => {
			tracing::warn!(
				"Unreadable snapshot {} ({err}); removing",
				snapshot_path.display()
			);
			let _ = fs::remove_file(snapshot_path);
			return Ok(None);
		}
	};

	if snapshot.version != SNAPSHOT_VERSION {
		tracing::warn!(
			"Snapshot version mismatch: expected {}, got {}; removing {}",
			SNAPSHOT_VERSION,
			snapshot.version,
			snapshot_path.display()
		);
		let _ = fs::remove_file(snapshot_path);
		return Ok(None);
	}

	let meta = SnapshotMeta {
		source_id: snapshot.source_id,
		root_path: snapshot.root_path.clone(),
		created_at_secs: snapshot.created_at_secs,
	};

	// Reconstruct index
	let cache = Arc::new(NameCache::new());

	// Rebuild name cache
	for name in &snapshot.name_cache_strings {
		cache.intern(name);
	}

	// Rebuild name registry. Keys are folded on insert so snapshots written
	// before registry keys were lowercased come back searchable.
	let mut registry = NameRegistry::new();
	for (name, ids) in &snapshot.name_registry_map {
		let interned = cache.intern(&name.to_lowercase());
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
		snapshot
			.stubs
			.into_iter()
			.map(|(id, bytes, files)| (id, super::types::Rollup { bytes, files }))
			.collect(),
	);

	tracing::info!(
		"Loaded snapshot: {} entries, took {:?}",
		index.snapshot_data().0.len(),
		start.elapsed()
	);

	Ok(Some((index, meta)))
}
