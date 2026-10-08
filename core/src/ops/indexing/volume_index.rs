//! # Volume Index
//!
//! Every attached drive mapped into memory, one [`Partition`] per drive. A
//! partition holds the drive's arena and the path state that belongs to the
//! map, and every source registered over the drive shares it; what each source
//! keeps durably lives in its own store. Restoring or clearing one drive never
//! touches another, and a detached drive's arena is restored read-only from its
//! snapshot while the drive is in a drawer.
//!
//! Paths on no tracked drive land in the **scratch** partition, which serves
//! ad-hoc browsing and never snapshots.

use super::sources::{SourceRecord, SourceRegistry, VolumeAnchor, VolumeKey};
use super::store::{DuplicateCopy, SourceStore};
use super::Arena;
use crate::infra::db::entities::source;
use crate::infra::db::Database;
use crate::infra::source_dirs::SourceDirs;
use crate::ops::indexing::sources::SourceConfig;
use parking_lot::{Mutex, RwLock};
use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Instant,
};
use tokio::sync::mpsc;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

/// One drive's index: the arena for it, plus the path state that
/// belongs to the map rather than to any registration.
///
/// Keyed by the drive and not by the source, so a source nested inside another shares
/// this rather than forking it. See [`VolumeKey`].
pub struct Partition {
	/// Which drive this maps. `Scratch` for paths under no registered source.
	pub volume: VolumeKey,
	/// Where the drive begins. Scratch has no root.
	root: RwLock<Option<PathBuf>>,
	index: Arc<TokioRwLock<Arena>>,
	indexed_paths: RwLock<HashSet<PathBuf>>,
	indexing_in_progress: RwLock<HashSet<PathBuf>>,
	watched_paths: RwLock<HashSet<PathBuf>>,
	/// A detached source's root is not present on disk; its index is served
	/// read-only from a restored snapshot and must never trigger indexing.
	detached: AtomicBool,
	/// Set once a snapshot restore or a store rebuild has populated the
	/// index this session.
	restored: AtomicBool,
	/// Single restore attempt per session: concurrent callers await the same
	/// load instead of racing three copies of a 100 MB deserialization.
	restore_once: tokio::sync::OnceCell<RestoreOutcome>,
	/// Serializes snapshot saves for this drive.
	save_lock: tokio::sync::Mutex<()>,
	/// Entry count at the last completed save; identical partitions skip the
	/// rewrite (a burst of browse jobs otherwise re-saves 100 MB per job).
	last_saved_entries: std::sync::atomic::AtomicU64,
}

/// How a partition's session restore ended.
///
/// A snapshot carries the whole drive map; a store rebuild carries only the
/// registered sources on it. The discovery pass walks the rest of a drive
/// whose snapshot is gone, so the two are kept apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestoreOutcome {
	/// Nothing populated the partition: no snapshot and no source store.
	Nothing,
	/// The drive snapshot loaded.
	Snapshot,
	/// At least one source's records were read back from its store.
	Stores,
}

impl Partition {
	fn new(volume: VolumeKey, root: Option<PathBuf>) -> std::io::Result<Arc<Self>> {
		Ok(Arc::new(Self {
			volume,
			root: RwLock::new(root),
			index: Arc::new(TokioRwLock::new(Arena::new()?)),
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

	/// This index's stable id, which keys its directory on disk.
	pub fn id(&self) -> Option<Uuid> {
		match self.volume {
			VolumeKey::Scratch => None,
			_ => Some(self.volume.id()),
		}
	}

	pub fn root(&self) -> Option<PathBuf> {
		self.root.read().clone()
	}

	pub fn index(&self) -> Arc<TokioRwLock<Arena>> {
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

/// A drive this machine maps.
#[derive(Debug, Clone)]
struct TrackedVolume {
	uuid: Uuid,
	mount_point: PathBuf,
	/// Whether the drive is mounted right now, as far as this process
	/// knows: live detection when the library attached with it, the stored
	/// flag otherwise, and the volume monitor's refreshes after that.
	mounted: bool,
	/// Whether `mount_point` is where a filesystem mounts, so the directory
	/// can be checked against the mount table before anything walks it. A
	/// drive learned from a volume row or from detection is; a directory a
	/// test tracks as a drive is not.
	is_mount: bool,
}

/// Rows read per arena lock during a store rebuild. At the measured insert
/// cost a page holds the lock for a few milliseconds.
const REBUILD_PAGE: usize = 2_000;

/// Add every filesystem row of a store to an arena, rooted at `root`.
///
/// Ancestors are synthesized by the arena itself and content kinds derive
/// from extensions the way a fresh walk derives them, which is what makes a
/// database browsable and searchable through the same paths an arena
/// snapshot is. The arena is locked one store page at a time, so a
/// multi-million-record rebuild never holds a listing on the same drive
/// for longer than one page of inserts. Returns how many entries were added.
pub(crate) async fn fill_arena_from_store(
	index: &TokioRwLock<Arena>,
	db: &sd_store::SourceDb,
	root: &Path,
) -> anyhow::Result<usize> {
	use crate::ops::indexing::metadata::EntryMetadata;
	use crate::ops::indexing::state::EntryKind;
	use std::time::{Duration, UNIX_EPOCH};

	let mut added = 0usize;
	let mut after_rowid = 0i64;
	loop {
		let (entries, last) =
			sd_store::read::rebuild_entries_page(db.pool(), after_rowid, REBUILD_PAGE)
				.await
				.map_err(|e| anyhow::anyhow!("database page failed: {e}"))?;
		let done = entries.len() < REBUILD_PAGE;
		after_rowid = last;

		let mut index = index.write().await;
		for entry in entries {
			let path = root.join(&entry.relative_path);
			let from_ms = |ms: Option<i64>| {
				ms.and_then(|ms| u64::try_from(ms).ok())
					.map(|ms| UNIX_EPOCH + Duration::from_millis(ms))
			};
			let metadata = EntryMetadata {
				path: path.clone(),
				kind: match entry.kind {
					sd_store::FileKind::File => EntryKind::File,
					sd_store::FileKind::Directory => EntryKind::Directory,
					sd_store::FileKind::Symlink => EntryKind::Symlink,
				},
				size: entry.size.unwrap_or(0).max(0) as u64,
				modified: from_ms(entry.mtime_ms),
				accessed: from_ms(entry.atime_ms),
				created: from_ms(entry.created_ms),
				inode: entry.inode.and_then(|inode| u64::try_from(inode).ok()),
				permissions: entry.mode.and_then(|mode| u32::try_from(mode).ok()),
				uid: entry.uid.and_then(|uid| u32::try_from(uid).ok()),
				gid: entry.gid.and_then(|gid| u32::try_from(gid).ok()),
				link_target: entry.link_target.clone(),
				is_hidden: entry.is_hidden,
			};
			index.add_entry(path, entry.uuid, metadata)?;
			added += 1;
		}
		drop(index);

		if done {
			break;
		}
	}
	Ok(added)
}

/// Fill a source's subtree of a partition from its store and, when the
/// store held anything, mark the root indexed and the partition restored.
/// Announcing the root is the caller's: it decides whether a watch should
/// follow. `None` when the store could not be read.
async fn fill_source_from_store(
	slot: &Partition,
	db: &sd_store::SourceDb,
	root: &Path,
) -> Option<usize> {
	let loaded = match fill_arena_from_store(&slot.index, db, root).await {
		Ok(loaded) => loaded,
		Err(error) => {
			tracing::warn!(root = %root.display(), %error, "could not rebuild the map from the store");
			return None;
		}
	};
	if loaded > 0 {
		slot.indexed_paths.write().insert(root.to_path_buf());
		slot.restored.store(true, Ordering::Release);
	}
	Some(loaded)
}

/// Whether a volume root is a service prefix rather than a filesystem path.
///
/// A cloud volume mounts at `s3://bucket` or `gdrive://id`: nothing on this
/// machine's mount table, nothing `Path::exists` can answer for, and never
/// something an unmount leaves a directory behind for. It is reachable as
/// long as its row says so.
fn is_cloud_root(root: &Path) -> bool {
	root.to_string_lossy().contains("://")
}

/// What volume detection reports at the moment a library attaches.
///
/// `attach_library` resolves every anchored source against this rather than
/// the `is_online` flag stored with the volume row, because the flag is
/// written by the monitor's refreshes and a drive that vanished while the
/// daemon was down still reads online. With detection off there is nothing
/// live to consult and the stored flag is the fallback.
pub enum LiveVolumes<'a> {
	/// Volume monitoring is disabled; trust the rows.
	Unavailable,
	/// Every volume detection currently returns.
	Detected(&'a [crate::volume::Volume]),
}

/// Where a path belongs: the drive that maps it, and the source that keeps it.
#[derive(Debug, Clone)]
struct Resolved {
	volume: VolumeKey,
	volume_root: PathBuf,
	source: Option<SourceRecord>,
}

/// Summary of one source for status surfaces.
#[derive(Debug, Clone)]
pub struct SourceStatus {
	pub id: Uuid,
	/// The library that registered this source; `None` for a registration
	/// made before any library was open.
	pub library: Option<Uuid>,
	pub root: PathBuf,
	/// The drive this source sits on, when it sits on one Spacedrive tracks.
	pub volume_uuid: Option<Uuid>,
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

/// Below this many entries a partition is small enough that losing it costs
/// nothing worth a refusal, and shrinking is ordinary.
const COLLAPSE_FLOOR: u64 = 1_000;

/// How much smaller a save has to be than what it replaces before it is treated
/// as a collapse rather than a deletion someone actually performed.
const COLLAPSE_FACTOR: u64 = 10;

/// Keeps a set of sources' stores closed; see [`VolumeIndex::quiesce_stores`].
pub struct StoreHold {
	_guards: Vec<tokio::sync::OwnedMutexGuard<()>>,
}

/// What a restore has to clear in memory and on disk for a set of sources:
/// the drive partitions that map them and the drive snapshots that would
/// restore those partitions. See [`VolumeIndex::quiesce_targets`].
#[derive(Debug, Default)]
pub struct QuiesceTargets {
	partitions: Vec<VolumeKey>,
	snapshots: Vec<PathBuf>,
}

/// One library's registrations and where they are written.
///
/// A registration made before any library is open has nowhere durable to
/// go; it lives in the session registry, whose `library` is `None`, for as
/// long as the process does.
struct LibrarySources {
	library: Option<Uuid>,
	db: Option<Arc<Database>>,
	registry: SourceRegistry,
}

/// A registered source together with the drive that maps it.
struct Located {
	record: SourceRecord,
	volume: VolumeKey,
	volume_root: PathBuf,
}

pub struct VolumeIndex {
	/// Registered sources, in memory, one registry per open library. The
	/// durable copy of each is the `sources` table in that library.
	///
	/// This cache is machine-scoped and a source registration is library
	/// metadata, so the two do not have the same lifetime. Each library
	/// attaches its own registry when it opens and takes it away when it
	/// closes; the drives underneath are mapped once and shared, since a
	/// mount is a fact about the machine and not about any library. A path
	/// resolves across every attached library, because a source belongs to
	/// exactly one of them.
	libraries: Mutex<Vec<LibrarySources>>,
	/// Per-source directory layout; `None` means no persistence.
	dirs: Option<SourceDirs>,
	/// Live partitions by source id.
	/// Live indexes by drive. One drive, one arena, however many sources
	/// are registered over it.
	slots: RwLock<HashMap<VolumeKey, Arc<Partition>>>,
	/// Durable stores by source id. One drive can host several, since a source
	/// nested inside another persists its own subtree.
	stores: RwLock<HashMap<Uuid, Arc<SourceStore>>>,
	/// Read-only store handles by source id. The write handle above exists to
	/// ingest; these exist to answer when no arena covers a source, and they
	/// open without DDL, ledger or writer task.
	read_stores: RwLock<HashMap<Uuid, Arc<sd_store::SourceDb>>>,
	/// One async gate per source, so concurrent first opens coalesce into a
	/// single pool, ledger load, and writer task. See [`Self::store_for`].
	store_open_gates: Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>,
	/// Fallback partition for paths on no tracked drive.
	scratch: Arc<Partition>,
	/// Drives this machine maps, whether or not anything is kept off them.
	///
	/// A drive is mapped by tracking it and persisted by registering a source
	/// over it, and those are different acts. Holding them apart is what lets
	/// the whole machine be searchable while only a home folder is kept.
	volumes: Mutex<Vec<TrackedVolume>>,
	/// Roots that have just become browsable, announced so whoever owns
	/// filesystem watching can arm them.
	///
	/// An index arrives two ways and only one of them used to arm a watch. A
	/// walk finishes and the indexing job watches what it walked; a restore
	/// rebuilds the same index from a snapshot and armed nothing, so every
	/// restart left a fully browsable drive that reported no changes. A channel
	/// rather than a handle because the watcher service holds this cache, and
	/// holding it back would be a cycle.
	restored_roots: RwLock<Option<mpsc::UnboundedSender<PathBuf>>>,
	/// Summarised directories something changed under.
	///
	/// A change under one cannot be applied to a tree that was never kept, and
	/// the event carries a path and a kind but no size, so the totals cannot be
	/// adjusted from it either. What the change does say is that the count is
	/// now wrong, which is enough: the directory is recounted.
	dirty_stubs: Mutex<HashSet<PathBuf>>,
	/// Roots whose OS watch subscription was refused, with the reason.
	///
	/// A root is either here or in a partition's `watched_paths`, never
	/// both: an active watch is one the OS accepted, and a refusal is what
	/// the status surface shows instead until a retry succeeds.
	refused_watches: Mutex<HashMap<PathBuf, String>>,
	created_at: Instant,
}

impl VolumeIndex {
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
		Ok(Self {
			libraries: Mutex::new(Vec::new()),
			dirs,
			slots: RwLock::new(HashMap::new()),
			scratch: Partition::new(VolumeKey::Scratch, None)?,
			volumes: Mutex::new(Vec::new()),
			stores: RwLock::new(HashMap::new()),
			read_stores: RwLock::new(HashMap::new()),
			store_open_gates: Mutex::new(HashMap::new()),
			restored_roots: RwLock::new(None),
			dirty_stubs: Mutex::new(HashSet::new()),
			refused_watches: Mutex::new(HashMap::new()),
			created_at: Instant::now(),
		})
	}

	/// Adopt the source rows of a library that just opened, and write
	/// registrations there from now on.
	///
	/// Absolute roots resolve against wherever each anchoring volume is mounted
	/// right now, so a drive that came back somewhere else needs no repair. A
	/// row whose volume is absent keeps its record and its counts; only its
	/// path is unavailable, which is what is true of a drive in a drawer.
	pub async fn attach_library(&self, library: Uuid, db: Arc<Database>) -> anyhow::Result<usize> {
		self.attach_library_with(library, db, None, LiveVolumes::Unavailable)
			.await
	}

	/// [`Self::attach_library`], resolving this device's volume rows against
	/// what detection reports now.
	///
	/// A row detection cannot see is offline whatever its flag says, and the
	/// flag is corrected in place so `sources.list` and the next attach agree
	/// with the sources adopted here. A row detection sees is mounted at the
	/// mount point detection reports, which is what lets a drive that came back
	/// elsewhere resolve without repair.
	pub async fn attach_library_with(
		&self,
		library: Uuid,
		db: Arc<Database>,
		device_id: Option<Uuid>,
		live: LiveVolumes<'_>,
	) -> anyhow::Result<usize> {
		use crate::infra::db::entities::volume;
		use sea_orm::ActiveModelTrait;

		let rows = source::Entity::find()
			.filter(source::Column::DataType.eq(source::FILESYSTEM_DATA_TYPE))
			.all(db.conn())
			.await?;

		// The volumes table holds every device's rows; only this device's
		// drives are mounted here, and only its rows may be corrected.
		// `None` is for tests that build a library without a device.
		let mut volume_query = volume::Entity::find();
		if let Some(device_id) = device_id {
			volume_query = volume_query.filter(volume::Column::DeviceId.eq(device_id));
		}
		let volume_rows = volume_query.all(db.conn()).await?;

		// Every drive with a known mount point is mapped, mounted or not, so a
		// source on a drive that is away still resolves to the root it had and
		// its map can be read there; whether anything may be dispatched at
		// that root is the partition's detached flag, set below.
		let mut mounts: HashMap<Uuid, PathBuf> = HashMap::new();
		for row in volume_rows {
			let (mounted, mount_point) = match &live {
				LiveVolumes::Unavailable => {
					(row.is_online, row.mount_point.as_ref().map(PathBuf::from))
				}
				LiveVolumes::Detected(volumes) => {
					let detected = volumes
						.iter()
						.find(|volume| volume.fingerprint.0 == row.fingerprint);
					match detected {
						Some(volume) if volume.is_mounted => {
							(true, Some(volume.mount_point.clone()))
						}
						_ => (false, row.mount_point.as_ref().map(PathBuf::from)),
					}
				}
			};

			let Some(mount_point) = mount_point else {
				continue;
			};
			mounts.insert(row.uuid, mount_point.clone());
			// Detection lists filesystems; a cloud volume is restored from
			// its row by the volume manager and is not away for being
			// absent here.
			if is_cloud_root(&mount_point) {
				self.track_volume(row.uuid, mount_point);
				continue;
			}
			self.track_volume_state(row.uuid, mount_point, mounted);

			if row.is_online != mounted {
				tracing::info!(
					volume = %row.uuid,
					mounted,
					"stored volume state disagrees with detection; correcting it"
				);
				let uuid = row.uuid;
				let mut active: volume::ActiveModel = row.into();
				active.is_online = Set(mounted);
				if let Err(e) = active.update(db.conn()).await {
					tracing::warn!(volume = %uuid, %e, "could not persist the volume's state");
				}
			}
		}

		let registry = SourceRegistry::from_rows(rows, |uuid| mounts.get(&uuid).cloned());
		let adopted = registry.all().len();

		for record in registry.all() {
			let (volume, volume_root) = registry.volume_of(record);
			let resolved = Resolved {
				volume,
				volume_root: volume_root.clone(),
				source: Some(record.clone()),
			};
			let slot = self.slot_for(&resolved);
			*slot.root.write() = Some(volume_root.clone());
			slot.set_detached(!self.root_attached(&slot.volume, &volume_root));
		}

		// Only this library's registrations are replaced; every other open
		// library keeps serving its own sources.
		let mut libraries = self.libraries.lock();
		libraries.retain(|attached| attached.library != Some(library));
		libraries.push(LibrarySources {
			library: Some(library),
			db: Some(db),
			registry,
		});
		Ok(adopted)
	}

	/// Whether a partition's root is reachable: its drive is mounted, as far as
	/// this process knows, and the root exists.
	///
	/// Existence alone is not attachment. A drive that is away leaves its
	/// mount point behind as an empty directory on the parent filesystem, and
	/// treating that directory as the drive is how an index of a locked
	/// dataset came to be walked as an empty source.
	fn root_attached(&self, volume: &VolumeKey, root: &Path) -> bool {
		if is_cloud_root(root) {
			return true;
		}
		let mounted = match volume {
			VolumeKey::Id(uuid) => self.volume_mounted(*uuid).unwrap_or(true),
			_ => true,
		};
		mounted && root.exists()
	}

	/// Whether a mapped drive is mounted, or `None` for one this machine does
	/// not map.
	pub fn volume_mounted(&self, uuid: Uuid) -> Option<bool> {
		Self::volume_mounted_locked(&self.volumes.lock(), uuid)
	}

	fn volume_mounted_locked(volumes: &[TrackedVolume], uuid: Uuid) -> Option<bool> {
		volumes
			.iter()
			.find(|tracked| tracked.uuid == uuid)
			.map(|tracked| tracked.mounted)
	}

	/// Record that a mapped drive mounted or unmounted while the daemon runs.
	///
	/// Every source on the drive follows. An unmounted drive's map stays
	/// readable and its partition detaches, so nothing dispatches at the
	/// mount point left behind. A mounted drive's sources resolve to their
	/// roots under the mount point detection reports, which also covers a
	/// drive that was away at attach and whose records had no root to give.
	/// Nothing here arms a watch; the restore announcement and the watcher's
	/// retry pass own that.
	pub fn set_volume_mounted(&self, uuid: Uuid, mount_point: &Path, mounted: bool) {
		let key = VolumeKey::Id(uuid);
		{
			let mut volumes = self.volumes.lock();
			match volumes.iter_mut().find(|tracked| tracked.uuid == uuid) {
				Some(tracked) => {
					tracked.mounted = mounted;
					tracked.is_mount = !is_cloud_root(&tracked.mount_point);
					if mounted {
						tracked.mount_point = mount_point.to_path_buf();
					}
				}
				None if mounted => volumes.push(TrackedVolume {
					uuid,
					mount_point: mount_point.to_path_buf(),
					mounted: true,
					is_mount: !is_cloud_root(mount_point),
				}),
				None => return,
			}
		}
		if mounted {
			for attached in self.libraries.lock().iter_mut() {
				attached.registry.remount(uuid, mount_point);
			}
		}
		let slot = self.slots.read().get(&key).cloned();
		if let Some(slot) = slot {
			if mounted {
				*slot.root.write() = Some(mount_point.to_path_buf());
			}
			let Some(root) = slot.root() else {
				return;
			};
			slot.set_detached(!self.root_attached(&key, &root));
		}
	}

	/// Forget a closing library's registrations.
	///
	/// Drives and their partitions stay: they belong to the machine, another
	/// open library may keep sources on them, and the OS watches armed over
	/// their roots outlive the close, so the registrations those watches
	/// depend on have to as well. Once no library is attached at all the
	/// drives and partitions go too, so a core that closes everything starts
	/// the next library from nothing. A restore, which must not serve the
	/// arena it just replaced, drops its partitions through
	/// [`Self::quiesce_stores`].
	pub fn detach_library(&self, library: Uuid) {
		let mut libraries = self.libraries.lock();
		libraries.retain(|attached| attached.library != Some(library));
		if libraries.iter().all(|attached| attached.library.is_none()) {
			drop(libraries);
			self.volumes.lock().clear();
			self.slots.write().clear();
		}
	}

	/// The registry that holds a source, with the record and its drive.
	fn find_source(&self, id: Uuid) -> Option<Located> {
		let libraries = self.libraries.lock();
		libraries.iter().find_map(|attached| {
			let record = attached.registry.by_id(id)?.clone();
			let (volume, volume_root) = attached.registry.volume_of(&record);
			Some(Located {
				record,
				volume,
				volume_root,
			})
		})
	}

	/// The innermost source owning `path` across every attached library.
	///
	/// Each registry answers with its own innermost match; the deepest root
	/// among them wins, the same rule one registry applies between a source
	/// and one nested inside it.
	fn resolve_source(&self, path: &Path) -> Option<Located> {
		let libraries = self.libraries.lock();
		libraries
			.iter()
			.filter_map(|attached| {
				let record = attached.registry.resolve(path)?.clone();
				let (volume, volume_root) = attached.registry.volume_of(&record);
				Some(Located {
					record,
					volume,
					volume_root,
				})
			})
			.max_by_key(|located| located.record.root.as_os_str().len())
	}

	/// Every registered source across the attached libraries, each with the
	/// library it belongs to and the drive that maps it.
	fn all_sources(&self) -> Vec<(Option<Uuid>, Located)> {
		let libraries = self.libraries.lock();
		libraries
			.iter()
			.flat_map(|attached| {
				attached.registry.all().iter().map(move |record| {
					let (volume, volume_root) = attached.registry.volume_of(record);
					(
						attached.library,
						Located {
							record: record.clone(),
							volume,
							volume_root,
						},
					)
				})
			})
			.collect()
	}

	/// Apply a change to the registry that holds a source, returning what
	/// the registry returned.
	fn update_registry<R>(
		&self,
		id: Uuid,
		update: impl FnOnce(&mut SourceRegistry) -> Option<R>,
	) -> Option<R> {
		let mut libraries = self.libraries.lock();
		let attached = libraries
			.iter_mut()
			.find(|attached| attached.registry.by_id(id).is_some())?;
		update(&mut attached.registry)
	}

	/// Register a root as a source, or refresh an existing registration.
	///
	/// `volume` anchors the source to the drive under it, which is what lets a
	/// remount cost nothing. Pass `None` for a root on media Spacedrive does not
	/// track; the source then stands on its absolute path.
	///
	/// Fails when the registration cannot be written to an open library: a
	/// source that is not durable is one that will not be recognised at next
	/// launch, and the snapshot it goes on to write would then belong to
	/// nothing. With no library open at all there is nothing to write to and
	/// the registration is in-memory by definition, which `persist` reports.
	///
	/// The registration goes to the one attached library. With several open
	/// the caller has to say which, through [`Self::register_source_in`].
	pub async fn register_source(
		&self,
		root: &Path,
		volume: Option<VolumeAnchor>,
	) -> anyhow::Result<Uuid> {
		let library = {
			let libraries = self.libraries.lock();
			let mut attached = libraries.iter().filter_map(|attached| attached.library);
			let library = attached.next();
			if attached.next().is_some() {
				anyhow::bail!("several libraries are open; the registration needs to name one");
			}
			library
		};
		self.register_source_in(library, root, volume).await
	}

	/// [`Self::register_source`] into a named library, or into the session
	/// registry with `None`.
	pub async fn register_source_in(
		&self,
		library: Option<Uuid>,
		root: &Path,
		volume: Option<VolumeAnchor>,
	) -> anyhow::Result<Uuid> {
		Ok(self
			.register_source_with(library, root, volume, None)
			.await?
			.0)
	}

	/// [`Self::register_source_in`], adopting `adopt` as the new source's id
	/// when a store on disk already carries that identity for this root. See
	/// [`Self::portable_identity`]. The flag says whether the source was
	/// already registered.
	pub async fn register_source_with(
		&self,
		library: Option<Uuid>,
		root: &Path,
		volume: Option<VolumeAnchor>,
		adopt: Option<Uuid>,
	) -> anyhow::Result<(Uuid, bool)> {
		let (record, existed, volume, volume_root) = {
			let mut libraries = self.libraries.lock();
			let position = libraries
				.iter()
				.position(|attached| attached.library == library);
			let attached = match position {
				Some(position) => &mut libraries[position],
				None if library.is_some() => {
					anyhow::bail!("library {} is not open", library.unwrap_or_default())
				}
				None => {
					libraries.push(LibrarySources {
						library: None,
						db: None,
						registry: SourceRegistry::default(),
					});
					libraries.last_mut().expect("just pushed")
				}
			};
			let (record, existed) = attached
				.registry
				.register_with(root, volume.as_ref(), adopt);
			let (volume, volume_root) = attached.registry.volume_of(&record);
			(record, existed, volume, volume_root)
		};
		self.persist(&record).await?;

		// The partition belongs to the drive, so registering a source over an
		// already-mapped one joins it rather than starting a second.
		let slot = self.slot_for(&Resolved {
			volume,
			volume_root: volume_root.clone(),
			source: Some(record.clone()),
		});
		*slot.root.write() = Some(volume_root.clone());
		slot.set_detached(!self.root_attached(&slot.volume, &volume_root));
		Ok((record.id, existed))
	}

	/// The directory a source's store lives in on this machine, resolved
	/// from its placement and its current root. `None` for a source this
	/// machine does not register, a cache with no persistence, or an
	/// on-source store whose drive is away.
	pub fn store_dir(&self, id: Uuid) -> Option<PathBuf> {
		let record = self.find_source(id)?.record;
		self.store_dir_of(&record)
	}

	fn store_dir_of(&self, record: &SourceRecord) -> Option<PathBuf> {
		let dirs = self.dirs.as_ref()?;
		super::sources::store_dir(record.config.placement, dirs, record.id, &record.root)
	}

	/// The identity a store already on disk holds for `root`, and where that
	/// store is placed, when one was written by `library_id` for the same
	/// volume and path. Both placements are searched, because a drive that
	/// arrives with its catalog on it is added with whatever the library's
	/// default placement is, and the catalog is the one to reopen whichever
	/// placement the add asked for. When both exist the one under `placement`
	/// wins. `None` when no store binds, which includes a store from another
	/// library or another drive.
	pub async fn portable_identity(
		&self,
		root: &Path,
		volume: Option<&VolumeAnchor>,
		placement: super::sources::StorePlacement,
		library_id: Uuid,
	) -> Option<(Uuid, super::sources::StorePlacement)> {
		use super::sources::StorePlacement;

		let dirs = self.dirs.as_ref()?;
		let (key, anchor) = SourceRegistry::key_for(root, volume);
		let volume_uuid = anchor.map(|a| a.uuid);
		let mut candidates = [StorePlacement::InLibrary, StorePlacement::OnSource];
		if placement == StorePlacement::OnSource {
			candidates.reverse();
		}
		for candidate in candidates {
			let stores_dir = match candidate {
				StorePlacement::InLibrary => dirs.root().to_path_buf(),
				StorePlacement::OnSource => super::sources::on_source_stores_dir(root),
			};
			if let Some((id, _)) = super::descriptor::SourceDescriptor::find_bound(
				&stores_dir,
				library_id,
				volume_uuid,
				&key,
			)
			.await
			{
				return Some((id, candidate));
			}
		}
		None
	}

	/// Write the source's descriptor beside its store, so the store can say
	/// what it is wherever it ends up. Called after every registration and
	/// settings change by the operation that knows which library it acts for.
	pub async fn write_descriptor(&self, id: Uuid, library_id: Uuid) -> anyhow::Result<()> {
		let record = self
			.find_source(id)
			.map(|located| located.record)
			.ok_or_else(|| anyhow::anyhow!("source {id} is not registered"))?;
		let Some(dir) = self.store_dir_of(&record) else {
			return Ok(());
		};
		super::descriptor::SourceDescriptor::for_record(&record, library_id)
			.write(&dir)
			.await
			.map_err(|e| anyhow::anyhow!("write descriptor for {id}: {e}"))
	}

	/// Drop a source's registration and retire its store handles, answering
	/// with the record and the store directory it resolved to.
	///
	/// The store itself stays on disk: removal from the library is not
	/// deletion of the catalog, and the caller decides whether anything is
	/// deleted. The handles go because a writer left open over a directory
	/// the caller may delete, or a later add may reopen under a new
	/// registration, would keep writing into a file nothing reads.
	pub async fn forget_source(&self, id: Uuid) -> Option<(SourceRecord, Option<PathBuf>)> {
		let record = self.update_registry(id, |registry| registry.remove(id))?;
		let dir = self.store_dir_of(&record);
		let store = self.stores.write().remove(&id);
		let reader = self.read_stores.write().remove(&id);
		self.store_open_gates.lock().remove(&id);
		if let Some(store) = store {
			if let Err(error) = store.flush().await {
				tracing::warn!(source = %id, %error, "store released without a clean flush");
			}
			store.db().pool().close().await;
		}
		if let Some(reader) = reader {
			reader.pool().close().await;
		}
		Some((record, dir))
	}

	/// Write a record to the open library, if one is open.
	///
	/// With no library open there is nowhere durable to put it, and the
	/// registration lives only as long as the process. That is legitimate for a
	/// cache serving paths before a library exists, and a silent loss anywhere
	/// else, so it says so.
	async fn persist(&self, record: &SourceRecord) -> anyhow::Result<()> {
		let db = self
			.libraries
			.lock()
			.iter()
			.find(|attached| attached.registry.by_id(record.id).is_some())
			.and_then(|attached| attached.db.clone());
		let Some(db) = db else {
			tracing::warn!(
				source = %record.id,
				root = %record.root.display(),
				"no library open; this registration will not survive the session"
			);
			return Ok(());
		};

		let existing = source::Entity::find()
			.filter(source::Column::Uuid.eq(record.id))
			.one(db.conn())
			.await?;

		let mut row = match existing {
			Some(row) => source::ActiveModel::from(row),
			None => source::ActiveModel {
				uuid: Set(record.id),
				data_type: Set(source::FILESYSTEM_DATA_TYPE.to_string()),
				adapter_id: Set(None),
				config: Set("{}".to_string()),
				status: Set("idle".to_string()),
				// A person's own drive is the authored tier by definition; an
				// adapter's manifest declares its own.
				trust_tier: Set(sd_store::TrustTier::Authored.to_string()),
				created_at: Set(record.created_at),
				..Default::default()
			},
		};

		row.name = Set(record.name.clone());
		row.root = Set(Some(record.relative_root.clone()));
		row.volume_uuid = Set(record.volume_uuid);
		row.record_count = Set(record.record_count.map(|c| c as i64));
		row.directory_count = Set(record.directory_count.map(|c| c as i64));
		row.total_bytes = Set(record.total_bytes.map(|b| b as i64));
		row.content_count = Set(record.content_count.map(|c| c as i64));
		row.unique_bytes = Set(record.unique_bytes.map(|b| b as i64));
		row.config = Set(record.config.to_json());
		row.last_seen_at = Set(record.last_seen_at);

		source::Entity::insert(row)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(source::Column::Uuid)
					.update_columns([
						source::Column::Name,
						source::Column::Root,
						source::Column::VolumeUuid,
						source::Column::RecordCount,
						source::Column::DirectoryCount,
						source::Column::TotalBytes,
						source::Column::ContentCount,
						source::Column::UniqueBytes,
						source::Column::Config,
						source::Column::LastSeenAt,
					])
					.to_owned(),
			)
			.exec(db.conn())
			.await?;

		Ok(())
	}

	/// Every registered source across the attached libraries, with its live
	/// state. A surface that answers for one library asks
	/// [`Self::sources_of`] instead.
	pub fn sources(&self) -> Vec<SourceStatus> {
		self.source_statuses(None)
	}

	/// One library's registered sources with their live state.
	pub fn sources_of(&self, library: Uuid) -> Vec<SourceStatus> {
		self.source_statuses(Some(library))
	}

	fn source_statuses(&self, library: Option<Uuid>) -> Vec<SourceStatus> {
		let located = self.all_sources();
		let slots = self.slots.read();
		located
			.into_iter()
			.filter(|(owner, _)| library.is_none_or(|library| *owner == Some(library)))
			.map(|(owner, located)| {
				let Located { record, volume, .. } = located;
				let slot = slots.get(&volume);
				SourceStatus {
					library: owner,
					attached: self.root_attached(&volume, &record.root),
					restored: slot
						.map(|s| s.restored.load(Ordering::Acquire))
						.unwrap_or(false),
					directory: self.store_dir_of(&record),
					// The hot tier belongs to the map, so it is the drive's.
					thumbs_path: self.dirs.as_ref().map(|d| d.thumbs_file(volume.id())),
					id: record.id,
					root: record.root,
					volume_uuid: record.volume_uuid,
					last_seen_secs: record.last_seen_at.timestamp().max(0) as u64,
					entry_count: record.record_count,
					total_bytes: record.total_bytes,
				}
			})
			.collect()
	}

	/// A source of a library other than `library` whose root is `root`, is
	/// under it, or contains it.
	///
	/// Paths resolve to one source whatever library asks, so two libraries
	/// covering one path would share a store and a capture policy by attach
	/// order. The track action refuses that; this is how it looks.
	pub fn overlapping_source_of_another_library(
		&self,
		library: Uuid,
		root: &Path,
	) -> Option<SourceStatus> {
		// A source whose drive is away has an empty root, and every path
		// starts with the empty path.
		self.sources().into_iter().find(|source| {
			source.library != Some(library)
				&& source.library.is_some()
				&& !source.root.as_os_str().is_empty()
				&& (root.starts_with(&source.root) || source.root.starts_with(root))
		})
	}

	/// Where a source's index snapshot lives on disk. The snapshot belongs to
	/// the drive's volume index rather than the source's own directory, so
	/// resolving it goes through the registry's volume assignment.
	pub fn source_snapshot_path(&self, source_id: Uuid) -> Option<PathBuf> {
		let dirs = self.dirs.as_ref()?;
		let located = self.find_source(source_id)?;
		Some(dirs.snapshot_file(located.volume.id()))
	}

	/// Where the restart cache covering `path` lives, when the path belongs to
	/// a mapped drive. Scratch paths deliberately have no on-disk copy.
	pub fn snapshot_path_for(&self, path: &Path) -> Option<PathBuf> {
		let dirs = self.dirs.as_ref()?;
		let resolved = self.locate(path)?;
		match resolved.volume {
			VolumeKey::Scratch => None,
			_ => Some(dirs.snapshot_file(resolved.volume.id())),
		}
	}

	/// Start mapping a drive. Idempotent; a remount moves its mount point.
	///
	/// Tracking is not registration: nothing appears in the sources list and
	/// nothing is persisted to a store. What it buys is a partition, a
	/// snapshot, and a place for every file on the drive to be found.
	///
	/// For a root that is not where a filesystem mounts: a cloud volume's
	/// prefix, or a directory a test stands in for a drive. A drive detection
	/// found goes through [`Self::track_detected_volume`] so the mount point
	/// check applies to it.
	pub fn track_volume(&self, uuid: Uuid, mount_point: PathBuf) {
		let mut volumes = self.volumes.lock();
		match volumes.iter_mut().find(|tracked| tracked.uuid == uuid) {
			Some(tracked) => {
				tracked.mount_point = mount_point;
				tracked.mounted = true;
			}
			None => volumes.push(TrackedVolume {
				uuid,
				mount_point,
				mounted: true,
				is_mount: false,
			}),
		}
	}

	/// Start mapping a drive detection returned, at the mount point and in
	/// the state detection reports.
	pub fn track_detected_volume(&self, uuid: Uuid, mount_point: PathBuf, mounted: bool) {
		self.track_volume_state(uuid, mount_point, mounted);
	}

	fn track_volume_state(&self, uuid: Uuid, mount_point: PathBuf, mounted: bool) {
		let is_mount = !is_cloud_root(&mount_point);
		let mut volumes = self.volumes.lock();
		match volumes.iter_mut().find(|tracked| tracked.uuid == uuid) {
			Some(tracked) => {
				tracked.mount_point = mount_point;
				tracked.mounted = mounted;
				tracked.is_mount = is_mount;
			}
			None => volumes.push(TrackedVolume {
				uuid,
				mount_point,
				mounted,
				is_mount,
			}),
		}
	}

	/// Which drive a path sits on, and which source keeps it, if any.
	///
	/// A registered source answers both, since it knows its own drive. A path
	/// on a tracked drive with nothing registered over it still has a map to
	/// belong to, which is the case that used to have no answer at all.
	fn locate(&self, path: &Path) -> Option<Resolved> {
		// One acquisition, because a guard held in an `if let` scrutinee lives
		// to the end of the block and taking the lock again inside it is a
		// deadlock rather than a re-entry.
		let located = self.resolve_source(path).map(|located| Resolved {
			volume: located.volume,
			volume_root: located.volume_root,
			source: Some(located.record),
		});

		let volumes = self.volumes.lock();
		// Longest mount point wins, and at the same mount point the drive
		// that is mounted there wins over one that is away: the directory a
		// drive left behind is a window onto whatever is mounted over it. A
		// drive that is away stays resolvable, so its map serves read-only.
		let drive = volumes
			.iter()
			.filter(|tracked| path.starts_with(&tracked.mount_point))
			.max_by_key(|tracked| (tracked.mount_point.as_os_str().len(), tracked.mounted));

		if let Some(located) = located {
			let swapped = match &located.volume {
				VolumeKey::Id(uuid) => drive.is_some_and(|tracked| {
					tracked.uuid != *uuid
						&& tracked.mounted && Self::volume_mounted_locked(&volumes, *uuid)
						== Some(false) && tracked.mount_point.as_os_str().len()
						>= located.volume_root.as_os_str().len()
				}),
				_ => false,
			};
			if !swapped {
				return Some(located);
			}
		}

		drive.map(|tracked| Resolved {
			volume: VolumeKey::Id(tracked.uuid),
			volume_root: tracked.mount_point.clone(),
			source: None,
		})
	}

	/// Get (or lazily create) the live index for a drive. Two sources on
	/// one drive get the same one, and so does a path with no source at all.
	fn slot_for(&self, resolved: &Resolved) -> Arc<Partition> {
		if let Some(slot) = self.slots.read().get(&resolved.volume) {
			return slot.clone();
		}
		let mut slots = self.slots.write();
		slots
			.entry(resolved.volume.clone())
			.or_insert_with(|| {
				let slot =
					Partition::new(resolved.volume.clone(), Some(resolved.volume_root.clone()))
						.expect("create arena for drive");
				slot.set_detached(!self.root_attached(&resolved.volume, &resolved.volume_root));
				slot
			})
			.clone()
	}

	/// Resolve the partition owning `path`: the drive it sits on, else scratch.
	pub fn resolve(&self, path: &Path) -> Arc<Partition> {
		match self.locate(path) {
			Some(resolved) => self.slot_for(&resolved),
			None => self.scratch.clone(),
		}
	}

	/// Resolve a persisted work scope after its drive has moved to another mount.
	pub fn volume_index_root(&self, id: Uuid) -> Option<PathBuf> {
		if let Some(volume) = self
			.volumes
			.lock()
			.iter()
			.find(|volume| VolumeKey::Id(volume.uuid).id() == id)
		{
			return Some(volume.mount_point.clone());
		}
		for source in self.sources() {
			let slot = self.resolve(&source.root);
			if slot.id() == Some(id) && !slot.is_detached() {
				return slot.root();
			}
		}
		None
	}

	/// Why nothing may be dispatched at `path` right now, or `None` when a
	/// walk, hash or thumbnail job over it is allowed.
	///
	/// A detached partition refuses by definition. A partition that reads as
	/// attached is still refused when its drive's mount point is not a mount
	/// point: the stored state can lag a lock or an unmount by a refresh
	/// interval, and the directory left behind must never be walked as the
	/// drive, whatever the state says.
	pub fn dispatch_refusal(&self, path: &Path) -> Option<String> {
		let resolved = self.locate(path)?;
		if is_cloud_root(&resolved.volume_root) {
			return None;
		}
		let slot = self.slot_for(&resolved);
		if slot.is_detached() {
			return Some(format!(
				"{} is on a volume that is not mounted",
				resolved.volume_root.display()
			));
		}
		let is_mount = match &resolved.volume {
			VolumeKey::Id(uuid) => self
				.volumes
				.lock()
				.iter()
				.any(|tracked| tracked.uuid == *uuid && tracked.is_mount),
			_ => false,
		};
		if is_mount && !crate::volume::utils::is_mount_point(&resolved.volume_root) {
			return Some(format!(
				"{} is not a mount point; its volume is not mounted",
				resolved.volume_root.display()
			));
		}
		None
	}

	/// The mapped drive whose mount point is `path` while the drive is away,
	/// by the stored state or by the directory itself.
	///
	/// Tracking that directory would register a second source over the
	/// parent volume, and a listing of it would read as an empty drive.
	pub fn unmounted_volume_at(&self, path: &Path) -> Option<Uuid> {
		let known = self
			.volumes
			.lock()
			.iter()
			.find(|tracked| tracked.is_mount && tracked.mount_point == path)
			.map(|tracked| (tracked.uuid, tracked.mounted))?;
		match known {
			(uuid, false) => Some(uuid),
			(uuid, true) if !crate::volume::utils::is_mount_point(path) => Some(uuid),
			_ => None,
		}
	}

	/// Whether `path` belongs to a detached source (data may be restorable,
	/// but the filesystem underneath is gone — never dispatch indexing).
	pub fn is_detached(&self, path: &Path) -> bool {
		let slot = self.resolve(path);
		slot.is_detached()
	}

	/// The on-disk layout for per-source storage, when this machine keeps one.
	pub fn source_dirs(&self) -> Option<&SourceDirs> {
		self.dirs.as_ref()
	}

	/// How the source owning `path` captures: rules off for an archival
	/// source, the defaults otherwise, including for paths no source owns,
	/// where the background map's policy applies.
	pub fn rule_toggles_for(&self, path: &Path) -> crate::ops::indexing::rules::RuleToggles {
		let unfiltered = self
			.resolve_source(path)
			.map(|located| located.record.config.unfiltered)
			.unwrap_or(false);
		if unfiltered {
			crate::ops::indexing::rules::RuleToggles::none()
		} else {
			crate::ops::indexing::rules::RuleToggles::default()
		}
	}

	/// A registered source's display name, by id.
	pub fn source_name(&self, id: Uuid) -> Option<String> {
		self.find_source(id).map(|located| located.record.name)
	}

	/// A registered source's capture policy, by id.
	pub fn source_config(&self, id: Uuid) -> Option<SourceConfig> {
		self.find_source(id).map(|located| located.record.config)
	}

	/// Update a source's settings, persisting the change. An error means the
	/// row did not take the change; the in-memory record did, so the caller
	/// reports the add or update as failed rather than as saved.
	pub async fn set_source_config(&self, id: Uuid, config: SourceConfig) -> anyhow::Result<()> {
		let updated = self.update_registry(id, |registry| registry.set_config(id, config));
		match updated {
			Some(updated) => self.persist(&updated).await,
			None => anyhow::bail!("source {id} is not registered"),
		}
	}

	/// Rename a source, persisting the change.
	pub async fn set_source_name(&self, id: Uuid, name: String) -> anyhow::Result<()> {
		let updated = self.update_registry(id, |registry| registry.set_name(id, name));
		match updated {
			Some(updated) => self.persist(&updated).await,
			None => anyhow::bail!("source {id} is not registered"),
		}
	}

	/// The root of the source owning `path`, when one does.
	pub fn source_root_for(&self, path: &Path) -> Option<PathBuf> {
		self.resolve_source(path).map(|located| located.record.root)
	}

	/// The id of the source owning `path`, when one does. The innermost
	/// registered source wins, as it does for [`Self::store_for`].
	pub fn source_id_for(&self, path: &Path) -> Option<Uuid> {
		self.resolve_source(path).map(|located| located.record.id)
	}

	/// A registered source's current absolute root, by id.
	///
	/// The registry is the authority: the library row stores the root
	/// relative to its volume, so anything that turns a source id into a
	/// path to open must ask here rather than read the row.
	pub fn source_root(&self, id: Uuid) -> Option<PathBuf> {
		self.find_source(id).map(|located| located.record.root)
	}

	/// The durable store that should hold `path`, opened on first use.
	///
	/// The innermost registered source wins, which is what makes a source
	/// inside another persist its own subtree without the outer one writing it
	/// twice. Keyed by source rather than by drive for the same reason: one
	/// drive, one map, any number of things kept off it.
	///
	/// `None` for a path under no source and for a cache with no sources
	/// directory. Browsing keeps working in both cases, which is what makes the
	/// arena the read path and the store an addition to it.
	pub async fn store_for(&self, path: &Path) -> Option<Arc<SourceStore>> {
		let record = self.resolve_source(path)?.record;
		let store_dir = self.store_dir_of(&record)?;
		let stores_dir = store_dir.parent()?.to_path_buf();

		if let Some(store) = self.stores.read().get(&record.id) {
			return Some(store.clone());
		}

		// One open per source, even for concurrent first callers. Without the
		// gate, simultaneous callers each open the pool, load the ledger, and
		// spawn a writer task before one wins the map insertion; the losers'
		// writers run until their queues drop.
		let gate = {
			let mut gates = self.store_open_gates.lock();
			gates
				.entry(record.id)
				.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
				.clone()
		};
		let _open = gate.lock().await;

		if let Some(store) = self.stores.read().get(&record.id) {
			return Some(store.clone());
		}

		let store = match SourceStore::open(&stores_dir, record.id, record.root.clone()).await {
			Ok(store) => store,
			Err(error) => {
				tracing::error!(source = %record.id, %error, "source store unavailable");
				return None;
			}
		};

		self.stores.write().insert(record.id, store.clone());
		Some(store)
	}

	/// Every writable store this machine has opened so far.
	pub fn open_stores(&self) -> Vec<Arc<SourceStore>> {
		self.stores.read().values().cloned().collect()
	}

	/// A read-only handle to a source's store, cached per source.
	///
	/// `None` means the source has no store on disk or it refused to open,
	/// which includes a generation too old to address; nothing is created on
	/// this path. The open shares the per-source gate with [`Self::store_for`]
	/// so a read never races a writer's first open.
	pub async fn read_store(&self, source_id: Uuid) -> Option<Arc<sd_store::SourceDb>> {
		if let Some(db) = self.read_stores.read().get(&source_id) {
			return Some(db.clone());
		}
		let dirs = self.dirs.as_ref()?;

		let gate = {
			let mut gates = self.store_open_gates.lock();
			gates
				.entry(source_id)
				.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
				.clone()
		};
		let _open = gate.lock().await;

		if let Some(db) = self.read_stores.read().get(&source_id) {
			return Some(db.clone());
		}

		// A registered source reads from wherever its placement put the
		// store; one this machine does not register, a replica being
		// inspected by id, reads from the in-library layout.
		let stores_dir = self
			.find_source(source_id)
			.and_then(|located| self.store_dir_of(&located.record))
			.and_then(|dir| dir.parent().map(Path::to_path_buf))
			.unwrap_or_else(|| dirs.root().to_path_buf());
		let manager = sd_store::SourceManager::new(stores_dir);
		match manager
			.open_read_only(&source_id.simple().to_string())
			.await
		{
			Ok(db) => {
				let db = Arc::new(db);
				self.read_stores.write().insert(source_id, db.clone());
				Some(db)
			}
			Err(error) => {
				tracing::debug!(source = %source_id, %error, "no readable store for source");
				None
			}
		}
	}

	/// The partitions and drive snapshots a restore of these sources has to
	/// clear, resolved while their library is still attached.
	///
	/// Resolution goes through the registry, and closing the library takes
	/// its registry away, so a restore asks this before it closes the
	/// library and hands the answer to [`Self::quiesce_stores`] after.
	pub fn quiesce_targets(&self, ids: &[Uuid]) -> QuiesceTargets {
		let mut partitions: Vec<VolumeKey> = ids
			.iter()
			.filter_map(|id| self.find_source(*id))
			.map(|located| located.volume)
			.collect();
		partitions.sort_by_key(|key| key.id());
		partitions.dedup();
		let mut snapshots: Vec<PathBuf> = self
			.dirs
			.iter()
			.flat_map(|dirs| partitions.iter().map(|key| dirs.snapshot_file(key.id())))
			.collect();
		snapshots.sort();
		snapshots.dedup();
		QuiesceTargets {
			partitions,
			snapshots,
		}
	}

	/// Close every handle on these sources' stores and keep them closed until
	/// the returned hold drops, so the files can be replaced on disk.
	///
	/// A restore swaps `data.db` underneath a running daemon. A pool still
	/// open on the old inode would keep writing to a file nothing reads any
	/// more, and a watcher event arriving mid-swap would reopen the old file
	/// and cache that handle. So the per-source open gates are taken first,
	/// which parks every `store_for` and `read_store` caller behind the hold,
	/// then the cached handles are flushed, dropped and closed. The drive
	/// partitions mapping these sources are dropped, whether or not another
	/// open library shares the drive, and the drive snapshots covering them
	/// are removed, so nothing in memory or on disk outlives the files it
	/// indexed: the reopened library re-adopts its sources and rebuilds the
	/// arena from the restored stores. The library's own registrations are
	/// the close's to take away. Returns the hold and the snapshot files
	/// removed.
	pub async fn quiesce_stores(
		&self,
		ids: &[Uuid],
		targets: &QuiesceTargets,
	) -> (StoreHold, Vec<PathBuf>) {
		// One gate per source; locking the same gate twice would wait on
		// itself.
		let mut ids = ids.to_vec();
		ids.sort();
		ids.dedup();
		let ids = &ids;
		let gates: Vec<Arc<tokio::sync::Mutex<()>>> = {
			let mut all = self.store_open_gates.lock();
			ids.iter()
				.map(|id| {
					all.entry(*id)
						.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
						.clone()
				})
				.collect()
		};
		let mut guards = Vec::with_capacity(gates.len());
		for gate in gates {
			guards.push(gate.lock_owned().await);
		}

		let writers: Vec<Arc<SourceStore>> = {
			let mut stores = self.stores.write();
			ids.iter().filter_map(|id| stores.remove(id)).collect()
		};
		let readers: Vec<Arc<sd_store::SourceDb>> = {
			let mut stores = self.read_stores.write();
			ids.iter().filter_map(|id| stores.remove(id)).collect()
		};
		for store in writers {
			if let Err(error) = store.flush().await {
				tracing::warn!(source = %store.id(), %error, "store released without a clean flush");
			}
			store.db().pool().close().await;
		}
		for db in readers {
			db.pool().close().await;
		}

		// The partition goes so nothing restores the replaced arena from
		// memory; its watch registrations stay, because the OS watch the
		// service armed over each root survives the swap and the handler
		// drops every event from a root the index no longer lists. The
		// fresh arena is empty until the restore re-walks the sources, which
		// is what fills it under the watched roots again.
		{
			let mut slots = self.slots.write();
			for key in &targets.partitions {
				let Some(old) = slots.remove(key) else {
					continue;
				};
				match Partition::new(key.clone(), old.root()) {
					Ok(fresh) => {
						*fresh.watched_paths.write() = old.watched_paths.read().clone();
						fresh.set_detached(old.is_detached());
						slots.insert(key.clone(), fresh);
					}
					Err(error) => {
						tracing::warn!(%error, "could not replace a quiesced drive partition");
					}
				}
			}
		}
		let mut removed = Vec::new();
		for path in &targets.snapshots {
			match tokio::fs::remove_file(path).await {
				Ok(()) => removed.push(path.clone()),
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
				Err(error) => {
					tracing::warn!(path = %path.display(), %error, "stale drive snapshot was not removed");
				}
			}
		}
		(StoreHold { _guards: guards }, removed)
	}

	/// The mount point of the volume a path resolves to. A source whose root
	/// differs from this is nested inside its volume, which is what decides
	/// whether its replica travels as its own database or as the volume's
	/// arena snapshot.
	pub fn volume_root_of(&self, path: &Path) -> Option<PathBuf> {
		self.locate(path).map(|resolved| resolved.volume_root)
	}

	/// Whether a loaded arena can answer a whole-scope query at this path:
	/// its partition restored from a snapshot, or a walk this session covers
	/// the path. A partition that merely exists is not an answer — an empty
	/// result from it would read as an empty source rather than an unloaded
	/// one, which is exactly the distinction the store fallback exists for.
	pub fn arena_answers(&self, path: &Path) -> bool {
		let Some(resolved) = self.locate(path) else {
			return false;
		};
		let slots = self.slots.read();
		let Some(slot) = slots.get(&resolved.volume) else {
			return false;
		};
		slot.restored.load(Ordering::Acquire)
			|| slot
				.indexed_paths
				.read()
				.iter()
				.any(|indexed| path.starts_with(indexed))
	}

	/// Every open store on this machine, one per source.
	///
	/// A detached drive is included: its records are still true, and a question
	/// about what exists is answerable while the drive is in a drawer even
	/// though a question about opening a file is not.
	pub async fn stores(&self) -> Vec<Arc<SourceStore>> {
		let roots: Vec<PathBuf> = self
			.all_sources()
			.into_iter()
			.map(|(_, located)| located.record.root)
			.collect();

		let mut stores = Vec::with_capacity(roots.len());
		for root in roots {
			if let Some(store) = self.store_for(&root).await {
				stores.push(store);
			}
		}
		stores
	}

	/// Where a record uuid lives, asked of every partition.
	///
	/// A uuid carries no path to route by, so this is a scan. It is over
	/// in-memory maps rather than the filesystem, which is what makes it
	/// affordable.
	pub async fn path_of_record(&self, record_uuid: Uuid) -> Option<PathBuf> {
		for index in self.all_indexes() {
			let index = index.read().await;
			if let Some(path) = index.get_path_by_uuid(record_uuid) {
				return Some(path);
			}
		}
		None
	}

	/// The identity of the bytes behind a record, asked of whichever source
	/// holds it.
	///
	/// `None` means either that no source has this record or that its bytes
	/// have not been hashed yet, and the caller cannot tell those apart because
	/// the answer is the same either way: there is nothing to key content by.
	pub async fn content_of(&self, record_uuid: Uuid) -> Option<Uuid> {
		for store in self.stores().await {
			if let Some(content) = store.content_of(record_uuid).await {
				return Some(content);
			}
		}
		None
	}

	/// Every copy of the given bytes this machine holds, across all sources.
	///
	/// Deduplicated by record, since a caller wants each copy once and a source
	/// nested inside another can report the same record twice.
	pub async fn copies_of_content(&self, content_uuid: Uuid) -> Vec<DuplicateCopy> {
		let mut seen = HashSet::new();
		let mut copies = Vec::new();

		for store in self.stores().await {
			for copy in store.copies_of_content(content_uuid).await {
				if seen.insert(copy.record_uuid) {
					copies.push(copy);
				}
			}
		}
		copies
	}

	/// The index owning `path`, unconditionally (scratch fallback).
	pub fn resolve_index(&self, path: &Path) -> Arc<TokioRwLock<Arena>> {
		self.resolve(path).index()
	}

	/// The watched root that should receive a change at `path`, if any.
	///
	/// Two questions, and the second is the one that was missing. Territory:
	/// which watched root contains this path, longest match winning. Then
	/// depth: does the index hold the directory the change landed in.
	///
	/// Depth belongs to the index rather than to the watch. A source is walked
	/// to whatever depth it was walked to, so requiring a change to sit
	/// directly under a watched root only ever worked for a browse of a single
	/// directory. A source rooted at the whole drive registers one watched path
	/// and every change arrives from somewhere below it. Asking whether the
	/// parent is indexed admits those, and still refuses a change under a
	/// directory nothing walked, which would otherwise graft a second tree
	/// beside the real one.
	pub async fn watched_root_for_change(&self, path: &Path) -> Option<PathBuf> {
		let parent = path.parent()?;
		let root = self.find_watched_root(path)?;

		let index = self.resolve_index(parent);
		let index = index.read().await;

		// A change under a summarised directory has no tree to land in, and
		// dropping it would leave the drive's totals quietly wrong for as long
		// as the session lasts. Climbing to the nearest ancestor the index does
		// hold is what finds the directory standing in for it.
		//
		// The watched root is held by definition: it was walked to completion.
		// A walk that found nothing under it wrote no entry for it, and the
		// writer creates that entry with the first change to land there.
		for ancestor in parent.ancestors() {
			if ancestor != root && index.get_entry_ref(&ancestor.to_path_buf()).is_none() {
				continue;
			}
			if index.is_summarised(ancestor) {
				self.dirty_stubs.lock().insert(ancestor.to_path_buf());
				return None;
			}
			return (ancestor == parent).then_some(root);
		}

		None
	}

	/// The summarised directories something has changed under since this was
	/// last asked, emptied as it answers.
	///
	/// Recounting them is somebody else's job: a change to the arena has to
	/// reach the clients drawing it, and this cache has no way to tell them.
	pub fn take_dirty_stubs(&self) -> Vec<PathBuf> {
		std::mem::take(&mut *self.dirty_stubs.lock())
			.into_iter()
			.collect()
	}

	/// Every live index, scratch included. For global lookups (uuid → entry)
	/// and aggregate stats.
	pub fn all_indexes(&self) -> Vec<Arc<TokioRwLock<Arena>>> {
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
	pub fn get_for_path(&self, path: &Path) -> Option<Arc<TokioRwLock<Arena>>> {
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
	pub fn get_for_search(&self, path: &Path) -> Option<Arc<TokioRwLock<Arena>>> {
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

	/// Whether `path` or a directory above it is being indexed. A recursive
	/// walk or a store fill in progress over an ancestor covers everything
	/// beneath it, and a browse dispatched into that tree would clear what
	/// the fill has already placed there.
	pub fn is_under_indexing(&self, path: &Path) -> bool {
		self.resolve(path)
			.indexing_in_progress
			.read()
			.iter()
			.any(|indexing| path.starts_with(indexing))
	}

	/// Restore a registered source's snapshot into its partition, if it has
	/// one and hasn't been restored this session. Returns true when the
	/// source's data is available afterwards (restored now or already live).
	///
	/// Detached sources restore read-only: their entries become queryable but
	/// `is_detached` stays true so nothing dispatches indexing at them.
	pub async fn ensure_restored(&self, path: &Path) -> bool {
		let Some(resolved) = self.locate(path) else {
			return false;
		};
		let slot = self.slot_for(&resolved);

		// Exactly one restore attempt per session, shared by all callers.
		// Data written by jobs before/while the attempt runs is merged over
		// afterwards by those jobs' own writes, never silently replaced.
		let already_restored = slot.restored.load(Ordering::Acquire);
		let root = resolved.volume_root.clone();
		let mounted = match &resolved.volume {
			VolumeKey::Id(uuid) => self.volume_mounted(*uuid).unwrap_or(true),
			_ => true,
		};
		let outcome = *slot
			.restore_once
			.get_or_init(|| async {
				// A library restore fills the drive's fresh partition through
				// `rebuild_quiesced` before anything asks here; reading the
				// stores again would only repeat it.
				if slot.restored.load(Ordering::Acquire) {
					return RestoreOutcome::Stores;
				}
				if Self::attempt_restore(self.dirs.clone(), slot.clone(), mounted).await {
					return RestoreOutcome::Snapshot;
				}
				// A missing, quarantined or outdated snapshot costs a walk of
				// every source on the drive, and every format bump produces
				// exactly that. The stores hold the same records with the same
				// uuids, so they refill the arena instead, off this request:
				// every reader on the drive funnels through this gate, so the
				// fill runs on its own task and each source announces itself
				// as it lands, which flips its routing and arms its watch. A
				// detached drive keeps nothing to rebuild for.
				if slot.is_detached() || !self.rebuild_sources_from_stores(&slot).await {
					return RestoreOutcome::Nothing;
				}
				RestoreOutcome::Stores
			})
			.await;

		if outcome == RestoreOutcome::Snapshot && !already_restored {
			self.announce_restored(&root);
		}

		outcome != RestoreOutcome::Nothing || !slot.indexed_paths.read().is_empty()
	}

	/// Whether the drive under `path` was restored from its snapshot this
	/// session. A store rebuild does not count: it covers the registered
	/// sources and nothing else on the drive.
	pub fn restored_from_snapshot(&self, path: &Path) -> bool {
		let Some(resolved) = self.locate(path) else {
			return false;
		};
		let slots = self.slots.read();
		slots
			.get(&resolved.volume)
			.and_then(|slot| slot.restore_once.get())
			.is_some_and(|outcome| *outcome == RestoreOutcome::Snapshot)
	}

	/// Start rebuilding every attached source on a drive partition from its
	/// store, on one background task, and say whether anything was started.
	///
	/// Only a source whose row records a count is rebuilt. The count is
	/// written when a snapshot is saved, so it is the evidence that a map
	/// existed and was lost; a source nothing has walked to completion keeps
	/// answering from its store without an arena, as R6 routes it, and a
	/// source with no readable store is left for the coverage heal to walk.
	///
	/// Each root is marked in progress until its fill lands, so the heal
	/// does not dispatch a walk over it, a listing serves its store in the
	/// meantime, and the status surface shows the work. The stores are
	/// opened here, before the task starts, so the caller learns at once
	/// whether the drive has anything to rebuild from.
	async fn rebuild_sources_from_stores(&self, slot: &Arc<Partition>) -> bool {
		let mut pending = Vec::new();
		for (_, located) in self.all_sources() {
			if located.volume != slot.volume
				|| located.record.record_count.unwrap_or(0) == 0
				|| !self.root_attached(&located.volume, &located.record.root)
			{
				continue;
			}
			let db = self.read_store(located.record.id).await;
			let first_row = match &db {
				Some(db) => sd_store::read::rebuild_entries_page(db.pool(), 0, 1)
					.await
					.map(|(rows, _)| !rows.is_empty())
					.unwrap_or(false),
				None => false,
			};
			let (Some(db), true) = (db, first_row) else {
				tracing::warn!(
					source = %located.record.id,
					root = %located.record.root.display(),
					"no usable snapshot and no store records; the source will be walked"
				);
				continue;
			};
			slot.indexing_in_progress
				.write()
				.insert(located.record.root.clone());
			pending.push((located.record.id, located.record.root, db));
		}
		if pending.is_empty() {
			return false;
		}

		let slot = slot.clone();
		let announce = self.restored_roots.read().clone();
		tokio::spawn(async move {
			for (source_id, root, db) in pending {
				let started = Instant::now();
				match fill_source_from_store(&slot, &db, &root).await {
					Some(loaded) if loaded > 0 => {
						slot.indexing_in_progress.write().remove(&root);
						if let Some(sender) = &announce {
							let _ = sender.send(root.clone());
						}
						tracing::info!(
							source = %source_id,
							root = %root.display(),
							loaded,
							took = ?started.elapsed(),
							"no usable snapshot; map rebuilt from the source store"
						);
					}
					_ => {
						slot.indexing_in_progress.write().remove(&root);
						tracing::warn!(
							source = %source_id,
							root = %root.display(),
							"no usable snapshot and no store records; the source will be walked"
						);
					}
				}
			}
		});
		true
	}

	/// Rebuild a source's map from its store, without walking the disk.
	///
	/// A restore drops the drive partition and the snapshot that would
	/// refill it, and a walk would re-hash what the store already knows. The
	/// store holds exactly the source's records, so they are read back into
	/// the arena, the root is marked indexed and announced the way a snapshot
	/// restore announces it, which is what arms the watch over it again.
	/// Returns how many entries were loaded, or `None` when the source has
	/// no readable store. A store with no records marks nothing: an empty
	/// arena has no parent to file a change under, so the root is left for
	/// a walk to claim.
	pub async fn rebuild_from_store(&self, source_id: Uuid) -> Option<usize> {
		let root = self.source_root(source_id)?;
		let db = self.read_store(source_id).await?;
		let slot = self.resolve(&root);
		let loaded = fill_source_from_store(&slot, &db, &root).await?;
		if loaded > 0 {
			self.announce_restored(&root);
		}
		Some(loaded)
	}

	/// Rebuild the map of every attached source whose drive partition a
	/// restore dropped, whichever library holds it.
	///
	/// A partition is the drive's, so quiescing one library's sources takes
	/// the arena away from every other open library's sources on that
	/// drive too; they get theirs back from their own stores here. Returns
	/// how many sources were rebuilt.
	pub async fn rebuild_quiesced(&self, targets: &QuiesceTargets) -> usize {
		let mut rebuilt = 0usize;
		for (_, located) in self.all_sources() {
			if !targets.partitions.contains(&located.volume)
				|| !self.root_attached(&located.volume, &located.record.root)
			{
				continue;
			}
			match self.rebuild_from_store(located.record.id).await {
				Some(loaded) => {
					rebuilt += 1;
					tracing::debug!(source = %located.record.id, loaded, "map rebuilt from its store");
				}
				None => tracing::warn!(
					source = %located.record.id,
					"source has no readable store; its map stays empty until walked"
				),
			}
		}
		rebuilt
	}

	/// Restore every drive this machine maps, and every source registered over
	/// one, so a query that fans out across partitions sees all of them.
	///
	/// A query that wants the whole picture cannot ask the sources list for it
	/// any more. A machine can map several drives and keep nothing, which is the
	/// default now, and the collections a person sees are on the drives rather
	/// than in the registrations.
	pub async fn restore_everything(&self) {
		let mut roots: Vec<PathBuf> = self
			.volumes
			.lock()
			.iter()
			.map(|tracked| tracked.mount_point.clone())
			.collect();
		roots.extend(
			self.all_sources()
				.into_iter()
				.map(|(_, located)| located.record)
				.filter(|source| source.is_locatable())
				.map(|source| source.root),
		);

		for root in roots {
			self.ensure_restored(&root).await;
		}
	}

	/// Receive the root of every source whose index becomes browsable from a
	/// snapshot. One subscriber; a second call replaces the first.
	pub fn subscribe_restored_roots(&self) -> mpsc::UnboundedReceiver<PathBuf> {
		let (tx, rx) = mpsc::unbounded_channel();
		*self.restored_roots.write() = Some(tx);
		rx
	}

	fn announce_restored(&self, root: &Path) {
		let sender = self.restored_roots.read().clone();
		if let Some(sender) = sender {
			let _ = sender.send(root.to_path_buf());
		}
	}

	/// The single restore attempt for a slot. Returns whether the snapshot
	/// was loaded; the result is cached by `restore_once` for the session.
	///
	/// `mounted` is what the caller knows about the drive; the snapshot's root
	/// existing on disk is not enough on its own, since an unmounted drive
	/// leaves its mount point behind as an empty directory.
	async fn attempt_restore(
		dirs: Option<SourceDirs>,
		slot: Arc<Partition>,
		mounted: bool,
	) -> bool {
		let Some(dirs) = dirs else {
			return false;
		};
		// The snapshot is the arena's durable copy, so it belongs to the drive
		// rather than to whatever is registered over it.
		let volume_index_id = slot.volume.id();
		let Some(volume_root) = slot.root() else {
			return false;
		};
		let snapshot_path = dirs.snapshot_file(volume_index_id);
		let loaded = match Arena::load_snapshot(&snapshot_path) {
			Ok(Some((index, meta))) => Some((index, meta)),
			Ok(None) => None,
			Err(err) => {
				tracing::warn!(
					"Snapshot restore failed for {}: {err}",
					volume_root.display()
				);
				None
			}
		};
		let Some((loaded_index, meta)) = loaded else {
			return false;
		};

		// The snapshot names the drive it was taken for. The file is keyed by
		// volume index id in its path, so a mismatch means the cache directory was
		// copied or edited from outside; adopting it would bind one drive's
		// contents to another's identity.
		if meta.source_id != volume_index_id {
			tracing::warn!(
				"Snapshot at {} belongs to volume index {}, not {}; ignoring",
				snapshot_path.display(),
				meta.source_id,
				volume_index_id
			);
			return false;
		}

		// A drive that came back at a different mount point has absolute
		// paths from the old mount baked into the snapshot. Reindexing the
		// present drive is cheaper than being subtly wrong, but a root
		// disagreement can also mean this resolver picked a different root
		// than the one that wrote the snapshot. The artifact moves aside
		// rather than being deleted, so a resolution bug cannot destroy the
		// only copy; the next save still lands clean at the original path.
		if volume_root.exists() && meta.root_path != volume_root {
			let aside = snapshot_path.with_extension("mismatched-root");
			tracing::warn!(
				"Snapshot for {} was taken at {}; moving aside to {} for reindex",
				volume_root.display(),
				meta.root_path.display(),
				aside.display()
			);
			if aside.exists() {
				// A repeating mismatch keeps regenerating snapshots; the one
				// already aside is the oldest and stays. This copy was written
				// after the disagreement began and adds nothing.
				let _ = std::fs::remove_file(&snapshot_path);
			} else if let Err(e) = std::fs::rename(&snapshot_path, &aside) {
				tracing::warn!(
					"Could not move mismatched snapshot {} aside: {e}",
					snapshot_path.display()
				);
			}
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
		slot.set_detached(!(mounted && meta.root_path.exists()));

		tracing::info!(
			"Restored volume index {} from snapshot ({}, {})",
			volume_index_id,
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

	/// Save the owning partition to its drive's snapshot file. A path on no
	/// tracked drive has no snapshot and skips silently.
	pub async fn save_snapshot(&self, path: &Path) -> anyhow::Result<()> {
		let Some(resolved) = self.locate(path) else {
			tracing::debug!("No tracked drive for {}; skipping snapshot", path.display());
			return Ok(());
		};
		let Some(dirs) = &self.dirs else {
			return Ok(());
		};
		let slot = self.slot_for(&resolved);
		let volume_index_id = slot.volume.id();
		let volume_root = resolved.volume_root.clone();
		let snapshot_path = dirs.snapshot_file(volume_index_id);

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
				volume_root.display()
			);
			return Ok(());
		}

		let _save_guard = slot.save_lock.lock().await;
		let (entry_count, total_bytes) = {
			let mut index = slot.index.write().await;
			let stats = index.get_stats();
			let entry_count = stats.total_entries as u64;

			// The snapshot is the arena's only copy, and more than one path can
			// leave a partition holding a single browsed directory: a whole
			// drive's index has been replaced by the twenty entries of the
			// folder someone happened to open. A collapse of this size is that
			// bug rather than a drive that actually emptied, so refuse it and
			// name both counts. The records themselves are in the source store,
			// so what this costs is a re-index; what it prevents is a silent
			// overwrite that a re-index cannot undo.
			//
			// The guard above this one only stops a session that never restored
			// from clobbering. It exempts any partition that has already saved
			// once, which is exactly when a full index is present to lose.
			let previous = match slot.last_saved_entries.load(Ordering::Acquire) {
				u64::MAX => resolved.source.as_ref().and_then(|s| s.record_count),
				saved => Some(saved),
			};
			if let Some(previous) = previous {
				if previous >= COLLAPSE_FLOOR
					&& entry_count.saturating_mul(COLLAPSE_FACTOR) < previous
				{
					tracing::error!(
						"Refusing to save snapshot for {}: {} entries would replace {}. \
						 The partition lost its contents without the drive emptying; \
						 re-index to rebuild it.",
						volume_root.display(),
						entry_count,
						previous
					);
					return Ok(());
				}
			}

			// Entry count cannot answer "did anything change": a rename, or a
			// delete balanced by an add, leaves it identical while changing what
			// has to persist. The index tracks its own mutations instead.
			if snapshot_path.exists() && !index.is_dirty() {
				tracing::debug!(
					"Snapshot for {} unchanged ({} entries); skipping rewrite",
					volume_root.display(),
					entry_count
				);
				return Ok(());
			}
			index.save_snapshot(&snapshot_path, volume_index_id, &volume_root)?;
			index.clear_dirty();
			slot.last_saved_entries
				.store(entry_count, Ordering::Release);
			(entry_count, stats.total_file_bytes)
		};
		// Persist counts on the registry row so listings can show a size without
		// loading a snapshot. What a source reports is what its store holds,
		// not what the partition around it does: the arena maps the whole
		// drive, and a source is a scope over part of it. The arena's figures
		// stand in only when a source spans its whole drive, which is the case
		// that has no store to ask yet.
		//
		// A store holding nothing has not been written yet, and a source over an
		// already-mapped drive is in exactly that state until its adoption walk
		// runs. Reporting zero there would empty a listing that has a full drive
		// behind it, so the partition's figure stands until the store has one of
		// its own.
		//
		// A drive with nothing registered over it has no row to write to, which
		// is the ordinary case for a machine that is mapped but keeps nothing.
		let Some(record) = resolved.source else {
			tracing::info!(
				"Saved snapshot for volume index {} ({})",
				volume_index_id,
				volume_root.display()
			);
			return Ok(());
		};

		// The store's own count is the durable one. The arena's is the fallback
		// for a source whose store has not been written yet, and it can only
		// answer two of the four.
		let counts = match self.store_for(&record.root).await {
			Some(store) => store.counts().await.filter(|counts| counts.records > 0),
			None => None,
		}
		.unwrap_or(crate::ops::indexing::SourceCounts {
			records: entry_count,
			bytes: total_bytes,
			..Default::default()
		});

		// The snapshot itself is already on disk, so a failure here costs a
		// stale count in listings rather than the index: report it and keep the
		// save successful.
		let updated = self.update_registry(record.id, |registry| {
			registry.update_stats(record.id, counts)
		});
		if let Some(updated) = updated {
			if let Err(err) = self.persist(&updated).await {
				tracing::error!(
					"Saved snapshot for source {} but could not persist its counts: {err}",
					record.id
				);
			}
		}
		tracing::info!(
			"Saved snapshot for source {} ({})",
			record.id,
			record.root.display()
		);
		Ok(())
	}

	/// Prepare the owning partition for indexing a new path.
	pub fn create_for_indexing(&self, path: PathBuf) -> Arc<TokioRwLock<Arena>> {
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

	fn fold_slots<T>(&self, mut f: impl FnMut(&Partition) -> T) -> Vec<T> {
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
		if slot.is_detached() {
			return false;
		}
		// A walked-to-completion root and a registered source root are both
		// watchable. `indexed_paths` is session state: a source nested inside
		// its volume's partition is not re-listed there by the partition's
		// restore, so after a restart the registration is the only durable
		// evidence the root deserves a watch. Events over a sparser arena
		// still file correctly; the writer synthesizes missing ancestors.
		let indexed = slot.indexed_paths.read().contains(&path);
		let registered = self
			.libraries
			.lock()
			.iter()
			.flat_map(|attached| attached.registry.all())
			.any(|record| record.root == path && record.root.exists());
		if !indexed && !registered {
			return false;
		}
		self.refused_watches.lock().remove(&path);
		slot.watched_paths.write().insert(path);
		true
	}

	pub fn unregister_from_watching(&self, path: &Path) {
		self.resolve(path).watched_paths.write().remove(path);
		self.refused_watches.lock().remove(path);
	}

	/// Record that the OS refused to watch a root, so status reports the
	/// failure instead of an active watch and the watcher knows what to
	/// retry.
	pub fn record_watch_refusal(&self, path: PathBuf, reason: String) {
		self.resolve(&path).watched_paths.write().remove(&path);
		self.refused_watches.lock().insert(path, reason);
	}

	/// Roots whose watch was refused, with the reason, sorted by path.
	pub fn refused_watches(&self) -> Vec<(PathBuf, String)> {
		let mut refused: Vec<(PathBuf, String)> = self
			.refused_watches
			.lock()
			.iter()
			.map(|(path, reason)| (path.clone(), reason.clone()))
			.collect();
		refused.sort();
		refused
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
		let old_slots: Vec<Arc<Partition>> = {
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
			self.refused_watches.lock().clear();
			watched.clear();
		}
		{
			let mut index = self.scratch.index.write().await;
			*index = Arena::new().expect("Failed to create new arena");
		}

		cleared
	}

	pub fn stats(&self) -> VolumeIndexStats {
		VolumeIndexStats {
			indexed_paths: self.len(),
			indexing_in_progress: self
				.fold_slots(|s| s.indexing_in_progress.read().len())
				.into_iter()
				.sum(),
			watched_paths: self
				.fold_slots(|s| s.watched_paths.read().len())
				.into_iter()
				.sum(),
			sources: self
				.libraries
				.lock()
				.iter()
				.map(|attached| attached.registry.all().len())
				.sum(),
		}
	}

	pub fn age(&self) -> std::time::Duration {
		self.created_at.elapsed()
	}

	pub fn get_age(&self, _path: &Path) -> Option<f64> {
		Some(self.created_at.elapsed().as_secs_f64())
	}
}

impl Default for VolumeIndex {
	fn default() -> Self {
		Self::new().expect("Failed to create default VolumeIndex")
	}
}

/// Statistics about the volume index
#[derive(Debug, Clone)]
pub struct VolumeIndexStats {
	pub indexed_paths: usize,
	pub indexing_in_progress: usize,
	pub watched_paths: usize,
	pub sources: usize,
}

impl VolumeIndexStats {
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

	/// The one library most tests attach; a second one is minted where a
	/// test is about two.
	const LIBRARY: Uuid = Uuid::from_u128(0x1);

	fn isolated_cache() -> VolumeIndex {
		VolumeIndex::with_sources_dir(None).expect("failed to create cache")
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

	/// A library database for tests that rebuild the cache: registrations live
	/// in `library.db`, so a session boundary needs one to cross.
	async fn test_library(dir: &Path) -> Arc<Database> {
		let db = Database::create(&dir.join("library.db"))
			.await
			.expect("create library");
		db.migrate().await.expect("migrate");
		Arc::new(db)
	}

	/// A source anchors to a volume, and its absolute root is rebuilt from
	/// that volume's mount point on every attach. A test that skips this
	/// gets a source with no root after a restart, which is a different
	/// bug than the one being measured.
	async fn tracked_volume(db: &Arc<Database>, mount_point: &Path) -> VolumeAnchor {
		use crate::infra::db::entities::volume;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set};

		use crate::infra::db::entities::device;

		// `volume.device_id` is a foreign key, so the machine has to exist
		// before a drive can be attached to it.
		let device_id = Uuid::now_v7();
		device::ActiveModel {
			uuid: Set(device_id),
			name: Set("test".to_string()),
			slug: Set(format!("test-{}", device_id.simple())),
			os: Set("macos".to_string()),
			network_addresses: Set(serde_json::json!([])),
			capabilities: Set(serde_json::json!({})),
			is_online: Set(true),
			sync_enabled: Set(false),
			last_seen_at: Set(chrono::Utc::now()),
			created_at: Set(chrono::Utc::now()),
			updated_at: Set(chrono::Utc::now()),
			..Default::default()
		}
		.insert(db.conn())
		.await
		.expect("insert device");

		let uuid = Uuid::now_v7();
		volume::ActiveModel {
			uuid: Set(uuid),
			device_id: Set(device_id),
			fingerprint: Set(uuid.to_string()),
			tracked_at: Set(chrono::Utc::now()),
			last_seen_at: Set(chrono::Utc::now()),
			is_online: Set(true),
			mount_point: Set(Some(mount_point.to_string_lossy().into_owned())),
			..Default::default()
		}
		.insert(db.conn())
		.await
		.expect("insert volume");

		VolumeAnchor {
			uuid,
			mount_point: mount_point.to_path_buf(),
		}
	}

	/// A registered source root is watchable after a restart even though no
	/// walk has run this session: the partition restore does not re-list a
	/// nested source in `indexed_paths`, and the registration is the durable
	/// evidence the root deserves a watch. A path with neither evidence stays
	/// refused.
	#[tokio::test]
	async fn a_registered_root_is_watchable_without_a_walk_this_session() {
		let cache = isolated_cache();
		let volume = tempfile::tempdir().expect("volume");
		let nested = volume.path().join("kept");
		std::fs::create_dir(&nested).expect("nested source dir");

		cache.track_volume(Uuid::now_v7(), volume.path().to_path_buf());
		cache
			.register_source(&nested, None)
			.await
			.expect("register");

		assert!(
			cache.register_for_watching(nested.clone()),
			"the registration alone earns the watch"
		);
		assert!(cache.is_watched(&nested));
		assert!(
			!cache.register_for_watching(volume.path().join("stray")),
			"an unregistered, unwalked path is still refused"
		);
	}

	#[tokio::test]
	async fn test_partition_isolation() {
		let cache = isolated_cache();

		let a = cache
			.register_source(Path::new("/test/vol-a"), None)
			.await
			.unwrap();
		let b = cache
			.register_source(Path::new("/test/vol-b"), None)
			.await
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

	#[tokio::test]
	async fn test_longest_prefix_resolution() {
		let cache = isolated_cache();

		cache
			.register_source(Path::new("/mnt"), None)
			.await
			.unwrap();
		let nested = cache
			.register_source(Path::new("/mnt/drive"), None)
			.await
			.unwrap();

		// Both sources sit on one drive, because nesting narrows what is
		// persisted rather than which map a file belongs to.
		let outer = cache.resolve(Path::new("/mnt/other.txt"));
		let inner = cache.resolve(Path::new("/mnt/drive/file.txt"));
		assert!(Arc::ptr_eq(&outer, &inner));
		assert_eq!(inner.volume, VolumeKey::Path(PathBuf::from("/mnt")));

		// The innermost source is still the one that persists it.
		assert_eq!(
			cache.source_id_for(Path::new("/mnt/drive/file.txt")),
			Some(nested)
		);
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

	#[tokio::test]
	async fn test_stats_aggregate_across_partitions() {
		let cache = isolated_cache();

		cache
			.register_source(Path::new("/test/vol-a"), None)
			.await
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

	/// The real lifecycle, driven the way the app drives it.
	///
	/// These exist because every bug in this file was found by hand, in a
	/// running daemon, after the index was already gone. The sequences below are
	/// the ones that actually happened.
	mod lifecycle {
		use super::*;
		use crate::ops::indexing::metadata::EntryMetadata;
		use crate::ops::indexing::state::EntryKind;

		fn directory(path: &Path) -> EntryMetadata {
			EntryMetadata {
				kind: EntryKind::Directory,
				..entry(path)
			}
		}

		fn entry(path: &Path) -> EntryMetadata {
			EntryMetadata {
				kind: EntryKind::File,
				path: path.to_path_buf(),
				size: 1,
				modified: None,
				accessed: None,
				created: None,
				inode: None,
				permissions: None,
				uid: None,
				gid: None,
				link_target: None,
				is_hidden: false,
			}
		}

		/// A store rebuild lands on its own task; wait for the root to leave
		/// the in-progress set.
		async fn wait_for_rebuild(cache: &VolumeIndex, root: &Path) {
			for _ in 0..600 {
				if !cache.is_indexing(root) {
					return;
				}
				tokio::time::sleep(std::time::Duration::from_millis(50)).await;
			}
			panic!("the rebuild of {} did not finish", root.display());
		}

		/// Index `count` files under an already registered source and save
		/// its snapshot, the way a completed walk leaves it. Store records
		/// written before this call land on the registry row's count.
		async fn walked_and_saved(cache: &VolumeIndex, root: &Path, count: u64) {
			let index = cache.create_for_indexing(root.to_path_buf());
			{
				let mut index = index.write().await;
				for i in 0..count {
					let path = root.join(format!("file-{i}"));
					index
						.add_entry(path.clone(), Uuid::now_v7(), entry(&path))
						.expect("add");
				}
			}
			cache.mark_indexing_complete(root);
			cache.save_snapshot(root).await.expect("save");
		}

		/// A source whose partition holds `count` files, indexed and saved the
		/// way a completed walk leaves it.
		async fn indexed_source(
			cache: &VolumeIndex,
			root: &Path,
			anchor: VolumeAnchor,
			count: u64,
		) -> Uuid {
			let id = cache
				.register_source(root, Some(anchor))
				.await
				.expect("register");
			let index = cache.create_for_indexing(root.to_path_buf());
			{
				let mut index = index.write().await;
				for i in 0..count {
					let path = root.join(format!("file-{i}"));
					index
						.add_entry(path.clone(), Uuid::now_v7(), entry(&path))
						.expect("add");
				}
			}
			cache.mark_indexing_complete(root);
			cache.save_snapshot(root).await.expect("save");
			id
		}

		/// What `directory_listing` does when someone opens a folder: take the
		/// partition, clear the folder's stale children, index what is there
		/// now, and save.
		async fn browse(cache: &VolumeIndex, dir: &Path, names: &[&str]) {
			let index = cache.create_for_indexing(dir.to_path_buf());
			cache.clear_for_reindex(dir).await;
			{
				let mut index = index.write().await;
				for name in names {
					let path = dir.join(name);
					index
						.add_entry(path.clone(), Uuid::now_v7(), entry(&path))
						.expect("add");
				}
			}
			cache.mark_indexing_complete(dir);
			cache.save_snapshot(dir).await.expect("save");
		}

		fn counted(cache: &VolumeIndex, id: Uuid) -> u64 {
			cache
				.sources()
				.into_iter()
				.find(|s| s.id == id)
				.and_then(|s| s.entry_count)
				.expect("source has a count")
		}

		/// A snapshot has to survive its own round trip at real sizes. If it
		/// does not, a restart restores nothing and the first browse afterwards
		/// writes its handful of entries over a whole drive's index.
		#[tokio::test]
		async fn a_snapshot_round_trips_at_size() {
			for count in [8_u64, 2_000] {
				let dir = tempfile::tempdir().unwrap();
				let root = dir.path().to_path_buf();
				let file = root.join("snap.bin");
				let source_id = Uuid::now_v7();

				let mut index = Arena::new().unwrap();
				for i in 0..count {
					let path = root.join(format!("file-{i}"));
					index
						.add_entry(path.clone(), Uuid::now_v7(), entry(&path))
						.unwrap();
				}
				let saved = index.get_stats().total_entries;
				index.save_snapshot(&file, source_id, &root).unwrap();

				let loaded = Arena::load_snapshot(&file)
					.unwrap_or_else(|e| panic!("{count} entries: load errored: {e}"));
				let (loaded, meta) =
					loaded.unwrap_or_else(|| panic!("{count} entries: snapshot did not read back"));

				assert_eq!(loaded.get_stats().total_entries, saved, "{count} entries");
				assert_eq!(meta.source_id, source_id);
			}
		}

		/// A snapshot whose saved root disagrees with the resolved root is
		/// evidence, and evidence moves aside instead of being deleted. The
		/// disagreement can mean a genuine remount, but it can also mean the
		/// root resolver changed its answer, and a resolution bug must not be
		/// able to destroy the only artifact.
		#[tokio::test]
		async fn a_root_mismatched_snapshot_moves_aside_instead_of_deleting() {
			use sea_orm::{
				ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter,
			};

			use crate::infra::db::entities::volume;

			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let old_dir = tempfile::tempdir().unwrap();
			let new_dir = tempfile::tempdir().unwrap();

			// Session one indexes the drive at its original mount.
			let anchor = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, old_dir.path()).await;
				indexed_source(&cache, old_dir.path(), anchor.clone(), 4).await;
				anchor
			};

			// The drive comes back mounted somewhere else.
			let row = volume::Entity::find()
				.filter(volume::Column::Uuid.eq(anchor.uuid))
				.one(library.conn())
				.await
				.expect("query")
				.expect("volume row");
			let mut remounted: volume::ActiveModel = row.into();
			remounted.mount_point = Set(Some(new_dir.path().to_string_lossy().into_owned()));
			remounted.update(library.conn()).await.expect("remount");

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");

			let snapshot_path = cache
				.snapshot_path_for(new_dir.path())
				.expect("snapshot path");

			assert!(
				!cache.ensure_restored(new_dir.path()).await,
				"a mismatched snapshot must not restore"
			);
			assert!(
				!snapshot_path.exists(),
				"the mismatched snapshot must leave its slot for the next save"
			);
			assert!(
				snapshot_path.with_extension("mismatched-root").exists(),
				"the mismatched snapshot must survive as an aside artifact"
			);
		}

		/// A restored index has to announce itself so something can watch it.
		///
		/// An index arrives two ways and only one of them used to arm a watch.
		/// A walk finishes and the indexing job watches what it walked; a
		/// restart rebuilds the same index from a snapshot and armed nothing,
		/// so a drive that browsed perfectly reported no changes at all until
		/// it was indexed again. Measured on a live daemon: 2.1 million entries
		/// restored, zero watched roots, a new file on the desktop producing no
		/// event.
		#[tokio::test]
		async fn a_restored_source_announces_itself_for_watching() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			{
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				indexed_source(&cache, &root, anchor, 8).await;
			}

			// A new session, as a restarted daemon sees it.
			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			let mut restored_roots = cache.subscribe_restored_roots();

			assert!(
				cache.ensure_restored(&root).await,
				"snapshot did not restore"
			);
			assert_eq!(
				restored_roots.try_recv().ok(),
				Some(root.clone()),
				"a restored source was never offered for watching"
			);

			// Already restored, so nothing further to announce; arming twice
			// would leave the watch with a reference count it never sheds.
			assert!(cache.ensure_restored(&root).await);
			assert!(
				restored_roots.try_recv().is_err(),
				"a second look at the same source announced it again"
			);
		}

		/// A drive is mapped by tracking it and kept by registering a source
		/// over it, and those are different acts.
		///
		/// Indexing a whole machine and choosing what to keep off it were one
		/// operation, so every drive that got walked appeared in the sources
		/// list as a side effect. A tracked drive now gets a partition and a
		/// snapshot and stays out of that list, which is what lets the whole
		/// machine be searchable while only a home folder is persisted.
		#[tokio::test]
		async fn a_tracked_drive_maps_without_appearing_as_a_source() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			let volume = Uuid::now_v7();

			let count = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				cache.track_volume(volume, root.clone());

				// It has a map of its own rather than falling into scratch.
				let slot = cache.resolve(&root.join("a.txt"));
				assert_eq!(slot.volume, VolumeKey::Id(volume));

				let index = cache.create_for_indexing(root.clone());
				{
					let mut index = index.write().await;
					for i in 0..COLLAPSE_FLOOR * 2 {
						let path = root.join(format!("file-{i}"));
						index
							.add_entry(path.clone(), Uuid::now_v7(), entry(&path))
							.expect("add");
					}
				}
				cache.mark_indexing_complete(&root);
				cache.save_snapshot(&root).await.expect("save");

				assert!(
					cache.sources().is_empty(),
					"mapping a drive is not choosing to keep it"
				);
				let count = index.read().await.get_stats().total_entries;
				count
			};

			// And it comes back, because the snapshot belongs to the drive.
			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			cache.track_volume(volume, root.clone());
			assert!(
				cache.ensure_restored(&root).await,
				"snapshot did not restore"
			);
			assert_eq!(
				cache
					.resolve_index(&root)
					.read()
					.await
					.get_stats()
					.total_entries,
				count
			);
			assert!(cache.sources().is_empty());
		}

		/// A source inside another narrows what is kept, not which map a file
		/// belongs to.
		///
		/// Keyed by source id, a nested registration forked the index: paths
		/// under the inner root resolved to a second partition, the outer one
		/// kept an unreachable copy of them, and each ledger minted its own
		/// uuid for the same file, so a tag applied through one root was
		/// invisible through the other.
		#[tokio::test]
		async fn a_nested_source_shares_the_drive_it_sits_on() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			let inner = root.join("Photos");
			std::fs::create_dir_all(&inner).unwrap();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");

			let anchor = tracked_volume(&library, &root).await;
			let drive = cache
				.register_source(&root, Some(anchor.clone()))
				.await
				.expect("drive");
			let photos = cache
				.register_source(&inner, Some(anchor))
				.await
				.expect("photos");
			assert_ne!(drive, photos, "two registrations, two sources");

			// One drive, one arena.
			let outside = cache.resolve(&root.join("notes.txt"));
			let inside = cache.resolve(&inner.join("a.jpg"));
			assert!(
				Arc::ptr_eq(&outside, &inside),
				"a nested source forked the map of its own drive"
			);

			// And one snapshot, so a browse of either cannot overwrite the
			// other's copy of the same drive.
			let dirs = SourceDirs::new(data.path().join("sources")).expect("layout");
			assert_eq!(
				dirs.snapshot_file(inside.volume.id()),
				dirs.snapshot_file(outside.volume.id())
			);

			// What differs is what each one keeps.
			let outer_store = cache.store_for(&root.join("notes.txt")).await;
			let inner_store = cache.store_for(&inner.join("a.jpg")).await;
			assert!(outer_store.is_some() && inner_store.is_some());
			assert!(
				!Arc::ptr_eq(outer_store.as_ref().unwrap(), inner_store.as_ref().unwrap()),
				"the innermost source persists its own subtree"
			);
		}

		/// R8 "Same volume, nested roots, reversed registration order", the
		/// map half: with the inner source registered first, both still share
		/// the drive's one arena, and a file's identity is the same whether it
		/// is reached through the map or the innermost store.
		#[tokio::test]
		async fn nested_sources_registered_inner_first_share_one_map_and_identity() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			let inner = root.join("Photos");
			std::fs::create_dir_all(&inner).unwrap();
			std::fs::write(inner.join("a.jpg"), b"jpeg").unwrap();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			let anchor = tracked_volume(&library, &root).await;

			let photos = cache
				.register_source(&inner, Some(anchor.clone()))
				.await
				.expect("photos");
			let drive = cache
				.register_source(&root, Some(anchor))
				.await
				.expect("drive");
			assert_ne!(drive, photos);

			let outside = cache.resolve(&root.join("notes.txt"));
			let inside = cache.resolve(&inner.join("a.jpg"));
			assert!(
				Arc::ptr_eq(&outside, &inside),
				"registration order forked the drive's map"
			);
			assert_eq!(
				inside.root(),
				Some(root.clone()),
				"the volume root is the mount, not the first registered source"
			);

			// The map hands out the identity; both stores adopt it.
			let path = inner.join("a.jpg");
			let mapped = Uuid::now_v7();
			inside
				.index()
				.write()
				.await
				.add_entry(path.clone(), mapped, entry(&path))
				.expect("map");
			let store = cache.store_for(&path).await.expect("innermost store");
			assert_eq!(store.id(), photos, "the innermost source keeps the file");
			let identified = store
				.identify_one(&entry(&path), Some(mapped))
				.await
				.expect("identified");
			assert_eq!(identified, mapped, "one file, one identity");
			store.flush().await.expect("flush");
			assert_eq!(
				store.db().resolve_path("a.jpg").await.expect("query"),
				Some(mapped)
			);
		}

		/// R8 "Missing or invalid restart snapshot".
		///
		/// A snapshot that will not parse restores nothing from the file, but
		/// the source it belonged to stays registered and listed, and its map
		/// comes back from its store with the store's uuids, so no walk is
		/// needed to list it again.
		#[tokio::test]
		async fn an_invalid_snapshot_leaves_the_source_visible_and_rebuilt_from_its_store() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			std::fs::write(root.join("kept.txt"), b"kept").unwrap();

			let (id, snapshot_path, kept_uuid) = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let id = cache
					.register_source(&root, Some(anchor))
					.await
					.expect("register");
				let store = cache
					.store_for(&root.join("kept.txt"))
					.await
					.expect("store");
				let kept_uuid = store
					.identify_one(&entry(&root.join("kept.txt")), None)
					.await
					.expect("identified");
				store.flush().await.expect("flush");
				walked_and_saved(&cache, &root, 4).await;
				(
					id,
					cache.snapshot_path_for(&root).expect("snapshot path"),
					kept_uuid,
				)
			};
			assert!(snapshot_path.exists());
			std::fs::write(&snapshot_path, b"this is not a snapshot").unwrap();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			assert!(
				cache.ensure_restored(&root).await,
				"the store refills the map the junk could not"
			);
			wait_for_rebuild(&cache, &root).await;
			assert!(
				!cache.restored_from_snapshot(&root),
				"nothing came from the snapshot file"
			);
			assert!(
				cache.sources().iter().any(|s| s.id == id),
				"the source stays registered without its cache"
			);
			let db = cache.read_store(id).await.expect("the store still opens");
			assert_eq!(
				db.resolve_path("kept.txt").await.expect("query"),
				Some(kept_uuid),
				"retained records answer from the store"
			);
			assert!(
				cache.arena_answers(&root),
				"a query over this source routes to the rebuilt arena"
			);
			let index = cache.get_for_search(&root).expect("rebuilt index");
			let index = index.read().await;
			assert_eq!(
				index.get_entry_uuid(&root.join("kept.txt")),
				Some(kept_uuid),
				"the rebuilt map carries the store's identity"
			);
			assert!(
				index.get_entry_uuid(&root.join("file-0")).is_none(),
				"entries the snapshot alone held are gone with it"
			);
		}

		/// A v3-shaped artifact after the v4 bump: the header decodes, the
		/// version check refuses it, and the slot is quarantined like any
		/// other unreadable snapshot. The map is rebuilt from the store
		/// rather than walked, and the artifact is kept for diagnosis.
		#[tokio::test]
		async fn an_older_format_snapshot_is_quarantined_and_the_map_rebuilt_from_the_store() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			for name in ["a.txt", "b.txt", "sub/c.txt"] {
				let path = root.join(name);
				std::fs::create_dir_all(path.parent().unwrap()).unwrap();
				std::fs::write(&path, name).unwrap();
			}

			let (id, snapshot_path, volume_index_id, uuids) = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let volume_index_id = VolumeKey::Id(anchor.uuid).id();
				let id = cache
					.register_source(&root, Some(anchor))
					.await
					.expect("register");
				let store = cache.store_for(&root.join("a.txt")).await.expect("store");
				let mut uuids = Vec::new();
				for name in ["a.txt", "b.txt", "sub/c.txt"] {
					let path = root.join(name);
					let uuid = store
						.identify_one(&entry(&path), None)
						.await
						.expect("identified");
					uuids.push((path, uuid));
				}
				store.flush().await.expect("flush");
				walked_and_saved(&cache, &root, 0).await;
				(
					id,
					cache.snapshot_path_for(&root).expect("snapshot path"),
					volume_index_id,
					uuids,
				)
			};

			// The previous build's artifact: the same container and header
			// layout, stamped with the format it was written in.
			crate::ops::indexing::snapshot::write_artifact_with_version(
				&snapshot_path,
				3,
				volume_index_id,
				&root,
			);

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			assert!(
				cache.ensure_restored(&root).await,
				"the map is rebuilt without a walk"
			);
			assert!(
				cache.is_indexing(&root) || cache.arena_answers(&root),
				"the rebuild is in progress or landed"
			);
			wait_for_rebuild(&cache, &root).await;
			assert!(!cache.restored_from_snapshot(&root));
			assert!(cache.sources().iter().any(|s| s.id == id));
			assert!(
				!snapshot_path.exists(),
				"the slot is cleared so the next save lands clean"
			);
			let name = snapshot_path.file_name().unwrap().to_string_lossy();
			let retained = std::fs::read_dir(snapshot_path.parent().unwrap())
				.unwrap()
				.filter_map(|entry| entry.ok().map(|entry| entry.path()))
				.filter(|path| {
					path.file_name()
						.map(|n| n.to_string_lossy().starts_with(&format!("{name}.corrupt-")))
						.unwrap_or(false)
				})
				.count();
			assert_eq!(retained, 1, "the v3 artifact is kept beside the slot");

			let index = cache.get_for_search(&root).expect("rebuilt index");
			let index = index.read().await;
			for (path, uuid) in uuids {
				assert_eq!(
					index.get_entry_uuid(&path),
					Some(uuid),
					"{} keeps the store's uuid",
					path.display()
				);
			}
			assert!(
				index
					.list_directory(&root)
					.is_some_and(|children| !children.is_empty()),
				"the source root has coverage, so the heal has nothing to walk"
			);
		}

		/// The rebuild time behind the missing-snapshot path: a store of
		/// `SD_REBUILD_RECORDS` files (one million by default) in directories
		/// of a hundred, the way a walk lays them out, read back into an
		/// empty arena. Run with
		/// `cargo test -p sd-core --release --lib
		///   a_million_record_store_rebuilds -- --ignored --nocapture`.
		#[tokio::test]
		#[ignore = "measurement, not regression; run with --ignored --nocapture"]
		async fn a_million_record_store_rebuilds_in_seconds() {
			use sd_store::file::{FileKind, FileWrite, Ledger, Observation};

			let records: usize = std::env::var("SD_REBUILD_RECORDS")
				.ok()
				.and_then(|v| v.parse().ok())
				.unwrap_or(1_000_000);
			let dir = tempfile::tempdir().unwrap();
			let manager = sd_store::SourceManager::new(dir.path().to_path_buf());
			manager
				.create("source-1", &sd_store::filesystem_schema())
				.await
				.expect("create");
			let db = manager.open("source-1").await.expect("open");
			db.begin_sync().await.expect("epoch");
			let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
			let observe = |external_id: String, kind: FileKind| {
				let name = external_id.rsplit('/').next().unwrap().to_string();
				Observation {
					extension: (kind == FileKind::File).then(|| "dat".to_string()),
					external_id,
					kind,
					name,
					size: 1_000,
					mtime: 1_700_000_000_000,
					created: None,
					accessed: None,
					inode: None,
					mode: Some(0o644),
					uid: None,
					gid: None,
					link_target: None,
					is_hidden: false,
					identity: None,
				}
			};
			let built = Instant::now();
			let mut written = 0usize;
			let mut top = 0usize;
			while written < records {
				let mut writes = Vec::with_capacity(10_100);
				let top_obs = observe(format!("dir-{top}"), FileKind::Directory);
				let top_res = ledger.resolve(&top_obs);
				let top_uuid = top_res.uuid();
				writes.push(FileWrite {
					resolution: top_res,
					parent_uuid: None,
					observation: top_obs,
				});
				for sub in 0..100 {
					let sub_obs = observe(format!("dir-{top}/sub-{sub}"), FileKind::Directory);
					let sub_res = ledger.resolve(&sub_obs);
					let sub_uuid = sub_res.uuid();
					writes.push(FileWrite {
						resolution: sub_res,
						parent_uuid: Some(top_uuid),
						observation: sub_obs,
					});
					for file in 0..100 {
						if written >= records {
							break;
						}
						let obs = observe(
							format!("dir-{top}/sub-{sub}/file-{written}.dat"),
							FileKind::File,
						);
						let resolution = ledger.resolve(&obs);
						writes.push(FileWrite {
							resolution,
							parent_uuid: Some(sub_uuid),
							observation: obs,
						});
						written += 1;
					}
				}
				db.apply_files(&writes, &[], &[], None)
					.await
					.expect("apply");
				top += 1;
			}
			let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record")
				.fetch_one(db.pool())
				.await
				.expect("count");
			println!(
				"built a store of {total} records ({records} files) in {:?}",
				built.elapsed()
			);

			let read_only = Instant::now();
			let mut rows = 0usize;
			let mut after = 0i64;
			loop {
				let (entries, last) =
					sd_store::read::rebuild_entries_page(db.pool(), after, REBUILD_PAGE)
						.await
						.expect("page");
				rows += entries.len();
				after = last;
				if entries.len() < REBUILD_PAGE {
					break;
				}
			}
			println!(
				"read {rows} rows in {:?} (store pages alone)",
				read_only.elapsed()
			);

			let index = TokioRwLock::new(Arena::new().expect("arena"));
			let root = PathBuf::from("/volume/source");
			let started = Instant::now();
			let loaded = fill_arena_from_store(&index, &db, &root)
				.await
				.expect("rebuild");
			let elapsed = started.elapsed();
			let stats = index.read().await.get_stats();
			println!(
				"rebuilt {loaded} records ({} live entries) in {elapsed:?}",
				stats.total_entries
			);
			assert_eq!(loaded as i64, total);
		}

		/// R8 "Missing or invalid restart snapshot", the diagnosis half: an
		/// artifact that will not parse is evidence and must stay on disk
		/// until a validated replacement lands.
		#[tokio::test]
		async fn an_invalid_snapshot_is_retained_for_diagnosis() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let snapshot_path = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				indexed_source(&cache, &root, anchor, 4).await;
				cache.snapshot_path_for(&root).expect("snapshot path")
			};
			std::fs::write(&snapshot_path, b"this is not a snapshot").unwrap();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			cache.ensure_restored(&root).await;
			assert!(
				!cache.restored_from_snapshot(&root),
				"junk must not restore as an index"
			);
			assert!(
				!snapshot_path.exists(),
				"the slot is cleared so the next save lands clean"
			);
			let name = snapshot_path.file_name().unwrap().to_string_lossy();
			let retained: Vec<PathBuf> = std::fs::read_dir(snapshot_path.parent().unwrap())
				.unwrap()
				.filter_map(|entry| entry.ok().map(|entry| entry.path()))
				.filter(|path| {
					path.file_name()
						.map(|n| n.to_string_lossy().starts_with(&format!("{name}.corrupt-")))
						.unwrap_or(false)
				})
				.collect();
			assert_eq!(retained.len(), 1, "one retained artifact beside the slot");
			assert_eq!(
				std::fs::read(&retained[0]).ok().as_deref(),
				Some(&b"this is not a snapshot"[..]),
				"the unreadable artifact is kept for diagnosis"
			);
		}

		/// R8 "Mapped volume with no source", the watcher half: a walked
		/// tracked drive is watchable without being registered as a source.
		/// The browse and snapshot halves are
		/// `a_tracked_drive_maps_without_appearing_as_a_source`.
		#[tokio::test]
		async fn a_mapped_drive_without_a_source_is_watchable() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			cache.track_volume(Uuid::now_v7(), root.clone());

			assert!(
				!cache.register_for_watching(root.clone()),
				"an unwalked drive has nothing to watch yet"
			);
			let index = cache.create_for_indexing(root.clone());
			let path = root.join("a.txt");
			index
				.write()
				.await
				.add_entry(path.clone(), Uuid::now_v7(), entry(&path))
				.expect("add");
			cache.mark_indexing_complete(&root);

			assert!(cache.register_for_watching(root.clone()));
			assert!(cache.is_watched(&root));
			assert_eq!(cache.find_watched_root(&path), Some(root));
			assert!(cache.sources().is_empty(), "still not a source");
		}

		/// A change deep inside a watched source has to route to that source.
		///
		/// A source rooted at the whole drive registers exactly one watched
		/// path, and every event it receives arrives from somewhere below it.
		/// Matching an event's parent against the watched root admitted only
		/// the root's own children, so a screenshot landing on the desktop of a
		/// fully indexed drive was dropped as unmatched and the UI never moved.
		/// These are the two questions the handler asks of a routed event.
		#[tokio::test]
		async fn a_change_deep_in_a_source_routes_to_it() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");

			let anchor = tracked_volume(&library, &root).await;
			cache
				.register_source(&root, Some(anchor))
				.await
				.expect("register");

			// A walk that went all the way down, as a source walk does.
			let desktop = root.join("Users").join("me").join("Desktop");
			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				for dir in [
					root.join("Users"),
					root.join("Users").join("me"),
					desktop.clone(),
				] {
					index
						.add_entry(dir.clone(), Uuid::now_v7(), directory(&dir))
						.expect("add");
				}
				let existing = desktop.join("already-here.png");
				index
					.add_entry(existing.clone(), Uuid::now_v7(), entry(&existing))
					.expect("add");
			}
			cache.mark_indexing_complete(&root);

			// The index job watches the root it just walked, and only that.
			assert!(
				cache.register_for_watching(root.clone()),
				"a completed walk registers its root for watching"
			);

			let screenshot = desktop.join("Screenshot.png");
			assert_eq!(
				cache.watched_root_for_change(&screenshot).await,
				Some(root.clone()),
				"a file several levels down belongs to the source that indexed it"
			);
		}

		/// A change under a summarised directory has no tree to land in
		/// either, but it does say the count standing in for that subtree is
		/// now wrong, so the directory is marked for recounting.
		#[tokio::test]
		async fn a_change_under_a_summary_marks_it_for_recounting() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");

			let anchor = tracked_volume(&library, &root).await;
			cache
				.register_source(&root, Some(anchor))
				.await
				.expect("register");

			// A directory the walk turned back at, standing in for whatever is
			// under it.
			let summarised = root.join("Library");
			std::fs::create_dir_all(&summarised).expect("mkdir");

			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				index
					.add_entry(summarised.clone(), Uuid::now_v7(), directory(&summarised))
					.expect("add");
				index.summarise(
					&summarised,
					crate::ops::indexing::Rollup {
						bytes: 4_096,
						files: 9,
					},
				);
			}
			cache.mark_indexing_complete(&root);
			assert!(cache.register_for_watching(root.clone()));

			assert_eq!(
				cache
					.watched_root_for_change(&summarised.join("cache.bin"))
					.await,
				None,
				"there is no tree under a summary for the change to land in"
			);

			assert_eq!(
				cache.take_dirty_stubs(),
				vec![summarised],
				"the summary standing in for it is what needs counting again"
			);
			assert!(
				cache.take_dirty_stubs().is_empty(),
				"and it is handed over once, not on every pass"
			);
		}

		/// The other half: a directory nothing walked has nowhere to put a
		/// change, and grafting one on would build a tree beside the real one.
		#[tokio::test]
		async fn a_change_under_an_unwalked_directory_is_not_grafted_on() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");

			let anchor = tracked_volume(&library, &root).await;
			cache
				.register_source(&root, Some(anchor))
				.await
				.expect("register");

			let index = cache.create_for_indexing(root.clone());
			{
				let mut index = index.write().await;
				let shallow = root.join("visible.txt");
				index
					.add_entry(shallow.clone(), Uuid::now_v7(), entry(&shallow))
					.expect("add");
			}
			cache.mark_indexing_complete(&root);
			assert!(cache.register_for_watching(root.clone()));

			let unwalked = root.join("Deep").join("Nested");
			assert_eq!(
				cache.find_watched_root(&unwalked.join("file.txt")),
				Some(root.clone()),
				"it is still this source's territory"
			);
			assert_eq!(
				cache
					.watched_root_for_change(&unwalked.join("file.txt"))
					.await,
				None,
				"but nothing walked it, so the change has nowhere to land"
			);
		}

		/// A walk of an empty directory writes no entry for it, since nothing
		/// under it synthesizes it as an ancestor. The first file dropped into
		/// it still has to land.
		#[tokio::test]
		async fn a_change_under_an_empty_walked_root_lands() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");

			let anchor = tracked_volume(&library, &root).await;
			cache
				.register_source(&root, Some(anchor))
				.await
				.expect("register");

			// The walk found nothing, so the index holds nothing, the root included.
			let _index = cache.create_for_indexing(root.clone());
			cache.mark_indexing_complete(&root);
			assert!(cache.register_for_watching(root.clone()));

			assert_eq!(
				cache
					.watched_root_for_change(&root.join("dropped-in.txt"))
					.await,
				Some(root.clone()),
				"a file dropped into the empty root belongs to it"
			);
			assert_eq!(
				cache
					.watched_root_for_change(&root.join("Deep").join("file.txt"))
					.await,
				None,
				"while a change deeper down still has nowhere to land"
			);
		}

		/// Observed in a running daemon: a full drive index of 2.1 million
		/// entries was replaced by the 29 entries of a folder opened four
		/// minutes later, because opening a folder saves the whole partition.
		#[tokio::test]
		async fn opening_a_folder_does_not_shrink_the_drive_index() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");

			let anchor = tracked_volume(&library, &root).await;
			let id = indexed_source(&cache, &root, anchor, COLLAPSE_FLOOR * 3).await;
			let full = counted(&cache, id);

			let folder = root.join("Desktop");
			std::fs::create_dir_all(&folder).unwrap();
			browse(&cache, &folder, &["a.png", "b.png"]).await;

			// Growing is fine: the folder's own entries join the partition.
			// Shrinking is the bug.
			assert!(
				counted(&cache, id) >= full,
				"opening a folder replaced the drive's index with its contents: {} then {}",
				full,
				counted(&cache, id)
			);
		}

		/// The index has to still be there tomorrow. A snapshot that saves but
		/// does not restore is the same as no snapshot, and the arena is the
		/// only thing the client reads.
		#[tokio::test]
		async fn a_full_index_survives_a_restart() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();

			let (id, before) = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let id = indexed_source(&cache, &root, anchor, COLLAPSE_FLOOR * 2).await;
				let before = counted(&cache, id);
				(id, before)
			};

			// A new session, as a restarted daemon sees it.
			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			assert!(
				cache.ensure_restored(&root).await,
				"snapshot did not restore"
			);

			let restored = cache
				.resolve_index(&root)
				.read()
				.await
				.get_stats()
				.total_entries as u64;
			assert_eq!(
				restored, before,
				"the arena came back smaller than it was saved"
			);
			assert_eq!(counted(&cache, id), before);
		}

		/// A restart followed by opening one folder is the ordinary way a day
		/// starts, and it must not cost the drive's index. This is the pair of
		/// the two sequences above, which is how it actually happened.
		#[tokio::test]
		async fn a_restart_then_a_browse_keeps_the_index() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			let folder = root.join("Desktop");
			std::fs::create_dir_all(&folder).unwrap();

			let (id, full) = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache
					.attach_library(LIBRARY, library.clone())
					.await
					.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let id = indexed_source(&cache, &root, anchor, COLLAPSE_FLOOR * 3).await;
				let full = counted(&cache, id);
				(id, full)
			};

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library)
				.await
				.expect("attach");
			cache.ensure_restored(&root).await;
			browse(&cache, &folder, &["one.txt"]).await;

			assert!(
				counted(&cache, id) >= full,
				"a browse after a restart shrank the drive's index: {} then {}",
				full,
				counted(&cache, id)
			);
		}
	}

	/// A whole drive's index must survive whatever the app does next.
	///
	/// The failure this pins is real and was observed: a full volume index of
	/// 2.1 million entries was replaced by the 29 entries of a directory
	/// someone browsed four minutes later, because a save only has to clear the
	/// "did this session restore" gate, and by then it had.
	#[tokio::test]
	async fn a_collapsed_partition_does_not_replace_a_full_index() {
		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let root = tempfile::tempdir().unwrap();
		let root = root.path().to_path_buf();

		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library(LIBRARY, library.clone())
			.await
			.expect("attach");
		let source_id = cache
			.register_source(&root, Some(tracked_volume(&library, &root).await))
			.await
			.unwrap();

		use crate::ops::indexing::metadata::EntryMetadata;
		use crate::ops::indexing::state::EntryKind;
		let meta = |path: &Path| EntryMetadata {
			kind: EntryKind::File,
			path: path.to_path_buf(),
			size: 1,
			modified: None,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		};

		// A full index lands and persists.
		let index = cache.create_for_indexing(root.clone());
		{
			let mut index = index.write().await;
			for i in 0..(COLLAPSE_FLOOR * 3) {
				let path = root.join(format!("file-{i}"));
				index
					.add_entry(path.clone(), Uuid::now_v7(), meta(&path))
					.unwrap();
			}
		}
		cache.mark_indexing_complete(&root);
		cache.save_snapshot(&root).await.unwrap();

		let full = cache
			.sources()
			.into_iter()
			.find(|s| s.id == source_id)
			.and_then(|s| s.entry_count)
			.expect("counted");
		assert!(full >= COLLAPSE_FLOOR * 3);

		// Something empties the partition and asks to save the remains.
		{
			let mut index = index.write().await;
			*index = Arena::new().unwrap();
			let path = root.join("only-this");
			index
				.add_entry(path.clone(), Uuid::now_v7(), meta(&path))
				.unwrap();
		}
		cache.save_snapshot(&root).await.unwrap();

		let after = cache
			.sources()
			.into_iter()
			.find(|s| s.id == source_id)
			.and_then(|s| s.entry_count)
			.expect("counted");
		assert_eq!(
			after, full,
			"a collapsed partition overwrote a full index instead of being refused"
		);
	}

	/// L1 of the locked volumes plan: a drive detection cannot see is away,
	/// whatever its row says and whatever is left at its mount point.
	///
	/// The monitor only ever wrote `is_online` for drives it still saw, so a
	/// drive that vanished while the daemon was down read online forever, and
	/// an unmounted drive leaves its mount point behind as an empty directory
	/// that `Path::exists` is happy with. Attaching against live detection
	/// corrects the row, keeps the map readable, and refuses the watch; the
	/// drive returning attaches it again without a restart.
	#[tokio::test]
	async fn a_volume_detection_cannot_see_comes_up_detached_whatever_its_row_says() {
		use crate::infra::db::entities::volume;
		use crate::ops::indexing::state::EntryKind;
		use crate::ops::indexing::EntryMetadata;

		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();
		let photo = root.join("photo.jpg");

		let anchor = tracked_volume(&library, &root).await;
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			cache
				.register_source(&root, Some(anchor.clone()))
				.await
				.unwrap();
			let index = cache.create_for_indexing(root.clone());
			index
				.write()
				.await
				.add_entry(
					photo.clone(),
					Uuid::now_v7(),
					EntryMetadata {
						kind: EntryKind::File,
						path: photo.clone(),
						size: 42,
						modified: None,
						accessed: None,
						created: None,
						inode: None,
						permissions: None,
						uid: None,
						gid: None,
						link_target: None,
						is_hidden: false,
					},
				)
				.unwrap();
			cache.mark_indexing_complete(&root);
			cache.save_snapshot(&root).await.expect("save snapshot");
		}

		// The drive is unmounted: its directory is still there, and empty,
		// and its row still says online. Detection returns nothing.
		assert!(root.exists());
		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library_with(LIBRARY, library.clone(), None, LiveVolumes::Detected(&[]))
			.await
			.expect("attach");

		let source = &cache.sources()[0];
		assert!(!source.attached, "an empty mount point is not the drive");
		assert_eq!(source.root, root, "the record keeps the root it had");
		assert_eq!(cache.volume_mounted(anchor.uuid), Some(false));
		assert!(cache.is_detached(&root));
		assert!(
			!cache.register_for_watching(root.clone()),
			"no watch is armed on the directory left behind"
		);
		assert!(cache.ensure_restored(&photo).await, "the map restores");
		assert!(cache.is_detached(&photo), "and stays read-only");
		assert!(cache
			.get_for_search(&photo)
			.expect("restored index")
			.read()
			.await
			.get_entry_uuid(&photo)
			.is_some());

		let row = volume::Entity::find()
			.filter(volume::Column::Uuid.eq(anchor.uuid))
			.one(library.conn())
			.await
			.unwrap()
			.expect("volume row");
		assert!(!row.is_online, "the stale flag is corrected in place");

		// The drive returns at the same mount point.
		cache.set_volume_mounted(anchor.uuid, &root, true);
		assert!(cache.sources()[0].attached);
		assert!(!cache.is_detached(&photo));
		assert!(cache.register_for_watching(root.clone()));

		// And goes away again under the running daemon.
		cache.set_volume_mounted(anchor.uuid, &root, false);
		assert!(!cache.sources()[0].attached);
		assert!(cache.is_detached(&photo));
	}

	/// A mapped drive with no source over it is still reachable while it is
	/// away: its map serves read-only from the drive's partition rather than
	/// falling through to scratch, and nothing may be dispatched at it.
	#[tokio::test]
	async fn a_mapped_drive_that_is_away_keeps_its_partition() {
		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();
		let anchor = tracked_volume(&library, &root).await;

		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library_with(LIBRARY, library.clone(), None, LiveVolumes::Detected(&[]))
			.await
			.expect("attach");

		let slot = cache.resolve(&root.join("photo.jpg"));
		assert_eq!(slot.id(), Some(VolumeKey::Id(anchor.uuid).id()));
		assert!(slot.is_detached());
		assert!(cache.dispatch_refusal(&root).is_some());

		// A drive detection found at a directory that is not a mount point
		// is refused whatever its state says; a directory a test stands in
		// for a drive is not held to that.
		let other = tempfile::tempdir().unwrap();
		cache.track_detected_volume(Uuid::now_v7(), other.path().to_path_buf(), true);
		assert!(cache.dispatch_refusal(other.path()).is_some());
		let fixture = tempfile::tempdir().unwrap();
		cache.track_volume(Uuid::now_v7(), fixture.path().to_path_buf());
		assert!(cache.dispatch_refusal(fixture.path()).is_none());
	}

	/// A cloud volume's root is a service prefix, not a directory: detection
	/// never lists it, nothing exists at it, and it is never away for that.
	#[tokio::test]
	async fn a_cloud_root_is_never_refused_for_not_being_mounted() {
		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let bucket = PathBuf::from("s3://bucket");
		let anchor = tracked_volume(&library, &bucket).await;

		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library_with(LIBRARY, library.clone(), None, LiveVolumes::Detected(&[]))
			.await
			.expect("attach");
		assert_eq!(cache.volume_mounted(anchor.uuid), Some(true));
		assert!(!cache.resolve(&bucket.join("photos")).is_detached());
		assert!(cache.dispatch_refusal(&bucket).is_none());

		cache.track_detected_volume(anchor.uuid, bucket.clone(), true);
		assert!(cache.dispatch_refusal(&bucket).is_none());
	}

	/// With detection off there is nothing live to consult, and the row's
	/// flag is what there is.
	#[tokio::test]
	async fn without_detection_the_stored_flag_decides_attachment() {
		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();

		let anchor = tracked_volume(&library, &root).await;
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			cache.register_source(&root, Some(anchor)).await.unwrap();
		}

		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library_with(LIBRARY, library.clone(), None, LiveVolumes::Unavailable)
			.await
			.expect("attach");
		assert!(cache.sources()[0].attached);
	}

	/// A daemon with two libraries serves both. Each library's registry is
	/// its own, so the second to attach adds to the index instead of
	/// replacing what the first adopted, whichever order `read_dir` hands
	/// them to the library manager on a given machine.
	#[tokio::test]
	async fn two_libraries_each_list_their_own_sources_in_either_load_order() {
		let first_dir = tempfile::tempdir().unwrap();
		let second_dir = tempfile::tempdir().unwrap();
		let first_library = test_library(first_dir.path()).await;
		let second_library = test_library(second_dir.path()).await;
		let first_id = Uuid::from_u128(0xa);
		let second_id = Uuid::from_u128(0xb);
		let first_root = tempfile::tempdir().unwrap();
		let second_root = tempfile::tempdir().unwrap();

		let cache_dir = tempfile::tempdir().unwrap();
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			let anchor = tracked_volume(&first_library, first_root.path()).await;
			cache
				.attach_library(first_id, first_library.clone())
				.await
				.expect("attach");
			cache
				.register_source_in(Some(first_id), first_root.path(), Some(anchor))
				.await
				.unwrap();
			let anchor = tracked_volume(&second_library, second_root.path()).await;
			cache
				.attach_library(second_id, second_library.clone())
				.await
				.expect("attach");
			cache
				.register_source_in(Some(second_id), second_root.path(), Some(anchor))
				.await
				.unwrap();
		}

		let roots = |cache: &VolumeIndex, library: Uuid| -> Vec<PathBuf> {
			cache
				.sources_of(library)
				.into_iter()
				.map(|source| source.root)
				.collect()
		};

		for order in [[first_id, second_id], [second_id, first_id]] {
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			for library in order {
				let db = if library == first_id {
					&first_library
				} else {
					&second_library
				};
				cache
					.attach_library(library, db.clone())
					.await
					.expect("attach");
			}

			assert_eq!(
				roots(&cache, first_id),
				vec![first_root.path().to_path_buf()],
				"load order {order:?}"
			);
			assert_eq!(
				roots(&cache, second_id),
				vec![second_root.path().to_path_buf()],
				"load order {order:?}"
			);
			assert_eq!(cache.sources().len(), 2, "load order {order:?}");
			assert!(cache.sources().iter().all(|source| source.attached));
			assert_eq!(
				cache.source_id_for(second_root.path()),
				cache.sources_of(second_id).first().map(|source| source.id)
			);

			// Closing one library takes only its sources away.
			cache.detach_library(first_id);
			assert!(roots(&cache, first_id).is_empty());
			assert_eq!(
				roots(&cache, second_id),
				vec![second_root.path().to_path_buf()]
			);
			assert!(cache.source_id_for(second_root.path()).is_some());
			assert!(cache.source_id_for(first_root.path()).is_none());
			assert_ne!(cache.resolve(second_root.path()).volume, VolumeKey::Scratch);
		}
	}

	#[tokio::test]
	async fn test_snapshot_roundtrip_and_detached_restore() {
		use crate::ops::indexing::state::EntryKind;
		use crate::ops::indexing::EntryMetadata;

		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
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
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		};

		// Session one: register the drive, index some entries, snapshot.
		let saved_uuid;
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			cache
				.register_source(&root, Some(tracked_volume(&library, &root).await))
				.await
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
		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library(LIBRARY, library.clone())
			.await
			.expect("attach");
		assert_eq!(cache.sources().len(), 1);

		let child = unplugged_root.join("photo.jpg");
		assert!(
			!cache.arena_answers(&unplugged_root),
			"before the restore, the arena cannot answer and reads route to the store"
		);
		assert!(cache.ensure_restored(&child).await);
		assert!(cache.is_detached(&child));
		assert!(
			cache.arena_answers(&unplugged_root),
			"a restored partition answers, so an empty result from it is final"
		);

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
		let library = test_library(cache_dir.path()).await;
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
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		};

		// One session, no pre-existing snapshot: a shallow browse saves a few
		// entries, then a full scan of the same source saves many more. The
		// second save must replace the first — the existing file is this
		// session's own artifact, not a prior session's index.
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			cache
				.register_source(&root, Some(tracked_volume(&library, &root).await))
				.await
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
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
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
		let library = test_library(cache_dir.path()).await;
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
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		};

		// Session one: a full index of three entries, snapshotted.
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			cache
				.register_source(&root, Some(tracked_volume(&library, &root).await))
				.await
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
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
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
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
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
		let library = test_library(cache_dir.path()).await;
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
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		};

		let before = root.join("before.txt");
		let after = root.join("after.txt");
		let source_id;
		let renamed_uuid;

		// Session one: index one file, snapshot it.
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			source_id = cache
				.register_source(&root, Some(tracked_volume(&library, &root).await))
				.await
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
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
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
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			assert!(cache.ensure_restored(&after).await);
			let index = cache.resolve_index(&root);
			let mut index = index.write().await;
			assert!(index.get_entry(&before).is_none(), "old name persisted");
			assert_eq!(index.get_entry_uuid(&after), Some(renamed_uuid));
		}

		// A different drive mounted where that one lives is a different source,
		// so it can never be served the first drive's snapshot.
		{
			let cache =
				VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
			cache
				.attach_library(LIBRARY, library.clone())
				.await
				.expect("attach");
			let other = cache
				.register_source(&root, Some(tracked_volume(&library, &root).await))
				.await
				.unwrap();
			assert_ne!(other, source_id);

			let statuses = cache.sources();
			let original = statuses.iter().find(|s| s.id == source_id).unwrap();
			let replacement = statuses.iter().find(|s| s.id == other).unwrap();
			assert_ne!(
				original.volume_uuid, replacement.volume_uuid,
				"the displaced drive kept its own identity"
			);
		}
	}
	/// Add to Library placement: an on-source store resolves beneath the
	/// root under `.spacedrive/sources/<id>`, opens there, and is reported
	/// there, while an in-library store stays under the data directory. The
	/// placement travels in the registration, so a fresh session resolves
	/// the same directory from the row.
	#[tokio::test]
	async fn placement_resolves_the_store_directory() {
		use crate::ops::indexing::sources::{SourceConfig, StorePlacement};

		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();
		let anchor = tracked_volume(&library, &root).await;

		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library(LIBRARY, library.clone())
			.await
			.expect("attach");
		let id = cache
			.register_source(&root, Some(anchor.clone()))
			.await
			.unwrap();
		assert_eq!(
			cache.store_dir(id),
			Some(cache_dir.path().join(id.simple().to_string())),
			"the default placement is the data directory"
		);

		cache
			.set_source_config(
				id,
				SourceConfig {
					placement: StorePlacement::OnSource,
					..SourceConfig::default()
				},
			)
			.await
			.unwrap();
		let expected = root
			.join(".spacedrive")
			.join("sources")
			.join(id.simple().to_string());
		assert_eq!(cache.store_dir(id), Some(expected.clone()));

		let store = cache.store_for(&root.join("a.txt")).await.expect("store");
		assert_eq!(store.id(), id);
		assert!(
			expected.join("data.db").exists(),
			"the store was created beneath the source root"
		);
		assert!(
			!cache_dir.path().join(id.simple().to_string()).exists(),
			"nothing was created in the library for an on-source store"
		);
		let status = cache
			.sources()
			.into_iter()
			.find(|status| status.id == id)
			.unwrap();
		assert_eq!(status.directory, Some(expected.clone()));

		// A restarted daemon resolves the same directory from the row.
		drop(store);
		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library(LIBRARY, library)
			.await
			.expect("attach");
		assert_eq!(cache.store_dir(id), Some(expected));
		assert!(
			cache.read_store(id).await.is_some(),
			"the on-source store reads back from where placement put it"
		);
	}

	/// Removing a source retires its handles and leaves its catalog on
	/// disk; adding the same scope again finds the catalog through its
	/// descriptor and registers under the identity the store carries, so
	/// the records and assertions it holds are the new registration's.
	#[tokio::test]
	async fn a_removed_source_is_readopted_from_its_descriptor() {
		use crate::ops::indexing::sources::StorePlacement;

		let cache_dir = tempfile::tempdir().unwrap();
		let library = test_library(cache_dir.path()).await;
		let library_id = Uuid::now_v7();
		let drive_dir = tempfile::tempdir().unwrap();
		let root = drive_dir.path().to_path_buf();
		let anchor = tracked_volume(&library, &root).await;

		let cache =
			VolumeIndex::with_sources_dir(Some(cache_dir.path().to_path_buf())).expect("cache");
		cache
			.attach_library(LIBRARY, library.clone())
			.await
			.expect("attach");
		let id = cache
			.register_source(&root, Some(anchor.clone()))
			.await
			.unwrap();
		cache.write_descriptor(id, library_id).await.unwrap();
		let store = cache.store_for(&root).await.expect("store");
		let dir = cache.store_dir(id).unwrap();
		assert!(dir.join("source.json").exists());
		drop(store);

		let (record, forgotten) = cache.forget_source(id).await.expect("registered");
		assert_eq!(record.id, id);
		assert_eq!(forgotten, Some(dir.clone()));
		assert!(dir.join("data.db").exists(), "removal keeps the catalog");
		assert!(cache.source_root(id).is_none(), "the registration is gone");
		assert!(cache.store_for(&root).await.is_none());

		// The same scope, the same library: the store's identity is adopted.
		let found = cache
			.portable_identity(&root, Some(&anchor), StorePlacement::InLibrary, library_id)
			.await;
		assert_eq!(found, Some((id, StorePlacement::InLibrary)));
		let (again, existed) = cache
			.register_source_with(
				Some(LIBRARY),
				&root,
				Some(anchor.clone()),
				found.map(|(id, _)| id),
			)
			.await
			.unwrap();
		assert_eq!(again, id);
		assert!(!existed);

		// Another library, another drive, or another placement: not adopted.
		assert!(cache
			.portable_identity(
				&root,
				Some(&anchor),
				StorePlacement::InLibrary,
				Uuid::now_v7()
			)
			.await
			.is_none());
		let other = VolumeAnchor {
			uuid: Uuid::now_v7(),
			mount_point: root.clone(),
		};
		assert!(cache
			.portable_identity(&root, Some(&other), StorePlacement::InLibrary, library_id)
			.await
			.is_none());
		// An add asking for the other placement still finds the catalog that
		// exists, and learns where it is.
		assert_eq!(
			cache
				.portable_identity(&root, Some(&anchor), StorePlacement::OnSource, library_id)
				.await,
			Some((id, StorePlacement::InLibrary))
		);
	}
}
