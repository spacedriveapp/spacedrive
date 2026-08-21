//! # Ephemeral Index Cache
//!
//! Partition manager for ephemeral indexes. Each registered **source** (a
//! volume, an external drive, an explicitly indexed root) owns its own
//! `EphemeralIndex` arena and its own snapshot, so restoring or clearing one
//! source never touches another, and a detached drive's index can be restored
//! read-only while the drive is in a drawer.
//!
//! Paths that fall under no registered source land in the **scratch**
//! partition — ad-hoc directory browsing behaves exactly as the old global
//! index did, minus persistence. Scratch never snapshots.
//!
//! Path → partition resolution is longest-root-prefix over registered sources.

use super::sources::{SourceRecord, SourceRegistry};
use super::EphemeralIndex;
use crate::infra::source_dirs::SourceDirs;
use parking_lot::{Mutex, RwLock};
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Instant,
};
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

/// One source's partition: its index plus the per-source path state the old
/// global cache used to track globally.
pub struct SourceSlot {
	/// Source identity; `None` for the scratch partition.
	pub id: Option<Uuid>,
	/// Root at last attach. Scratch has no root.
	root: RwLock<Option<PathBuf>>,
	index: Arc<TokioRwLock<EphemeralIndex>>,
	indexed_paths: RwLock<HashSet<PathBuf>>,
	indexing_in_progress: RwLock<HashSet<PathBuf>>,
	watched_paths: RwLock<HashSet<PathBuf>>,
	/// A detached source's root is not present on disk; its index is served
	/// read-only from a restored snapshot and must never trigger indexing.
	detached: AtomicBool,
	/// Set once a snapshot restore has populated the index this session.
	restored: AtomicBool,
	/// Single restore attempt per session: concurrent callers await the same
	/// load instead of racing three copies of a 100 MB deserialization.
	restore_once: tokio::sync::OnceCell<bool>,
	/// Serializes snapshot saves for this source.
	save_lock: tokio::sync::Mutex<()>,
	/// Entry count at the last completed save; identical partitions skip the
	/// rewrite (a burst of browse jobs otherwise re-saves 100 MB per job).
	last_saved_entries: std::sync::atomic::AtomicU64,
}

impl SourceSlot {
	fn new(id: Option<Uuid>, root: Option<PathBuf>) -> std::io::Result<Arc<Self>> {
		Ok(Arc::new(Self {
			id,
			root: RwLock::new(root),
			index: Arc::new(TokioRwLock::new(EphemeralIndex::new()?)),
			indexed_paths: RwLock::new(HashSet::new()),
			indexing_in_progress: RwLock::new(HashSet::new()),
			watched_paths: RwLock::new(HashSet::new()),
			detached: AtomicBool::new(false),
			restored: AtomicBool::new(false),
			restore_once: tokio::sync::OnceCell::new(),
			save_lock: tokio::sync::Mutex::new(()),
			last_saved_entries: std::sync::atomic::AtomicU64::new(u64::MAX),
		}))
	}

	pub fn root(&self) -> Option<PathBuf> {
		self.root.read().clone()
	}

	pub fn index(&self) -> Arc<TokioRwLock<EphemeralIndex>> {
		self.index.clone()
	}

	pub fn is_detached(&self) -> bool {
		self.detached.load(Ordering::Acquire)
	}

	fn set_detached(&self, detached: bool) {
		self.detached.store(detached, Ordering::Release);
	}

	fn root_len(&self) -> usize {
		self.root
			.read()
			.as_ref()
			.map(|r| r.as_os_str().len())
			.unwrap_or(0)
	}

	fn contains(&self, path: &Path) -> bool {
		self.root
			.read()
			.as_ref()
			.map(|r| path.starts_with(r))
			.unwrap_or(false)
	}
}

/// Summary of one source for status surfaces.
#[derive(Debug, Clone)]
pub struct SourceStatus {
	pub id: Uuid,
	pub root: PathBuf,
	pub fingerprint: Option<String>,
	pub attached: bool,
	pub restored: bool,
	pub last_seen_secs: u64,
	pub entry_count: Option<u64>,
	pub total_bytes: Option<u64>,
	/// The source's directory in the per-source layout, when persistence is on.
	pub directory: Option<PathBuf>,
	/// The source's thumbnail cache file within that directory.
	pub thumbs_path: Option<PathBuf>,
}

pub struct EphemeralIndexCache {
	/// Persistent source registrations.
	registry: Mutex<SourceRegistry>,
	/// Per-source directory layout; `None` means no persistence.
	dirs: Option<SourceDirs>,
	/// Live partitions by source id.
	slots: RwLock<HashMap<Uuid, Arc<SourceSlot>>>,
	/// Fallback partition for paths under no registered source.
	scratch: Arc<SourceSlot>,
	created_at: Instant,
}

impl EphemeralIndexCache {
	pub fn new() -> std::io::Result<Self> {
		Self::with_sources_dir(
			SourceDirs::from_default_data_dir()
				.ok()
				.map(|d| d.root().to_path_buf()),
		)
	}

	/// Build a cache backed by an explicit sources directory.
	///
	/// `None` means no persistence: sources work for the session but
	/// registrations and snapshots are not written anywhere.
	pub fn with_sources_dir(root: Option<PathBuf>) -> std::io::Result<Self> {
		let dirs = match root {
			Some(root) => Some(SourceDirs::new(root).map_err(std::io::Error::other)?),
			None => None,
		};
		let registry = match &dirs {
			Some(dirs) => SourceRegistry::load(dirs.root()),
			None => SourceRegistry::in_memory(),
		};
		Ok(Self {
			registry: Mutex::new(registry),
			dirs,
			slots: RwLock::new(HashMap::new()),
			scratch: SourceSlot::new(None, None)?,
			created_at: Instant::now(),
		})
	}

	/// Register a root as a source (idempotent; fingerprint match rebinds a
	/// returning drive to its existing identity). Returns the source id.
	///
	/// Fails when the registration cannot be written: a source that is not on
	/// disk is one that will not be recognised at next launch, and the snapshot
	/// it goes on to write would then belong to nothing.
	pub fn register_source(
		&self,
		root: &Path,
		fingerprint: Option<String>,
	) -> anyhow::Result<Uuid> {
		let record = self.registry.lock().register(root, fingerprint)?;
		let slot = self.slot_for_record(&record);
		*slot.root.write() = Some(record.root.clone());
		slot.set_detached(!record.root.exists());
		Ok(record.id)
	}

	/// All registered sources with their live state.
	pub fn sources(&self) -> Vec<SourceStatus> {
		let records: Vec<SourceRecord> = self.registry.lock().all().to_vec();
		let slots = self.slots.read();
		records
			.into_iter()
			.map(|record| {
				let slot = slots.get(&record.id);
				SourceStatus {
					attached: record.root.exists(),
					restored: slot
						.map(|s| s.restored.load(Ordering::Acquire))
						.unwrap_or(false),
					directory: self.dirs.as_ref().map(|d| d.source_dir(record.id)),
					thumbs_path: self.dirs.as_ref().map(|d| d.thumbs_file(record.id)),
					id: record.id,
					root: record.root,
					fingerprint: record.fingerprint,
					last_seen_secs: record.last_seen_secs,
					entry_count: record.entry_count,
					total_bytes: record.total_bytes,
				}
			})
			.collect()
	}

	/// Get (or lazily create) the live slot for a registry record.
	fn slot_for_record(&self, record: &SourceRecord) -> Arc<SourceSlot> {
		if let Some(slot) = self.slots.read().get(&record.id) {
			return slot.clone();
		}
		let mut slots = self.slots.write();
		slots
			.entry(record.id)
			.or_insert_with(|| {
				let slot = SourceSlot::new(Some(record.id), Some(record.root.clone()))
					.expect("create ephemeral index for source");
				slot.set_detached(!record.root.exists());
				slot
			})
			.clone()
	}

	/// Resolve the partition owning `path`: the registered source whose root
	/// is the longest prefix, else scratch.
	pub fn resolve(&self, path: &Path) -> Arc<SourceSlot> {
		let record = self.registry.lock().resolve(path).cloned();
		match record {
			Some(record) => self.slot_for_record(&record),
			None => self.scratch.clone(),
		}
	}

	/// Whether `path` belongs to a detached source (data may be restorable,
	/// but the filesystem underneath is gone — never dispatch indexing).
	pub fn is_detached(&self, path: &Path) -> bool {
		let slot = self.resolve(path);
		slot.is_detached()
	}

	/// The index owning `path`, unconditionally (scratch fallback).
	pub fn resolve_index(&self, path: &Path) -> Arc<TokioRwLock<EphemeralIndex>> {
		self.resolve(path).index()
	}

	/// Every live index, scratch included. For global lookups (uuid → entry)
	/// and aggregate stats.
	pub fn all_indexes(&self) -> Vec<Arc<TokioRwLock<EphemeralIndex>>> {
		let mut indexes: Vec<_> = self
			.slots
			.read()
			.values()
			.map(|slot| slot.index())
			.collect();
		indexes.push(self.scratch.index());
		indexes
	}

	/// Get the owning index if the given path has been indexed.
	///
	/// Exact-match only (for directory listing); `get_for_search` also accepts
	/// descendants of indexed roots.
	pub fn get_for_path(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let slot = self.resolve(path);
		if slot.indexed_paths.read().contains(path) {
			Some(slot.index())
		} else {
			None
		}
	}

	/// Get the owning index for searching within a path.
	///
	/// Accepts the path itself or any indexed ancestor, resolving symlinks
	/// (e.g. /Users → /System/Volumes/Data/Users).
	pub fn get_for_search(&self, path: &Path) -> Option<Arc<TokioRwLock<EphemeralIndex>>> {
		let slot = self.resolve(path);
		let indexed = slot.indexed_paths.read();

		if indexed.contains(path) {
			return Some(slot.index());
		}

		let canonical_path = path.canonicalize().ok();

		for indexed_path in indexed.iter() {
			if path.starts_with(indexed_path) {
				return Some(slot.index());
			}
			if let Some(ref canon) = canonical_path {
				if canon.starts_with(indexed_path) {
					return Some(slot.index());
				}
			}
			if let Ok(canonical_indexed) = indexed_path.canonicalize() {
				if path.starts_with(&canonical_indexed) {
					return Some(slot.index());
				}
				if let Some(ref canon) = canonical_path {
					if canon.starts_with(&canonical_indexed) {
						return Some(slot.index());
					}
				}
			}
		}
		drop(indexed);

		// The canonical form of the path may resolve into a different
		// partition (a symlinked volume root); try that partition too.
		if let Some(canon) = canonical_path {
			if canon != path {
				let canon_slot = self.resolve(&canon);
				if !Arc::ptr_eq(&canon_slot, &slot) {
					let indexed = canon_slot.indexed_paths.read();
					if indexed.contains(&canon) || indexed.iter().any(|p| canon.starts_with(p)) {
						return Some(canon_slot.index());
					}
				}
			}
		}

		None
	}

	pub fn is_indexed(&self, path: &Path) -> bool {
		self.resolve(path).indexed_paths.read().contains(path)
	}

	pub fn is_indexing(&self, path: &Path) -> bool {
		self.resolve(path)
			.indexing_in_progress
			.read()
			.contains(path)
	}

	/// Restore a registered source's snapshot into its partition, if it has
	/// one and hasn't been restored this session. Returns true when the
	/// source's data is available afterwards (restored now or already live).
	///
	/// Detached sources restore read-only: their entries become queryable but
	/// `is_detached` stays true so nothing dispatches indexing at them.
	pub async fn ensure_restored(&self, path: &Path) -> bool {
		let record = match self.registry.lock().resolve(path).cloned() {
			Some(record) => record,
			None => return false,
		};
		let slot = self.slot_for_record(&record);

		// Exactly one restore attempt per session, shared by all callers.
		// Data written by jobs before/while the attempt runs is merged over
		// afterwards by those jobs' own writes, never silently replaced.
		let restored = *slot
			.restore_once
			.get_or_init(|| Self::attempt_restore(self.dirs.clone(), record, slot.clone()))
			.await;

		restored || !slot.indexed_paths.read().is_empty()
	}

	/// The single restore attempt for a slot. Returns whether the snapshot
	/// was loaded; the result is cached by `restore_once` for the session.
	async fn attempt_restore(
		dirs: Option<SourceDirs>,
		record: SourceRecord,
		slot: Arc<SourceSlot>,
	) -> bool {
		let Some(dirs) = dirs else {
			return false;
		};
		let snapshot_path = dirs.snapshot_file(record.id);
		let loaded = match EphemeralIndex::load_snapshot(&snapshot_path) {
			Ok(Some((index, meta))) => Some((index, meta)),
			Ok(None) => None,
			Err(err) => {
				tracing::warn!(
					"Snapshot restore failed for {}: {err}",
					record.root.display()
				);
				None
			}
		};
		let Some((loaded_index, meta)) = loaded else {
			return false;
		};

		// The snapshot names the source it was taken for. The file is keyed by
		// source id in its path, so a mismatch means the cache directory was
		// copied or edited from outside; adopting it would bind one drive's
		// contents to another's identity.
		if meta.source_id != record.id {
			tracing::warn!(
				"Snapshot at {} belongs to source {}, not {}; ignoring",
				snapshot_path.display(),
				meta.source_id,
				record.id
			);
			return false;
		}

		// A drive that came back at a different mount point has absolute
		// paths from the old mount baked into the snapshot. Reindexing the
		// present drive is cheaper than being subtly wrong; the stale
		// snapshot goes so the next save is clean.
		if record.root.exists() && meta.root_path != record.root {
			tracing::info!(
				"Snapshot for {} was taken at {}; discarding for reindex",
				record.root.display(),
				meta.root_path.display()
			);
			let _ = std::fs::remove_file(&snapshot_path);
			return false;
		}

		{
			let mut index = slot.index.write().await;
			// Entries written before the restore completed (a browse job that
			// raced the load) are re-applied on top of the snapshot so neither
			// side's data is lost. add_entry keeps existing identities on
			// duplicate paths, so snapshot entries win their uuids.
			let mut fresh = std::mem::replace(&mut *index, loaded_index);
			let fresh_paths: Vec<PathBuf> = fresh.snapshot_data().3.keys().cloned().collect();
			for path in fresh_paths {
				let uuid = fresh.get_entry_uuid(&path).unwrap_or_else(Uuid::now_v7);
				if let Some(entry_meta) = fresh.get_entry(&path) {
					let _ = index.add_entry(path, uuid, entry_meta);
				}
			}
		}
		slot.indexed_paths.write().insert(meta.root_path.clone());
		slot.restored.store(true, Ordering::Release);
		slot.set_detached(!meta.root_path.exists());

		tracing::info!(
			"Restored source {} from snapshot ({}, {})",
			record.id,
			meta.root_path.display(),
			if slot.is_detached() {
				"detached"
			} else {
				"attached"
			}
		);
		true
	}

	/// Try to load a snapshot before indexing. Only registered sources have
	/// snapshots; scratch paths return false and index fresh.
	pub async fn try_load_snapshot_or_create(&self, path: &Path) -> anyhow::Result<bool> {
		if self.is_indexed(path) {
			return Ok(true);
		}
		Ok(self.ensure_restored(path).await)
	}

	/// Save the owning partition to its source-keyed snapshot file. Scratch
	/// partitions have no snapshot and skip silently.
	pub async fn save_snapshot(&self, path: &Path) -> anyhow::Result<()> {
		let record = match self.registry.lock().resolve(path).cloned() {
			Some(record) => record,
			None => {
				tracing::debug!(
					"No registered source for {}; skipping snapshot",
					path.display()
				);
				return Ok(());
			}
		};
		let Some(dirs) = &self.dirs else {
			return Ok(());
		};
		let slot = self.slot_for_record(&record);
		let snapshot_path = dirs.snapshot_file(record.id);

		// Funnel through the restore gate so a save can never precede the
		// session's restore attempt.
		self.ensure_restored(path).await;

		// A partition that was not seeded from the existing snapshot must not
		// overwrite it: a fresh session's few browsed directories would
		// replace a full drive index. Failed or skipped restores forfeit
		// saving; the durable artifact outlives the session that couldn't
		// read it. A snapshot this session wrote itself carries no data the
		// partition lacks, so overwriting it is always safe — without this
		// exemption, the first save after a failed restore would freeze the
		// source at that save for the rest of the session.
		let wrote_this_session = slot.last_saved_entries.load(Ordering::Acquire) != u64::MAX;
		if snapshot_path.exists() && !slot.restored.load(Ordering::Acquire) && !wrote_this_session {
			tracing::warn!(
				"Skipping snapshot save for {}: existing snapshot was not restored this session",
				record.root.display()
			);
			return Ok(());
		}

		let _save_guard = slot.save_lock.lock().await;
		let (entry_count, total_bytes) = {
			let mut index = slot.index.write().await;
			let stats = index.get_stats();
			let entry_count = stats.total_entries as u64;
			// Entry count cannot answer "did anything change": a rename, or a
			// delete balanced by an add, leaves it identical while changing what
			// has to persist. The index tracks its own mutations instead.
			if snapshot_path.exists() && !index.is_dirty() {
				tracing::debug!(
					"Snapshot for {} unchanged ({} entries); skipping rewrite",
					record.root.display(),
					entry_count
				);
				return Ok(());
			}
			index.save_snapshot(&snapshot_path, record.id, &record.root)?;
			index.clear_dirty();
			slot.last_saved_entries
				.store(entry_count, Ordering::Release);
			(entry_count, stats.total_file_bytes)
		};
		// Persist counts on the registry row so listings can show a drive's
		// size without loading its snapshot. The snapshot itself is already on
		// disk, so a failure here costs a stale count in listings rather than
		// the index: report it and keep the save successful.
		if let Err(err) = self
			.registry
			.lock()
			.update_stats(record.id, entry_count, total_bytes)
		{
			tracing::error!(
				"Saved snapshot for source {} but could not persist its counts: {err}",
				record.id
			);
		}
		tracing::info!(
			"Saved snapshot for source {} ({})",
			record.id,
			record.root.display()
		);
		Ok(())
	}

	/// Prepare the owning partition for indexing a new path.
	pub fn create_for_indexing(&self, path: PathBuf) -> Arc<TokioRwLock<EphemeralIndex>> {
		let slot = self.resolve(&path);
		let mut in_progress = slot.indexing_in_progress.write();
		let mut indexed = slot.indexed_paths.write();
		indexed.remove(&path);
		in_progress.insert(path);
		slot.index()
	}

	/// Clear stale entries for a path before re-indexing.
	pub async fn clear_for_reindex(&self, path: &Path) -> usize {
		let slot = self.resolve(path);
		let indexed = slot.indexed_paths.read().clone();
		let mut index = slot.index.write().await;
		let (cleared, deleted_browsed_dirs) = index.clear_directory_children(path, &indexed);

		if !deleted_browsed_dirs.is_empty() {
			let mut indexed_paths = slot.indexed_paths.write();
			for deleted_path in deleted_browsed_dirs {
				indexed_paths.remove(&deleted_path);
			}
		}

		cleared
	}

	/// Indexing finished and the arena holds the result.
	pub fn mark_indexing_complete(&self, path: &Path) {
		let slot = self.resolve(path);
		slot.indexing_in_progress.write().remove(path);
		slot.indexed_paths.write().insert(path.to_path_buf());
	}

	/// Indexing ended without producing a result. Clearing the in-progress
	/// flag is what lets the next browse re-dispatch; the path must not be
	/// recorded as indexed, or the partial arena is served as if complete
	/// and nothing ever tries again.
	pub fn mark_indexing_failed(&self, path: &Path) {
		let slot = self.resolve(path);
		slot.indexing_in_progress.write().remove(path);
		slot.indexed_paths.write().remove(path);
	}

	pub fn invalidate_path(&self, path: &Path) {
		self.resolve(path).indexed_paths.write().remove(path);
	}

	fn fold_slots<T>(&self, mut f: impl FnMut(&SourceSlot) -> T) -> Vec<T> {
		let mut out: Vec<T> = self.slots.read().values().map(|s| f(s)).collect();
		out.push(f(&self.scratch));
		out
	}

	pub fn len(&self) -> usize {
		self.fold_slots(|s| s.indexed_paths.read().len())
			.into_iter()
			.sum()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn indexed_paths(&self) -> Vec<PathBuf> {
		self.fold_slots(|s| s.indexed_paths.read().iter().cloned().collect::<Vec<_>>())
			.into_iter()
			.flatten()
			.collect()
	}

	pub fn paths_in_progress(&self) -> Vec<PathBuf> {
		self.fold_slots(|s| {
			s.indexing_in_progress
				.read()
				.iter()
				.cloned()
				.collect::<Vec<_>>()
		})
		.into_iter()
		.flatten()
		.collect()
	}

	/// Register a path for filesystem watching. The path must already be
	/// indexed in its partition; detached partitions refuse.
	pub fn register_for_watching(&self, path: PathBuf) -> bool {
		let slot = self.resolve(&path);
		if slot.is_detached() || !slot.indexed_paths.read().contains(&path) {
			return false;
		}
		slot.watched_paths.write().insert(path);
		true
	}

	pub fn unregister_from_watching(&self, path: &Path) {
		self.resolve(path).watched_paths.write().remove(path);
	}

	pub fn is_watched(&self, path: &Path) -> bool {
		self.resolve(path).watched_paths.read().contains(path)
	}

	pub fn watched_paths(&self) -> Vec<PathBuf> {
		self.fold_slots(|s| s.watched_paths.read().iter().cloned().collect::<Vec<_>>())
			.into_iter()
			.flatten()
			.collect()
	}

	/// Find the watched root that contains the given path, across all
	/// partitions; longest match wins.
	pub fn find_watched_root(&self, path: &Path) -> Option<PathBuf> {
		self.fold_slots(|s| {
			s.watched_paths
				.read()
				.iter()
				.filter(|w| path.starts_with(w))
				.max_by_key(|w| w.as_os_str().len())
				.cloned()
		})
		.into_iter()
		.flatten()
		.max_by_key(|w| w.as_os_str().len())
	}

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

	/// Clear every partition (registrations survive; snapshots on disk
	/// survive — this resets in-memory state only).
	///
	/// Slots are dropped rather than emptied so each source gets a fresh
	/// restore gate: the next touch re-restores from its snapshot instead of
	/// carrying a spent gate over an empty arena.
	pub async fn clear_all(&self) -> usize {
		let old_slots: Vec<Arc<SourceSlot>> = {
			let mut slots = self.slots.write();
			let old: Vec<_> = slots.values().cloned().collect();
			slots.clear();
			old
		};

		let mut cleared = 0usize;
		for slot in old_slots {
			cleared += slot.indexed_paths.read().len() + slot.indexing_in_progress.read().len();
		}

		{
			let mut indexed = self.scratch.indexed_paths.write();
			let mut in_progress = self.scratch.indexing_in_progress.write();
			let mut watched = self.scratch.watched_paths.write();
			cleared += indexed.len() + in_progress.len();
			indexed.clear();
			in_progress.clear();
			watched.clear();
		}
		{
			let mut index = self.scratch.index.write().await;
			*index = EphemeralIndex::new().expect("Failed to create new ephemeral index");
		}

		cleared
	}

	pub fn stats(&self) -> EphemeralIndexCacheStats {
		EphemeralIndexCacheStats {
			indexed_paths: self.len(),
			indexing_in_progress: self
				.fold_slots(|s| s.indexing_in_progress.read().len())
				.into_iter()
				.sum(),
			watched_paths: self
				.fold_slots(|s| s.watched_paths.read().len())
				.into_iter()
				.sum(),
			sources: self.registry.lock().all().len(),
		}
	}

	pub fn age(&self) -> std::time::Duration {
		self.created_at.elapsed()
	}

	pub fn get_age(&self, _path: &Path) -> Option<f64> {
		Some(self.created_at.elapsed().as_secs_f64())
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
	pub indexed_paths: usize,
	pub indexing_in_progress: usize,
	pub watched_paths: usize,
	pub sources: usize,
}

impl EphemeralIndexCacheStats {
	pub fn total_entries(&self) -> usize {
		self.indexed_paths
	}

	pub fn indexing_count(&self) -> usize {
		self.indexing_in_progress
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn isolated_cache() -> EphemeralIndexCache {
		EphemeralIndexCache::with_sources_dir(None).expect("failed to create cache")
	}

	#[test]
	fn test_scratch_workflow() {
		let cache = isolated_cache();
		let path = PathBuf::from("/test/path");

		assert!(cache.is_empty());
		assert!(cache.get_for_path(&path).is_none());

		let _index = cache.create_for_indexing(path.clone());
		assert!(cache.is_indexing(&path));
		assert!(!cache.is_indexed(&path));

		cache.mark_indexing_complete(&path);
		assert!(!cache.is_indexing(&path));
		assert!(cache.is_indexed(&path));
		assert!(cache.get_for_path(&path).is_some());

		cache.invalidate_path(&path);
		assert!(!cache.is_indexed(&path));
	}

	#[test]
	fn test_partition_isolation() {
		let cache = isolated_cache();

		let a = cache
			.register_source(Path::new("/test/vol-a"), None)
			.unwrap();
		let b = cache
			.register_source(Path::new("/test/vol-b"), None)
			.unwrap();
		assert_ne!(a, b);

		let index_a = cache.create_for_indexing(PathBuf::from("/test/vol-a/dir"));
		let index_b = cache.create_for_indexing(PathBuf::from("/test/vol-b/dir"));
		let scratch = cache.create_for_indexing(PathBuf::from("/elsewhere"));

		// Distinct partitions get distinct indexes.
		assert!(!Arc::ptr_eq(&index_a, &index_b));
		assert!(!Arc::ptr_eq(&index_a, &scratch));

		// Same partition shares one index.
		let index_a2 = cache.create_for_indexing(PathBuf::from("/test/vol-a/other"));
		assert!(Arc::ptr_eq(&index_a, &index_a2));
	}

	#[test]
	fn test_longest_prefix_resolution() {
		let cache = isolated_cache();

		cache.register_source(Path::new("/mnt"), None).unwrap();
		let nested = cache
			.register_source(Path::new("/mnt/drive"), None)
			.unwrap();

		let slot = cache.resolve(Path::new("/mnt/drive/file.txt"));
		assert_eq!(slot.id, Some(nested));
	}

	#[test]
	fn test_watch_registration_and_root_lookup() {
		let cache = isolated_cache();

		let root = PathBuf::from("/mnt/nas");
		let child = PathBuf::from("/mnt/nas/documents/report.pdf");

		assert!(!cache.register_for_watching(root.clone()));

		let _index = cache.create_for_indexing(root.clone());
		cache.mark_indexing_complete(&root);
		assert!(cache.register_for_watching(root.clone()));
		assert!(cache.is_watched(&root));
		assert_eq!(cache.find_watched_root(&child), Some(root.clone()));
		assert_eq!(cache.find_watched_root(Path::new("/other/path")), None);

		cache.unregister_from_watching(&root);
		assert!(!cache.is_watched(&root));
	}

	#[test]
	fn test_stats_aggregate_across_partitions() {
		let cache = isolated_cache();

		cache
			.register_source(Path::new("/test/vol-a"), None)
			.unwrap();

		let ready = PathBuf::from("/test/vol-a/ready");
		let in_progress = PathBuf::from("/scratch/in_progress");

		let _i = cache.create_for_indexing(ready.clone());
		cache.mark_indexing_complete(&ready);
		let _i = cache.create_for_indexing(in_progress);

		let stats = cache.stats();
		assert_eq!(stats.indexed_paths, 1);
		assert_eq!(stats.indexing_in_progress, 1);
	}

	#[tokio::test]
	async fn test_snapshot_roundtrip_and_detached_restore() {
		use crate::ops::indexing::state::EntryKind;
		use crate::ops::indexing::EntryMetadata;

		let cache_dir = tempfile::tempdir().unwrap();
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();

		let file_meta = |path: &Path| EntryMetadata {
			kind: EntryKind::File,
			path: path.to_path_buf(),
			size: 42,
			modified: None,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			is_hidden: false,
		};

		// Session one: register the drive, index some entries, snapshot.
		let saved_uuid;
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			cache
				.register_source(&root, Some("fp-roundtrip".into()))
				.unwrap();

			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				let file_path = root.join("photo.jpg");
				let (_, uuid) = index
					.add_entry(
						file_path,
						uuid::Uuid::now_v7(),
						file_meta(&root.join("photo.jpg")),
					)
					.unwrap();
				saved_uuid = uuid;
			}
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.expect("save snapshot");
		}

		// The drive is "unplugged": its root no longer exists.
		let unplugged_root = root.clone();
		drop(drive_dir);
		assert!(!unplugged_root.exists());

		// Session two: fresh cache, same registry dir. The source is known,
		// restores from its snapshot, and serves read-only as detached.
		let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
			.expect("cache");
		assert_eq!(cache.sources().len(), 1);

		let child = unplugged_root.join("photo.jpg");
		assert!(cache.ensure_restored(&child).await);
		assert!(cache.is_detached(&child));

		let index = cache
			.get_for_search(&child)
			.expect("restored index should cover the drive");
		let index = index.read().await;
		assert_eq!(index.get_entry_uuid(&child), Some(saved_uuid));
	}

	#[tokio::test]
	async fn test_session_can_overwrite_its_own_snapshot() {
		use crate::ops::indexing::state::EntryKind;
		use crate::ops::indexing::EntryMetadata;

		let cache_dir = tempfile::tempdir().unwrap();
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();

		let meta = |path: &Path| EntryMetadata {
			kind: EntryKind::File,
			path: path.to_path_buf(),
			size: 1,
			modified: None,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			is_hidden: false,
		};

		// One session, no pre-existing snapshot: a shallow browse saves a few
		// entries, then a full scan of the same source saves many more. The
		// second save must replace the first — the existing file is this
		// session's own artifact, not a prior session's index.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			cache
				.register_source(&root, Some("fp-self-overwrite".into()))
				.unwrap();

			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				let p = root.join("shallow.txt");
				index
					.add_entry(p.clone(), Uuid::now_v7(), meta(&p))
					.unwrap();
			}
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.unwrap();

			{
				let mut index = index.write().await;
				for name in ["a.txt", "b.txt", "c.txt"] {
					let p = root.join(name);
					index
						.add_entry(p.clone(), Uuid::now_v7(), meta(&p))
						.unwrap();
				}
			}
			cache.save_snapshot(&root).await.unwrap();
		}

		// A fresh session restores everything the scan found.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			assert!(cache.ensure_restored(&root.join("a.txt")).await);
			let index = cache.resolve_index(&root);
			let index = index.read().await;
			for name in ["shallow.txt", "a.txt", "b.txt", "c.txt"] {
				assert!(
					index.get_entry_uuid(&root.join(name)).is_some(),
					"{name} missing from overwritten snapshot"
				);
			}
		}
	}

	#[tokio::test]
	async fn test_unrestored_session_cannot_clobber_snapshot() {
		use crate::ops::indexing::state::EntryKind;
		use crate::ops::indexing::EntryMetadata;

		let cache_dir = tempfile::tempdir().unwrap();
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();

		let meta = |path: &Path| EntryMetadata {
			kind: EntryKind::File,
			path: path.to_path_buf(),
			size: 1,
			modified: None,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			is_hidden: false,
		};

		// Session one: a full index of three entries, snapshotted.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			cache
				.register_source(&root, Some("fp-clobber".into()))
				.unwrap();
			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				for name in ["a.txt", "b.txt", "c.txt"] {
					let p = root.join(name);
					index
						.add_entry(p.clone(), Uuid::now_v7(), meta(&p))
						.unwrap();
				}
			}
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.unwrap();
		}

		// Session two writes one entry and saves. The restore gate runs inside
		// save_snapshot, seeding the partition first, so the save merges the
		// snapshot's three entries with the new one instead of replacing them.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				let p = root.join("d.txt");
				index
					.add_entry(p.clone(), Uuid::now_v7(), meta(&p))
					.unwrap();
			}
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.unwrap();
		}

		// Session three: the snapshot holds the union, not the last writer.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			assert!(cache.ensure_restored(&root.join("a.txt")).await);
			let index = cache.resolve_index(&root);
			let index = index.read().await;
			for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
				assert!(
					index.get_entry_uuid(&root.join(name)).is_some(),
					"{name} missing from merged snapshot"
				);
			}
		}
	}

	/// A rename changes no counts. The snapshot has to notice anyway, and the
	/// drive that comes back at a familiar mount point has to stay itself.
	#[tokio::test]
	async fn test_rename_persists_and_foreign_drive_keeps_its_distance() {
		use crate::ops::indexing::state::EntryKind;
		use crate::ops::indexing::EntryMetadata;

		let cache_dir = tempfile::tempdir().unwrap();
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();

		let meta = |path: &Path| EntryMetadata {
			kind: EntryKind::File,
			path: path.to_path_buf(),
			size: 3,
			modified: None,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			is_hidden: false,
		};

		let before = root.join("before.txt");
		let after = root.join("after.txt");
		let source_id;
		let renamed_uuid;

		// Session one: index one file, snapshot it.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			source_id = cache
				.register_source(&root, Some("fp-rename".into()))
				.unwrap();
			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				index
					.add_entry(before.clone(), Uuid::now_v7(), meta(&before))
					.unwrap();
			}
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.unwrap();
		}

		// Session two: rename it. The entry count is identical either side, so
		// only a real change signal gets this to disk.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			cache.ensure_restored(&root).await;
			let index = cache.resolve_index(&root);
			{
				let mut index = index.write().await;
				let uuid = index.get_entry_uuid(&before).expect("restored entry");
				renamed_uuid = uuid;
				index.remove_entry(&before);
				index.add_entry(after.clone(), uuid, meta(&after)).unwrap();
			}
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.unwrap();
		}

		// Session three: the rename survived, under the same identity.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			assert!(cache.ensure_restored(&after).await);
			let index = cache.resolve_index(&root);
			let mut index = index.write().await;
			assert!(index.get_entry(&before).is_none(), "old name persisted");
			assert_eq!(index.get_entry_uuid(&after), Some(renamed_uuid));
		}

		// A different drive mounted where that one lives is a different source,
		// so it can never be served the first drive's snapshot.
		{
			let cache = EphemeralIndexCache::with_sources_dir(Some(cache_dir.path().to_path_buf()))
				.expect("cache");
			let other = cache
				.register_source(&root, Some("fp-other".into()))
				.unwrap();
			assert_ne!(other, source_id);

			let statuses = cache.sources();
			let original = statuses.iter().find(|s| s.id == source_id).unwrap();
			assert_eq!(
				original.fingerprint.as_deref(),
				Some("fp-rename"),
				"the displaced drive kept its own identity"
			);
		}
	}
}
