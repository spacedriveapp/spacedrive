//! Snapshot persistence for arenas
//!
//! Saves each drive's arena to disk for fast restoration between sessions.
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

use super::{Arena, EntryId, NameCache, NameRegistry};
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

/// Serializable snapshot of an arena
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
	index: &super::Arena,
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

/// Move an artifact that will not load to `<name>.corrupt-<unix seconds>`
/// beside it.
///
/// The file is evidence of what went wrong and the only copy of it, so it is
/// kept rather than deleted; moving it clears the slot so the next save lands
/// clean and no launch parses it again. One copy is enough evidence: when a
/// quarantined sibling already exists the slot is deleted instead, since a
/// recurring failure (two builds alternating snapshot versions, saves that
/// keep landing torn) would otherwise retain a full artifact per launch. The
/// oldest copy stays, as the mismatched-root path does. A rename that fails
/// leaves the file where it is, which costs a parse per launch and loses
/// nothing.
fn quarantine(snapshot_path: &Path, reason: &str) {
	let name = snapshot_path
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	let prefix = format!("{name}.corrupt-");
	let already_retained = snapshot_path
		.parent()
		.and_then(|dir| fs::read_dir(dir).ok())
		.into_iter()
		.flatten()
		.flatten()
		.any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix));
	if already_retained {
		tracing::warn!(
			snapshot = %snapshot_path.display(),
			"snapshot {reason}; an earlier copy is already retained, removing this one"
		);
		let _ = fs::remove_file(snapshot_path);
		return;
	}

	let stamp = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);
	let aside = snapshot_path.with_file_name(format!("{prefix}{stamp}"));
	match fs::rename(snapshot_path, &aside) {
		Ok(()) => tracing::warn!(
			snapshot = %snapshot_path.display(),
			retained = %aside.display(),
			"snapshot {reason}; moved aside for diagnosis"
		),
		Err(err) => tracing::warn!(
			snapshot = %snapshot_path.display(),
			%err,
			"snapshot {reason}; could not move it aside"
		),
	}
}

/// Internal implementation for loading snapshots (called from index.rs)
pub(super) fn load_snapshot_impl(
	snapshot_path: &Path,
) -> Result<Option<(super::Arena, SnapshotMeta)>> {
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
	let compressed_len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
	let decoder = zstd::Decoder::new(file).context("Failed to create zstd decoder")?;

	// Bound the decode. A snapshot's postcard payload sits within a couple
	// orders of magnitude of its compressed file (measured ratios run 3-5x);
	// a stream promising more is corrupt or hostile, and reading it to the
	// end would grow this buffer until the allocator gives out. Peer-fetched
	// artifacts pass through this same load before publication.
	let bound = compressed_len
		.saturating_mul(128)
		.max(64 * 1024 * 1024)
		.saturating_add(1);
	let mut reader = BufReader::new(decoder).take(bound);
	let mut bytes = Vec::new();
	use std::io::Read;
	if let Err(err) = reader.read_to_end(&mut bytes) {
		quarantine(snapshot_path, &format!("unreadable: {err}"));
		return Ok(None);
	}
	if bytes.len() as u64 >= bound {
		quarantine(
			snapshot_path,
			&format!(
				"decodes past {} bytes from a {compressed_len} byte file",
				bound - 1
			),
		);
		return Ok(None);
	}

	// A snapshot from an older format version fails either here (layout
	// changed) or at the version check below; both cases move the file aside
	// so the source reindexes cleanly instead of retrying a dead artifact on
	// every launch.
	let snapshot: IndexSnapshot = match postcard::from_bytes(&bytes) {
		Ok(snapshot) => snapshot,
		Err(err) => {
			quarantine(snapshot_path, &format!("undecodable: {err}"));
			return Ok(None);
		}
	};

	if snapshot.version != SNAPSHOT_VERSION {
		quarantine(
			snapshot_path,
			&format!(
				"version mismatch: expected {SNAPSHOT_VERSION}, got {}",
				snapshot.version
			),
		);
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
	let index = super::Arena::from_snapshot_parts(
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
