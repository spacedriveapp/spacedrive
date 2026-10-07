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
	/// Set once a snapshot restore has populated the index this session.
	restored: AtomicBool,
	/// Single restore attempt per session: concurrent callers await the same
	/// load instead of racing three copies of a 100 MB deserialization.
	restore_once: tokio::sync::OnceCell<bool>,
	/// Serializes snapshot saves for this drive.
	save_lock: tokio::sync::Mutex<()>,
	/// Entry count at the last completed save; identical partitions skip the
	/// rewrite (a burst of browse jobs otherwise re-saves 100 MB per job).
	last_saved_entries: std::sync::atomic::AtomicU64,
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

pub struct VolumeIndex {
	/// Registered sources, in memory. The durable copy is the `sources` table
	/// in the open library.
	registry: Mutex<SourceRegistry>,
	/// Where registrations are written. Set when a library opens, cleared when
	/// it closes.
	///
	/// This cache is machine-scoped and a source registration is library
	/// metadata, so the two do not have the same lifetime. Until the registry
	/// is reachable from wherever a source is mutated, the cache follows the
	/// open library: with none open it serves paths from memory and persists
	/// nothing.
	db: RwLock<Option<Arc<Database>>>,
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
			registry: Mutex::new(SourceRegistry::default()),
			db: RwLock::new(None),
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
	pub async fn attach_library(&self, db: Arc<Database>) -> anyhow::Result<usize> {
		let rows = source::Entity::find()
			.filter(source::Column::DataType.eq(source::FILESYSTEM_DATA_TYPE))
			.all(db.conn())
			.await?;

		let mounts: HashMap<Uuid, PathBuf> = crate::infra::db::entities::volume::Entity::find()
			.all(db.conn())
			.await?
			.into_iter()
			.filter(|volume| volume.is_online)
			.filter_map(|volume| Some((volume.uuid, PathBuf::from(volume.mount_point.as_ref()?))))
			.collect();

		// Every online drive is mapped, whether or not anything is kept off it.
		// A source is a scope over one of these, not a replacement for it.
		for (uuid, mount_point) in &mounts {
			self.track_volume(*uuid, mount_point.clone());
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
			slot.set_detached(!volume_root.exists());
		}

		*self.registry.lock() = registry;
		*self.db.write() = Some(db);
		Ok(adopted)
	}

	/// Stop persisting; the library that owned these registrations is closing.
	pub fn detach_library(&self) {
		*self.db.write() = None;
		*self.registry.lock() = SourceRegistry::default();
		self.volumes.lock().clear();
		self.slots.write().clear();
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
	pub async fn register_source(
		&self,
		root: &Path,
		volume: Option<VolumeAnchor>,
	) -> anyhow::Result<Uuid> {
		let record = self.registry.lock().register(root, volume.as_ref());
		self.persist(&record).await?;

		// The partition belongs to the drive, so registering a source over an
		// already-mapped one joins it rather than starting a second.
		let (volume, volume_root) = self.registry.lock().volume_of(&record);
		let slot = self.slot_for(&Resolved {
			volume,
			volume_root: volume_root.clone(),
			source: Some(record.clone()),
		});
		*slot.root.write() = Some(volume_root.clone());
		slot.set_detached(!volume_root.exists());
		Ok(record.id)
	}

	/// Write a record to the open library, if one is open.
	///
	/// With no library open there is nowhere durable to put it, and the
	/// registration lives only as long as the process. That is legitimate for a
	/// cache serving paths before a library exists, and a silent loss anywhere
	/// else, so it says so.
	async fn persist(&self, record: &SourceRecord) -> anyhow::Result<()> {
		let Some(db) = self.db.read().clone() else {
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

	/// All registered sources with their live state.
	pub fn sources(&self) -> Vec<SourceStatus> {
		let (records, volumes): (Vec<SourceRecord>, Vec<VolumeKey>) = {
			let registry = self.registry.lock();
			let records = registry.all().to_vec();
			let volumes = records
				.iter()
				.map(|record| registry.volume_of(record).0)
				.collect();
			(records, volumes)
		};
		let slots = self.slots.read();
		records
			.into_iter()
			.zip(volumes)
			.map(|(record, volume)| {
				let slot = slots.get(&volume);
				SourceStatus {
					attached: record.root.exists(),
					restored: slot
						.map(|s| s.restored.load(Ordering::Acquire))
						.unwrap_or(false),
					directory: self.dirs.as_ref().map(|d| d.source_dir(record.id)),
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

	/// Where a source's index snapshot lives on disk. The snapshot belongs to
	/// the drive's volume index rather than the source's own directory, so
	/// resolving it goes through the registry's volume assignment.
	pub fn source_snapshot_path(&self, source_id: Uuid) -> Option<PathBuf> {
		let dirs = self.dirs.as_ref()?;
		let registry = self.registry.lock();
		let record = registry.all().iter().find(|r| r.id == source_id)?;
		let (volume, _) = registry.volume_of(record);
		Some(dirs.snapshot_file(volume.id()))
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
	pub fn track_volume(&self, uuid: Uuid, mount_point: PathBuf) {
		let mut volumes = self.volumes.lock();
		match volumes.iter_mut().find(|tracked| tracked.uuid == uuid) {
			Some(tracked) => tracked.mount_point = mount_point,
			None => volumes.push(TrackedVolume { uuid, mount_point }),
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
		let located = {
			let registry = self.registry.lock();
			registry.resolve(path).cloned().map(|record| {
				let (volume, volume_root) = registry.volume_of(&record);
				Resolved {
					volume,
					volume_root,
					source: Some(record),
				}
			})
		};
		if located.is_some() {
			return located;
		}

		self.volumes
			.lock()
			.iter()
			.filter(|tracked| path.starts_with(&tracked.mount_point))
			.max_by_key(|tracked| tracked.mount_point.as_os_str().len())
			.map(|tracked| Resolved {
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
				slot.set_detached(!resolved.volume_root.exists());
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
			.registry
			.lock()
			.resolve(path)
			.map(|record| record.config.unfiltered)
			.unwrap_or(false);
		if unfiltered {
			crate::ops::indexing::rules::RuleToggles::none()
		} else {
			crate::ops::indexing::rules::RuleToggles::default()
		}
	}

	/// A registered source's display name, by id.
	pub fn source_name(&self, id: Uuid) -> Option<String> {
		self.registry
			.lock()
			.by_id(id)
			.map(|record| record.name.clone())
	}

	/// A registered source's capture policy, by id.
	pub fn source_config(&self, id: Uuid) -> Option<SourceConfig> {
		self.registry
			.lock()
			.by_id(id)
			.map(|record| record.config.clone())
	}

	/// Update a source's capture policy, persisting the change.
	pub async fn set_source_config(&self, id: Uuid, config: SourceConfig) {
		let updated = self.registry.lock().set_config(id, config);
		if let Some(updated) = updated {
			if let Err(err) = self.persist(&updated).await {
				tracing::error!(source = %id, %err, "could not persist capture policy");
			}
		}
	}

	/// Rename a source, persisting the change.
	pub async fn set_source_name(&self, id: Uuid, name: String) {
		let updated = self.registry.lock().set_name(id, name);
		if let Some(updated) = updated {
			if let Err(err) = self.persist(&updated).await {
				tracing::error!(source = %id, %err, "could not persist the rename");
			}
		}
	}

	/// The root of the source owning `path`, when one does.
	pub fn source_root_for(&self, path: &Path) -> Option<PathBuf> {
		self.registry
			.lock()
			.resolve(path)
			.map(|record| record.root.clone())
	}

	/// The id of the source owning `path`, when one does. The innermost
	/// registered source wins, as it does for [`Self::store_for`].
	pub fn source_id_for(&self, path: &Path) -> Option<Uuid> {
		self.registry.lock().resolve(path).map(|record| record.id)
	}

	/// A registered source's current absolute root, by id.
	///
	/// The registry is the authority: the library row stores the root
	/// relative to its volume, so anything that turns a source id into a
	/// path to open must ask here rather than read the row.
	pub fn source_root(&self, id: Uuid) -> Option<PathBuf> {
		self.registry
			.lock()
			.by_id(id)
			.map(|record| record.root.clone())
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
		let record = self.registry.lock().resolve(path).cloned()?;
		let dirs = self.dirs.as_ref()?;

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

		let store = match SourceStore::open(dirs, record.id, record.root.clone()).await {
			Ok(store) => store,
			Err(error) => {
				tracing::error!(source = %record.id, %error, "source store unavailable");
				return None;
			}
		};

		self.stores.write().insert(record.id, store.clone());
		Some(store)
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

		let manager = sd_store::SourceManager::new(dirs.root().to_path_buf());
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
			.registry
			.lock()
			.all()
			.iter()
			.map(|record| record.root.clone())
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
		let restored = *slot
			.restore_once
			.get_or_init(|| Self::attempt_restore(self.dirs.clone(), slot.clone()))
			.await;

		if restored && !already_restored {
			self.announce_restored(&root);
		}

		restored || !slot.indexed_paths.read().is_empty()
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
			self.registry
				.lock()
				.all()
				.iter()
				.filter(|source| source.is_locatable())
				.map(|source| source.root.clone()),
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
	async fn attempt_restore(dirs: Option<SourceDirs>, slot: Arc<Partition>) -> bool {
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
		slot.set_detached(!meta.root_path.exists());

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
		let updated = self.registry.lock().update_stats(record.id, counts);
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
		let registered = {
			let registry = self.registry.lock();
			registry
				.all()
				.iter()
				.any(|record| record.root == path && record.root.exists())
		};
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
			cache
				.registry
				.lock()
				.resolve(Path::new("/mnt/drive/file.txt"))
				.map(|s| s.id),
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
				cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library).await.expect("attach");

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
				cache.attach_library(library.clone()).await.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				indexed_source(&cache, &root, anchor, 8).await;
			}

			// A new session, as a restarted daemon sees it.
			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache.attach_library(library).await.expect("attach");
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
				cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");

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
			cache.attach_library(library.clone()).await.expect("attach");
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
		/// A snapshot that will not parse restores nothing, but the source it
		/// belonged to stays registered and listed, its store still answers,
		/// and the unreadable artifact stays on disk for diagnosis.
		#[tokio::test]
		async fn an_invalid_snapshot_leaves_the_source_visible_and_its_store_readable() {
			let data = tempfile::tempdir().unwrap();
			let library = test_library(data.path()).await;
			let root_dir = tempfile::tempdir().unwrap();
			let root = root_dir.path().to_path_buf();
			std::fs::write(root.join("kept.txt"), b"kept").unwrap();

			let (id, snapshot_path) = {
				let cache =
					VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
				cache.attach_library(library.clone()).await.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let id = indexed_source(&cache, &root, anchor, 4).await;
				let store = cache
					.store_for(&root.join("kept.txt"))
					.await
					.expect("store");
				store
					.identify_one(&entry(&root.join("kept.txt")), None)
					.await
					.expect("identified");
				store.flush().await.expect("flush");
				(id, cache.snapshot_path_for(&root).expect("snapshot path"))
			};
			assert!(snapshot_path.exists());
			std::fs::write(&snapshot_path, b"this is not a snapshot").unwrap();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache.attach_library(library).await.expect("attach");
			assert!(
				!cache.ensure_restored(&root).await,
				"junk must not restore as an index"
			);
			assert!(
				cache.sources().iter().any(|s| s.id == id),
				"the source stays registered without its cache"
			);
			let db = cache.read_store(id).await.expect("the store still opens");
			assert!(
				db.resolve_path("kept.txt").await.expect("query").is_some(),
				"retained records answer without the arena"
			);
			assert!(
				!cache.arena_answers(&root),
				"a query over this source routes to the store, not an empty arena"
			);
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
				cache.attach_library(library.clone()).await.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				indexed_source(&cache, &root, anchor, 4).await;
				cache.snapshot_path_for(&root).expect("snapshot path")
			};
			std::fs::write(&snapshot_path, b"this is not a snapshot").unwrap();

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache.attach_library(library).await.expect("attach");
			assert!(!cache.ensure_restored(&root).await);
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
			cache.attach_library(library).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");

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
			cache.attach_library(library.clone()).await.expect("attach");

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
			cache.attach_library(library.clone()).await.expect("attach");

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
			cache.attach_library(library.clone()).await.expect("attach");

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
			cache.attach_library(library.clone()).await.expect("attach");

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
				cache.attach_library(library.clone()).await.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let id = indexed_source(&cache, &root, anchor, COLLAPSE_FLOOR * 2).await;
				let before = counted(&cache, id);
				(id, before)
			};

			// A new session, as a restarted daemon sees it.
			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache.attach_library(library).await.expect("attach");
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
				cache.attach_library(library.clone()).await.expect("attach");
				let anchor = tracked_volume(&library, &root).await;
				let id = indexed_source(&cache, &root, anchor, COLLAPSE_FLOOR * 3).await;
				let full = counted(&cache, id);
				(id, full)
			};

			let cache =
				VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).expect("cache");
			cache.attach_library(library).await.expect("attach");
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
		cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
		cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
			cache.attach_library(library.clone()).await.expect("attach");
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
}
