use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sd_bake::{BakePool, BakeRequest, ImageProducer, Producer, ScaleMode, Tile, WorkItem};
use sd_pvcache::{Frame, Pvcache, TileState, DEFAULT_INITIAL_CAPACITY};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{mpsc, watch};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::domain::resource::Identifiable;
use crate::infra::event::{Event, EventBus, ResourceMetadata};
use crate::infra::source_dirs::SourceDirs;
use crate::infra::source_version::source_version;
use crate::ops::indexing::VolumeIndex;
use crate::service::external_tools::ExternalTools;

use super::ffmpeg::HostFfmpegProducer;
use super::sidecars::{self, SidecarRow, SidecarStore, StoredTile};
use super::{
	ffmpeg::is_video,
	thumbstrip::{self, COLUMNS as THUMBSTRIP_COLUMNS, ROWS as THUMBSTRIP_ROWS},
};

#[cfg(target_os = "macos")]
use super::platform::PlatformProducer;

/// Envelope edge in physical pixels: the largest frame a slot can hold, on
/// either axis. One geometry per cache file; the reader rejects a file baked
/// at any other size.
///
/// Tiles are aspect-fit inside this envelope, so the number that matters is
/// what it leaves on the short side. A square crop taken back out of a frame
/// with ratio `r` measures `TILE / r`, so an envelope of `256 × r` is what
/// keeps that crop at 256. At 384 a 3:2 frame stores 384×256 and crops to 256
/// exactly; wider frames fall off from there rather than off a cliff.
pub const TILE: u32 = 384;

/// How long completions collect before they go out as one event. A folder
/// filling at a few hundred tiles a second would otherwise put a few hundred
/// events a second on the bus, and every client wakes for each one.
pub(super) const COMPLETION_FLUSH: Duration = Duration::from_millis(100);

/// Completions per event, so a large fill still lands in bounded batches.
pub(super) const COMPLETION_BATCH: usize = 256;

/// How long a tile asked of a peer goes unasked again. Clients request every
/// cell each time it mounts, and without this each scroll would cost the
/// owner a stat per file; a file changed on the owner shows its new tile
/// within this window.
const REMOTE_REASK: Duration = Duration::from_secs(10 * 60);

/// Claims kept before expired ones are swept.
const REMOTE_CLAIMS_RETAINED: usize = 65_536;

/// Bump when the tile recipe changes, independently of the source file.
const THUMBNAIL_RECIPE: u64 = 2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailGenerationMode {
	/// Only fill empty slots, retaining even stale tiles.
	Missing,
	/// Fill empty slots and replace outdated tiles.
	#[default]
	Stale,
	/// Bake again even when the tile is current.
	Force,
}

impl ThumbnailGenerationMode {
	fn should_bake(self, state: TileState) -> bool {
		match self {
			Self::Missing => matches!(state, TileState::Absent),
			Self::Stale => !matches!(state, TileState::Fresh { .. }),
			Self::Force => true,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationOutcome {
	Generated,
	Skipped,
	Failed,
}

struct PendingBake {
	key: TileKey,
	completion: watch::Sender<Option<bool>>,
}

enum BakeSubmission {
	Skipped,
	Pending(watch::Receiver<Option<bool>>),
}

/// A baked tile, announced when it lands so clients re-read the slot.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Thumbnail {
	/// The entry the tile depicts; the cache key.
	pub id: Uuid,
	pub source_id: Uuid,
	/// Content version the tile was baked at. A string on the wire: the value
	/// exceeds what a JSON number survives in a browser.
	#[serde(with = "crate::infra::wire::u64_string")]
	#[specta(type = String)]
	pub version: u64,
	/// False when generation or publication failed. A previous tile is retained.
	pub ok: bool,
}

impl Identifiable for Thumbnail {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str
	where
		Self: Sized,
	{
		Thumbnail::RESOURCE_TYPE
	}
}

impl Thumbnail {
	/// The resource type string clients filter events by, available without
	/// importing [`Identifiable`].
	pub const RESOURCE_TYPE: &'static str = "thumbnail";
}

crate::register_resource!(Thumbnail);

/// Completion event for an on-demand video scrub sheet.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Thumbstrip {
	pub id: Uuid,
	pub source_id: Uuid,
	#[serde(with = "crate::infra::wire::u64_string")]
	#[specta(type = String)]
	pub version: u64,
	pub ok: bool,
}

impl Identifiable for Thumbstrip {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str
	where
		Self: Sized,
	{
		Self::RESOURCE_TYPE
	}
}

impl Thumbstrip {
	pub const RESOURCE_TYPE: &'static str = "thumbstrip";
}

crate::register_resource!(Thumbstrip);

/// What a path resolves to in the cache: which file holds its tile, under
/// which key, at which version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub struct TileIdentity {
	pub source_id: Uuid,
	pub uuid: Uuid,
	/// A string on the wire: the value exceeds what a JSON number survives in
	/// a browser, and a rounded version matches no tile.
	#[serde(with = "crate::infra::wire::u64_string")]
	#[specta(type = String)]
	pub version: u64,
}

/// A tile's pixels as a cache holds them: BGRA8 rows packed at
/// `frame.content_width * 4` bytes, `frame.len()` bytes in all.
pub(crate) struct TilePixels {
	pub version: u64,
	pub frame: Frame,
	pub bgra: Vec<u8>,
}

/// Address and state for one volume-scoped video scrub sheet.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub struct ThumbstripIdentity {
	pub source_id: Uuid,
	pub uuid: Uuid,
	#[serde(with = "crate::infra::wire::u64_string")]
	#[specta(type = String)]
	pub version: u64,
	pub columns: u8,
	pub rows: u8,
	pub ready: bool,
	pub pending: bool,
	pub available: bool,
}

/// The key the pool echoes back on a finished bake.
#[derive(Debug, Clone, Copy)]
struct TileKey {
	source_id: Uuid,
	uuid: Uuid,
	version: u64,
	attempt: Uuid,
	/// The registered source whose sidecars keep the tile, when one owns the
	/// file.
	sidecar: Option<Uuid>,
}

/// A tile baked here, on its way to its source's sidecars.
struct SidecarWrite {
	source: Uuid,
	uuid: Uuid,
	tile: TilePixels,
}

/// A tile to put back in the hot cache from its source's sidecars.
struct Refill {
	path: PathBuf,
	identity: TileIdentity,
	store: Arc<SidecarStore>,
}

struct ThumbstripWork {
	path: PathBuf,
	output: PathBuf,
	identity: TileIdentity,
}

/// Keeps every sidecar file closed; see [`ThumbService::hold_sidecars`].
pub struct SidecarHold<'a> {
	_open: tokio::sync::MutexGuard<'a, HashMap<PathBuf, Arc<SidecarStore>>>,
}

pub struct ThumbService {
	/// Per-source directory layout; `None` means no persistence, so no cache.
	dirs: Option<SourceDirs>,
	volume_index: Arc<VolumeIndex>,
	/// One writer per source, opened on first use and held for the process
	/// lifetime. This map is the single-writer guarantee.
	writers: Mutex<HashMap<Uuid, Arc<Mutex<Pvcache>>>>,
	/// Queued or in-flight bakes, so a viewport redrawing every frame does
	/// not resubmit work already on the queue.
	pending: Arc<Mutex<HashMap<(Uuid, Uuid), PendingBake>>>,
	pool: BakePool<TileKey>,
	external_tools: Arc<ExternalTools>,
	thumbstrip_pending: Arc<Mutex<HashSet<(Uuid, Uuid, u64)>>>,
	thumbstrip_tx: async_channel::Sender<ThumbstripWork>,
	/// Where tiles fetched from a peer are announced, as bakes are by the
	/// drain.
	events: Arc<EventBus>,
	/// When each `(cache, uuid)` was last asked of a peer. See
	/// [`REMOTE_REASK`].
	remote_asked: Mutex<HashMap<(Uuid, Uuid), Instant>>,
	/// Open sidecar files by path: this device's, one per source, and its
	/// copies of peers'.
	sidecars: tokio::sync::Mutex<HashMap<PathBuf, Arc<SidecarStore>>>,
	/// Tiles baked here, queued for their sources' sidecars. `None` for a
	/// service built outside a runtime, which leaves nothing to write them on.
	sidecar_tx: Option<mpsc::UnboundedSender<SidecarWrite>>,
}

impl ThumbService {
	/// Build the service and start its completion drain.
	pub fn new(
		dirs: Option<SourceDirs>,
		volume_index: Arc<VolumeIndex>,
		events: Arc<EventBus>,
		external_tools: Arc<ExternalTools>,
	) -> Arc<Self> {
		let chain = producer_chain(external_tools.clone());
		Self::with_producers(dirs, volume_index, events, external_tools, chain)
	}

	fn with_producers(
		dirs: Option<SourceDirs>,
		volume_index: Arc<VolumeIndex>,
		events: Arc<EventBus>,
		external_tools: Arc<ExternalTools>,
		chain: Vec<Box<dyn Producer>>,
	) -> Arc<Self> {
		let (pool, baked) = BakePool::<TileKey>::new(chain, TILE, bake_workers());
		let pending = Arc::new(Mutex::new(HashMap::new()));
		let thumbstrip_pending = Arc::new(Mutex::new(HashSet::new()));
		let (thumbstrip_tx, thumbstrip_rx) = async_channel::bounded(128);
		let runtime = tokio::runtime::Handle::try_current().ok();
		let (sidecar_tx, sidecar_rx) = mpsc::unbounded_channel();

		let service = Arc::new(Self {
			dirs,
			volume_index,
			writers: Mutex::new(HashMap::new()),
			pending: pending.clone(),
			pool,
			external_tools: external_tools.clone(),
			thumbstrip_pending: thumbstrip_pending.clone(),
			thumbstrip_tx,
			events: events.clone(),
			remote_asked: Mutex::new(HashMap::new()),
			sidecars: tokio::sync::Mutex::new(HashMap::new()),
			sidecar_tx: runtime.is_some().then_some(sidecar_tx),
		});
		if let Some(runtime) = runtime {
			runtime.spawn(service.clone().run_sidecar_writer(sidecar_rx));
		}

		let drain = service.clone();
		let drain_events = events.clone();
		std::thread::Builder::new()
			.name("thumb-drain".into())
			.spawn(move || drain.run_drain(baked, drain_events))
			.expect("failed to spawn thumbnail drain thread");

		for worker in 0..thumbstrip_workers() {
			let receiver = thumbstrip_rx.clone();
			let tools = external_tools.clone();
			let events = events.clone();
			let pending = thumbstrip_pending.clone();
			std::thread::Builder::new()
				.name(format!("thumbstrip-{worker}"))
				.spawn(move || run_thumbstrip_worker(receiver, tools, events, pending))
				.expect("failed to spawn thumbstrip worker");
		}

		service
	}

	/// Resolve `paths` to cache keys and queue a bake for every tile that is
	/// not already fresh. Earlier paths bake first, so a caller sends what it
	/// is drawing in the order it draws it.
	///
	/// A path on no mapped volume, or one the index does not know, has
	/// no identity and is reported as `None` in place.
	pub async fn request(self: &Arc<Self>, paths: &[PathBuf]) -> Vec<Option<TileIdentity>> {
		let mut out = Vec::with_capacity(paths.len());
		let mut refills = Vec::new();
		for (rank, path) in paths.iter().enumerate() {
			out.push(
				self.request_one(path, priority_for(rank, paths.len()), &mut refills)
					.await,
			);
		}
		if !refills.is_empty() {
			tokio::spawn(self.clone().refill(refills, priority_for(0, paths.len())));
		}
		out
	}

	/// Queue a bake for one path, or set it aside to refill from its sidecar
	/// when that holds the tile at the version the cache lacks.
	async fn request_one(
		self: &Arc<Self>,
		path: &PathBuf,
		priority: u32,
		refills: &mut Vec<Refill>,
	) -> Option<TileIdentity> {
		let identity = self.thumbnail_identity(path).await?;
		if let Some(store) = self.sidecar_holding(path, identity).await {
			if !matches!(
				self.cache_state(identity).await,
				Some(TileState::Fresh { .. })
			) {
				refills.push(Refill {
					path: path.clone(),
					identity,
					store,
				});
			}
			return Some(identity);
		}
		let service = self.clone();
		let path = path.clone();
		tokio::task::spawn_blocking(move || {
			service.enqueue(path, identity, priority, ThumbnailGenerationMode::Stale)
		})
		.await
		.ok()??;
		Some(identity)
	}

	/// Put tiles back in the hot cache from their sidecars, announcing them
	/// as one batch. A tile whose sidecar has gone or will not decode is baked
	/// from its file instead.
	async fn refill(self: Arc<Self>, refills: Vec<Refill>, priority: u32) {
		let mut stored = Vec::with_capacity(refills.len());
		for refill in refills {
			let tile = refill.store.read(refill.identity.uuid).await.ok().flatten();
			stored.push((refill, tile));
		}
		let service = self.clone();
		let unfilled = tokio::task::spawn_blocking(move || {
			let mut tiles = Vec::new();
			let mut unfilled = Vec::new();
			for (refill, stored) in stored {
				match stored.and_then(|stored| restored(stored, refill.identity.version)) {
					Some(pixels) => {
						tiles.push((refill.identity.source_id, refill.identity.uuid, pixels))
					}
					None => unfilled.push(refill),
				}
			}
			service.write_tiles(tiles);
			unfilled
		})
		.await
		.unwrap_or_default();
		for refill in unfilled {
			let service = self.clone();
			let _ = tokio::task::spawn_blocking(move || {
				service.enqueue(
					refill.path,
					refill.identity,
					priority,
					ThumbnailGenerationMode::Stale,
				)
			})
			.await;
		}
	}

	/// Wait until pixels have reached the cache, rather than merely the queue.
	pub async fn generate_one(
		self: &Arc<Self>,
		path: &PathBuf,
		mode: ThumbnailGenerationMode,
	) -> GenerationOutcome {
		let Some(identity) = self.thumbnail_identity(path).await else {
			return GenerationOutcome::Failed;
		};
		let outcome = self.bake(path, identity, 0, mode).await;
		if outcome == GenerationOutcome::Skipped {
			self.keep_sidecar(path, identity).await;
		}
		outcome
	}

	/// Give a tile the hot cache already holds a sidecar, when its source has
	/// none at this version. Tiles baked before sidecars existed only have
	/// this way in, and encoding one from the cache is far cheaper than
	/// decoding its file again.
	async fn keep_sidecar(self: &Arc<Self>, path: &Path, identity: TileIdentity) {
		let Some(store) = self.local_sidecar(path).await else {
			return;
		};
		if store.version_of(identity.uuid) == Some(identity.version) {
			return;
		}
		let service = self.clone();
		let encoded = tokio::task::spawn_blocking(move || {
			let tile = service.read_fresh(identity)?;
			Some(StoredTile {
				version: tile.version,
				webp: sidecars::encode(&tile)?,
				frame: tile.frame,
			})
		})
		.await
		.ok()
		.flatten();
		if let Some(stored) = encoded {
			if let Err(error) = store.record(vec![(identity.uuid, stored)]).await {
				warn!("thumbnail sidecar for {}: {error:#}", identity.uuid);
			}
		}
	}

	/// `path`'s tile at `identity`, baked first when the cache holds none, for
	/// a peer that cannot read the file to bake its own.
	pub(super) async fn fresh_tile(
		self: &Arc<Self>,
		path: &PathBuf,
		identity: TileIdentity,
		priority: u32,
	) -> Option<TilePixels> {
		if self
			.bake(path, identity, priority, ThumbnailGenerationMode::Stale)
			.await == GenerationOutcome::Failed
		{
			return None;
		}
		let service = self.clone();
		tokio::task::spawn_blocking(move || service.read_fresh(identity))
			.await
			.ok()?
	}

	fn read_fresh(&self, identity: TileIdentity) -> Option<TilePixels> {
		let writer = self.writer_for(identity.source_id, self.dirs.as_ref()?)?;
		let writer = writer.lock().unwrap_or_else(|e| e.into_inner());
		let mut bgra = vec![0; writer.tile_len()];
		let Ok(TileState::Fresh { frame }) = writer.get(identity.uuid, identity.version, &mut bgra)
		else {
			return None;
		};
		bgra.truncate(frame.len());
		Some(TilePixels {
			version: identity.version,
			frame,
			bgra,
		})
	}

	/// The version of each tile a cache holds, `None` where it holds none.
	/// Blocks on the writers, so call it from the blocking pool.
	pub(super) fn held_versions(&self, tiles: &[(Uuid, Uuid)]) -> Vec<Option<u64>> {
		tiles
			.iter()
			.map(|&(cache_id, uuid)| {
				let writer = self.writer_for(cache_id, self.dirs.as_ref()?)?;
				let writer = writer.lock().unwrap_or_else(|e| e.into_inner());
				// A lookup reports the stored version whenever it differs from
				// the expected one, and matching it means the two are equal.
				match writer.lookup(uuid, 0) {
					TileState::Fresh { .. } => Some(0),
					TileState::Stale { version, .. } => Some(version),
					TileState::Absent => None,
				}
			})
			.collect()
	}

	/// Write tiles that did not come from a bake here, from a peer or a
	/// sidecar, and announce them. Each entry is `(cache, uuid, pixels)`.
	/// Blocks on the writers, so call it from the blocking pool.
	pub(super) fn write_tiles(&self, tiles: Vec<(Uuid, Uuid, TilePixels)>) {
		let Some(dirs) = self.dirs.as_ref() else {
			return;
		};
		let mut batch = Vec::with_capacity(tiles.len());
		for (cache_id, uuid, tile) in tiles {
			let Some(writer) = self.writer_for(cache_id, dirs) else {
				continue;
			};
			let written = writer.lock().unwrap_or_else(|e| e.into_inner()).write(
				uuid,
				tile.version,
				tile.frame,
				&tile.bgra,
			);
			match written {
				Ok(()) => batch.push(Thumbnail {
					id: uuid,
					source_id: cache_id,
					version: tile.version,
					ok: true,
				}),
				Err(error) => warn!("fetched thumbnail write for {uuid}: {error}"),
			}
		}
		flush(&mut batch, &self.events);
	}

	/// Claim the right to ask a peer about `(cache, uuid)`, refused while an
	/// earlier ask is within [`REMOTE_REASK`].
	pub(super) fn claim_remote(&self, cache_id: Uuid, uuid: Uuid) -> bool {
		let mut asked = self.remote_asked.lock().unwrap_or_else(|e| e.into_inner());
		let now = Instant::now();
		if asked
			.get(&(cache_id, uuid))
			.is_some_and(|at| now.duration_since(*at) < REMOTE_REASK)
		{
			return false;
		}
		// Browsing a large share visits more tiles than stay worth
		// remembering, so expired claims go before the map grows further.
		if asked.len() >= REMOTE_CLAIMS_RETAINED {
			asked.retain(|_, at| now.duration_since(*at) < REMOTE_REASK);
		}
		asked.insert((cache_id, uuid), now);
		true
	}

	/// Drop claims whose ask never got an answer, so the next request asks
	/// again instead of waiting out [`REMOTE_REASK`].
	pub(super) fn release_remote(&self, tiles: &[(Uuid, Uuid)]) {
		let mut asked = self.remote_asked.lock().unwrap_or_else(|e| e.into_inner());
		for tile in tiles {
			asked.remove(tile);
		}
	}

	/// Queue a bake and wait until its pixels have reached the cache, rather
	/// than merely the queue. A tile its source's sidecars hold at this
	/// version is restored from there instead, unless a bake is forced.
	async fn bake(
		self: &Arc<Self>,
		path: &PathBuf,
		identity: TileIdentity,
		priority: u32,
		mode: ThumbnailGenerationMode,
	) -> GenerationOutcome {
		if mode != ThumbnailGenerationMode::Force {
			if let Some(store) = self.sidecar_holding(path, identity).await {
				let Some(state) = self.cache_state(identity).await else {
					return GenerationOutcome::Failed;
				};
				if !mode.should_bake(state) {
					return GenerationOutcome::Skipped;
				}
				if self.refill_one(identity, &store).await {
					return GenerationOutcome::Generated;
				}
			}
		}
		let service = self.clone();
		let path = path.clone();
		let submission =
			tokio::task::spawn_blocking(move || service.enqueue(path, identity, priority, mode))
				.await;
		let Ok(Some(submission)) = submission else {
			return GenerationOutcome::Failed;
		};
		match submission {
			BakeSubmission::Skipped => GenerationOutcome::Skipped,
			BakeSubmission::Pending(mut completion) => loop {
				if let Some(ok) = *completion.borrow_and_update() {
					return if ok {
						GenerationOutcome::Generated
					} else {
						GenerationOutcome::Failed
					};
				}
				if completion.changed().await.is_err() {
					return GenerationOutcome::Failed;
				}
			},
		}
	}

	/// Restore one tile from its sidecar. `false` when it will not decode.
	async fn refill_one(self: &Arc<Self>, identity: TileIdentity, store: &SidecarStore) -> bool {
		let Ok(Some(stored)) = store.read(identity.uuid).await else {
			return false;
		};
		let service = self.clone();
		tokio::task::spawn_blocking(move || {
			let Some(pixels) = restored(stored, identity.version) else {
				return false;
			};
			service.write_tiles(vec![(identity.source_id, identity.uuid, pixels)]);
			true
		})
		.await
		.unwrap_or(false)
	}

	/// `path`'s tile as its source's sidecars hold it, when they hold it at
	/// `identity`'s version.
	pub(super) async fn stored_tile(
		&self,
		path: &Path,
		identity: TileIdentity,
	) -> Option<StoredTile> {
		let store = self.sidecar_holding(path, identity).await?;
		store.read(identity.uuid).await.ok().flatten()
	}

	/// The hot cache's state for `identity`, `None` when there is no cache.
	async fn cache_state(self: &Arc<Self>, identity: TileIdentity) -> Option<TileState> {
		let service = self.clone();
		tokio::task::spawn_blocking(move || {
			let writer = service.writer_for(identity.source_id, service.dirs.as_ref()?)?;
			let state = writer
				.lock()
				.unwrap_or_else(|e| e.into_inner())
				.lookup(identity.uuid, identity.version);
			Some(state)
		})
		.await
		.ok()
		.flatten()
	}

	/// The sidecars of `path`'s source, when they hold its tile at
	/// `identity`'s version.
	async fn sidecar_holding(
		&self,
		path: &Path,
		identity: TileIdentity,
	) -> Option<Arc<SidecarStore>> {
		let store = self.local_sidecar(path).await?;
		(store.version_of(identity.uuid) == Some(identity.version)).then_some(store)
	}

	/// The sidecars of the registered source that owns `path` here.
	async fn local_sidecar(&self, path: &Path) -> Option<Arc<SidecarStore>> {
		self.source_sidecars(self.volume_index.source_id_for(path)?)
			.await
	}

	/// A source's sidecars on this device, created on first use.
	async fn source_sidecars(&self, source: Uuid) -> Option<Arc<SidecarStore>> {
		let file = self.dirs.as_ref()?.sidecars_file(source);
		self.sidecar_store(&file).await
	}

	/// A sidecar file, opened on first use and kept open: one of this
	/// device's, or its copy of a peer's.
	pub(crate) async fn sidecar_store(&self, file: &Path) -> Option<Arc<SidecarStore>> {
		let mut open = self.sidecars.lock().await;
		if let Some(store) = open.get(file) {
			return Some(store.clone());
		}
		match SidecarStore::open(file).await {
			Ok(store) => {
				let store = Arc::new(store);
				open.insert(file.to_path_buf(), store.clone());
				Some(store)
			}
			Err(error) => {
				warn!("thumbnail sidecars {}: {error:#}", file.display());
				None
			}
		}
	}

	/// Close the open handles on these sources' sidecar files and keep every
	/// sidecar closed until the returned guard drops, so a restore can replace
	/// the files without a tile write reopening the old one mid-swap. The
	/// next tile request after the guard drops reopens whatever is on disk.
	pub async fn hold_sidecars(&self, sources: &[Uuid]) -> SidecarHold<'_> {
		let mut open = self.sidecars.lock().await;
		if let Some(dirs) = self.dirs.as_ref() {
			for source in sources {
				if let Some(store) = open.remove(&dirs.sidecars_file(*source)) {
					store.close().await;
				}
			}
		}
		SidecarHold { _open: open }
	}

	/// A source's sidecar store and how far it has written, as its listing
	/// advertises them. Nil and zero while it has none; a listing never
	/// creates one.
	pub async fn sidecar_mark(&self, source: Uuid) -> (Uuid, u64) {
		let Some(store) = self.existing_sidecars(source).await else {
			return (Uuid::nil(), 0);
		};
		store.mark().await.unwrap_or((Uuid::nil(), 0))
	}

	/// A source's sidecars written after `after`, oldest first, for a peer
	/// copying them, with the id of the store they come from.
	pub async fn sidecar_rows(
		&self,
		source: Uuid,
		after: u64,
		limit: u32,
	) -> anyhow::Result<(Uuid, Vec<SidecarRow>)> {
		let Some(store) = self.existing_sidecars(source).await else {
			return Ok((Uuid::nil(), Vec::new()));
		};
		let (id, _) = store.mark().await?;
		Ok((id, store.rows_after(after, limit).await?))
	}

	async fn existing_sidecars(&self, source: Uuid) -> Option<Arc<SidecarStore>> {
		let file = self.dirs.as_ref()?.sidecars_file(source);
		if !tokio::fs::try_exists(&file).await.unwrap_or(false) {
			return None;
		}
		self.sidecar_store(&file).await
	}

	/// Encode baked tiles and keep them in their sources' sidecars, a batch
	/// at a time.
	async fn run_sidecar_writer(
		self: Arc<Self>,
		mut queued: mpsc::UnboundedReceiver<SidecarWrite>,
	) {
		let mut batch = Vec::with_capacity(COMPLETION_BATCH);
		while queued.recv_many(&mut batch, COMPLETION_BATCH).await > 0 {
			let writes = std::mem::take(&mut batch);
			let encoded = tokio::task::spawn_blocking(move || {
				let mut by_source: HashMap<Uuid, Vec<(Uuid, StoredTile)>> = HashMap::new();
				for write in writes {
					if let Some(webp) = sidecars::encode(&write.tile) {
						by_source.entry(write.source).or_default().push((
							write.uuid,
							StoredTile {
								version: write.tile.version,
								frame: write.tile.frame,
								webp,
							},
						));
					}
				}
				by_source
			})
			.await
			.unwrap_or_default();
			for (source, tiles) in encoded {
				let Some(store) = self.source_sidecars(source).await else {
					continue;
				};
				if let Err(error) = store.record(tiles).await {
					warn!(%source, "thumbnail sidecars: {error:#}");
				}
			}
		}
	}

	fn enqueue(
		&self,
		path: PathBuf,
		identity: TileIdentity,
		priority: u32,
		mode: ThumbnailGenerationMode,
	) -> Option<BakeSubmission> {
		let dirs = self.dirs.as_ref()?;
		// Publication and queue admission share this lock so an old completion
		// cannot overwrite a newer request, or disappear before its write lands.
		let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
		let address = (identity.source_id, identity.uuid);
		let writer = self.writer_for(identity.source_id, dirs)?;
		let state = {
			let writer = writer.lock().unwrap_or_else(|e| e.into_inner());
			writer.lookup(identity.uuid, identity.version)
		};
		if !mode.should_bake(state) {
			return Some(BakeSubmission::Skipped);
		}
		if let Some(work) = pending.get(&address) {
			if work.key.version == identity.version {
				return Some(BakeSubmission::Pending(work.completion.subscribe()));
			}
		}
		let key = TileKey {
			source_id: identity.source_id,
			uuid: identity.uuid,
			version: identity.version,
			attempt: Uuid::new_v4(),
			sidecar: self.volume_index.source_id_for(&path),
		};
		let (completion, receiver) = watch::channel(None);
		if let Some(obsolete) = pending.insert(address, PendingBake { key, completion }) {
			let _ = obsolete.completion.send(Some(false));
		}
		self.pool.submit(BakeRequest {
			key,
			item: WorkItem::file(path),
			priority,
		});
		Some(BakeSubmission::Pending(receiver))
	}

	pub(super) async fn thumbnail_identity(&self, path: &PathBuf) -> Option<TileIdentity> {
		let mut identity = self.resolve_identity(path).await?;
		let video = is_video(path);
		let tools = self.external_tools.clone();
		let ffmpeg = if video {
			tokio::task::spawn_blocking(move || tools.ffmpeg_path().is_some())
				.await
				.ok()?
		} else {
			false
		};
		identity.version = thumbnail_version(identity.version, THUMBNAIL_RECIPE, ffmpeg);
		Some(identity)
	}

	/// Flush completed tiles before a job advances its persisted cursor.
	pub async fn flush_cache(self: &Arc<Self>, id: Uuid) -> Result<(), String> {
		let service = self.clone();
		tokio::task::spawn_blocking(move || {
			let writer = service
				.writers
				.lock()
				.unwrap_or_else(|e| e.into_inner())
				.get(&id)
				.cloned();
			if let Some(writer) = writer {
				writer
					.lock()
					.unwrap_or_else(|e| e.into_inner())
					.flush()
					.map_err(|error| error.to_string())?;
			}
			Ok(())
		})
		.await
		.map_err(|error| error.to_string())?
	}

	/// Resolve and queue one video thumbstrip. Generation starts only after a
	/// client asks, normally on hover, and a fresh artifact is returned without
	/// requiring FFmpeg to remain installed.
	pub async fn request_thumbstrip(&self, path: &PathBuf) -> Option<ThumbstripIdentity> {
		if !is_video(path) {
			return None;
		}
		let identity = self.resolve_identity(path).await?;
		let output = self.dirs.as_ref()?.thumbstrip_file(
			identity.source_id,
			identity.uuid,
			identity.version,
		);
		if output.is_file() {
			return Some(thumbstrip_identity(identity, true, false, true));
		}

		let available = self.external_tools.ffmpeg_path().is_some()
			&& self.external_tools.ffprobe_path().is_some();
		if !available {
			return Some(thumbstrip_identity(identity, false, false, false));
		}

		let key = (identity.source_id, identity.uuid, identity.version);
		let inserted = self
			.thumbstrip_pending
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.insert(key);
		if inserted
			&& self
				.thumbstrip_tx
				.try_send(ThumbstripWork {
					path: path.clone(),
					output,
					identity,
				})
				.is_err()
		{
			self.thumbstrip_pending
				.lock()
				.unwrap_or_else(|e| e.into_inner())
				.remove(&key);
			return Some(thumbstrip_identity(identity, false, false, true));
		}

		Some(thumbstrip_identity(identity, false, true, true))
	}

	async fn resolve_identity(&self, path: &PathBuf) -> Option<TileIdentity> {
		let slot = self.volume_index.resolve(path);
		// The hot tier follows the arena, so it is keyed by the drive rather
		// than by whatever is persisted off it.
		let source_id = slot.id()?;
		// Identity comes from the same index that produced the listing, while
		// the version comes from full filesystem precision.
		if slot.is_detached() {
			return None;
		}
		let metadata = tokio::fs::metadata(path).await.ok()?;
		if !metadata.is_file() {
			return None;
		}
		let version = source_version(metadata.len(), metadata.modified().ok()?);
		let uuid = {
			let index = slot.index();
			let mut index = index.write().await;
			if !index.has_entry(path) {
				return None;
			}
			index.get_or_assign_uuid(path)
		};
		Some(TileIdentity {
			source_id,
			uuid,
			version,
		})
	}

	/// The cache file for a source, or `None` when persistence is off.
	pub fn cache_path(&self, source_id: Uuid) -> Option<PathBuf> {
		Some(self.dirs.as_ref()?.thumbs_file(source_id))
	}

	fn writer_for(&self, source_id: Uuid, dirs: &SourceDirs) -> Option<Arc<Mutex<Pvcache>>> {
		let mut writers = self.writers.lock().unwrap_or_else(|e| e.into_inner());
		if let Some(writer) = writers.get(&source_id) {
			return Some(writer.clone());
		}
		let path = dirs.thumbs_file(source_id);
		if let Some(parent) = path.parent() {
			if let Err(error) = std::fs::create_dir_all(parent) {
				warn!("thumbnail cache directory {}: {error}", parent.display());
				return None;
			}
		}
		match Pvcache::open_or_create_with_capacity(&path, TILE, TILE, DEFAULT_INITIAL_CAPACITY) {
			Ok(writer) => {
				let writer = Arc::new(Mutex::new(writer));
				writers.insert(source_id, writer.clone());
				Some(writer)
			}
			Err(error) => {
				warn!("thumbnail cache {}: {error}", path.display());
				None
			}
		}
	}

	/// Write finished tiles and announce them in batches.
	fn run_drain(
		&self,
		baked: std::sync::mpsc::Receiver<sd_bake::Baked<TileKey>>,
		events: Arc<EventBus>,
	) {
		let mut batch: Vec<Thumbnail> = Vec::new();
		let mut oldest: Option<Instant> = None;

		loop {
			let timeout = oldest
				.map(|first| COMPLETION_FLUSH.saturating_sub(first.elapsed()))
				.unwrap_or(COMPLETION_FLUSH);
			match baked.recv_timeout(timeout) {
				Ok(done) => {
					let key = done.key;
					let Some(ok) = self.finish_bake(&key, done.result) else {
						continue;
					};
					batch.push(Thumbnail {
						id: key.uuid,
						source_id: key.source_id,
						version: key.version,
						ok,
					});
					oldest.get_or_insert_with(Instant::now);
					if batch.len() >= COMPLETION_BATCH {
						flush(&mut batch, &events);
						oldest = None;
					}
				}
				Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
					if !batch.is_empty() {
						flush(&mut batch, &events);
					}
					oldest = None;
				}
				// The pool is gone; the daemon is shutting down.
				Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
					if !batch.is_empty() {
						flush(&mut batch, &events);
					}
					return;
				}
			}
		}
	}

	fn finish_bake(
		&self,
		key: &TileKey,
		result: Result<Tile, Vec<sd_bake::Decline>>,
	) -> Option<bool> {
		let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
		let address = (key.source_id, key.uuid);
		if !pending
			.get(&address)
			.is_some_and(|work| work.key.attempt == key.attempt)
		{
			return None;
		}
		let ok = self.store(key, result);
		if let Some(work) = pending.remove(&address) {
			let _ = work.completion.send(Some(ok));
		}
		Some(ok)
	}

	/// Put one finished bake in its slot. Returns whether pixels landed.
	fn store(&self, key: &TileKey, result: Result<Tile, Vec<sd_bake::Decline>>) -> bool {
		let Some(dirs) = self.dirs.as_ref() else {
			return false;
		};
		let Some(writer) = self.writer_for(key.source_id, dirs) else {
			return false;
		};
		match result {
			Ok(tile) => {
				let frame = Frame {
					content_width: tile.width(),
					content_height: tile.height(),
					source_width: tile.source_width(),
					source_height: tile.source_height(),
				};
				let written = writer.lock().unwrap_or_else(|e| e.into_inner()).write(
					key.uuid,
					key.version,
					frame,
					tile.bgra(),
				);
				if let Err(error) = written {
					warn!("thumbnail write for {}: {error}", key.uuid);
					return false;
				}
				// Every tile baked here is kept in its source's sidecars too.
				// The writer encodes off this thread, which holds up
				// publication for as long as it runs.
				if let (Some(source), Some(queue)) = (key.sidecar, &self.sidecar_tx) {
					let _ = queue.send(SidecarWrite {
						source,
						uuid: key.uuid,
						tile: TilePixels {
							version: key.version,
							frame,
							bgra: tile.into_bgra(),
						},
					});
				}
				true
			}
			Err(declines) => {
				debug!("no producer for {}: {declines:?}", key.uuid);
				false
			}
		}
	}
}

/// A sidecar's tile as cache pixels, when it is the version wanted and
/// decodes.
fn restored(stored: StoredTile, version: u64) -> Option<TilePixels> {
	if stored.version != version {
		return None;
	}
	let bgra = sidecars::decode(&stored.webp, stored.frame)?;
	Some(TilePixels {
		version: stored.version,
		frame: stored.frame,
		bgra,
	})
}

fn thumbnail_version(source: u64, recipe: u64, ffmpeg: bool) -> u64 {
	let mut hash = blake3::Hasher::new();
	hash.update(b"spacedrive.thumbnail");
	hash.update(&source.to_le_bytes());
	hash.update(&recipe.to_le_bytes());
	hash.update(&[u8::from(ffmpeg)]);
	u64::from_le_bytes(hash.finalize().as_bytes()[..8].try_into().unwrap())
}

fn thumbstrip_identity(
	identity: TileIdentity,
	ready: bool,
	pending: bool,
	available: bool,
) -> ThumbstripIdentity {
	ThumbstripIdentity {
		source_id: identity.source_id,
		uuid: identity.uuid,
		version: identity.version,
		columns: THUMBSTRIP_COLUMNS,
		rows: THUMBSTRIP_ROWS,
		ready,
		pending,
		available,
	}
}

fn run_thumbstrip_worker(
	receiver: async_channel::Receiver<ThumbstripWork>,
	tools: Arc<ExternalTools>,
	events: Arc<EventBus>,
	pending: Arc<Mutex<HashSet<(Uuid, Uuid, u64)>>>,
) {
	while let Ok(work) = receiver.recv_blocking() {
		let result = thumbstrip::generate(&tools, &work.path, &work.output);
		let ok = result.is_ok();
		if let Err(error) = result {
			warn!(path = %work.path.display(), "thumbstrip generation failed: {error}");
		}
		pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&(
			work.identity.source_id,
			work.identity.uuid,
			work.identity.version,
		));
		let resource = Thumbstrip {
			id: work.identity.uuid,
			source_id: work.identity.source_id,
			version: work.identity.version,
			ok,
		};
		if let Ok(resource) = serde_json::to_value(resource) {
			events.emit(Event::ResourceChanged {
				resource_type: Thumbstrip::RESOURCE_TYPE.to_string(),
				resource,
				metadata: Some(ResourceMetadata {
					no_merge_fields: Vec::new(),
					alternate_ids: Vec::new(),
					affected_paths: Vec::new(),
				}),
			});
		}
	}
}

fn flush(batch: &mut Vec<Thumbnail>, events: &EventBus) {
	let resources: Vec<serde_json::Value> = batch
		.drain(..)
		.filter_map(|thumbnail| serde_json::to_value(thumbnail).ok())
		.collect();
	if resources.is_empty() {
		return;
	}
	events.emit(Event::ResourceChangedBatch {
		resource_type: Thumbnail::RESOURCE_TYPE.to_string(),
		resources: serde_json::Value::Array(resources),
		metadata: Some(ResourceMetadata {
			no_merge_fields: Vec::new(),
			alternate_ids: Vec::new(),
			affected_paths: Vec::new(),
		}),
	});
}

/// Bake priority from a request's ordering: the first path is the most
/// urgent, and every path in one request outranks a later request's tail.
pub(super) fn priority_for(rank: usize, total: usize) -> u32 {
	(total.saturating_sub(rank)) as u32
}

/// Raster decode, then (on macOS) the platform icon tile so a file no decoder
/// recognizes still gets a real cell instead of a permanent placeholder.
///
/// Decoded content is aspect-fit: the grid draws a thumbnail at the file's own
/// proportions, and a square view crops back out of that. Baking the crop
/// instead would throw away the shape and leave the grid nothing to recover it
/// from. Icon tiles are square by nature and unaffected.
fn producer_chain(external_tools: Arc<ExternalTools>) -> Vec<Box<dyn Producer>> {
	let mut chain: Vec<Box<dyn Producer>> = vec![Box::new(ImageProducer::new(ScaleMode::Fit))];
	#[cfg(target_os = "macos")]
	chain.push(Box::new(PlatformProducer));
	chain.push(Box::new(HostFfmpegProducer::new(external_tools)));
	#[cfg(target_os = "macos")]
	chain.push(Box::new(sd_bake::IconProducer::new()));
	chain
}

/// What one worker may need while a decode is in flight. A 4K video frame and
/// the decoder's reference frames fit inside this; an image is far smaller.
const DECODE_BUDGET: u64 = 1024 * 1024 * 1024;

/// Decode workers: leave headroom for the daemon's own work and the client's
/// frame loop.
///
/// Cores alone are the wrong bound on a host whose memory is already spoken
/// for. A NAS running ZFS gives its ARC nearly all of RAM and frees it only
/// under pressure, so a twelve thread box can hold under two gigabytes free;
/// eight decodes bursting at once there get the daemon killed rather than
/// slowed. Whichever of the two bounds is smaller wins.
fn bake_workers() -> usize {
	let cores = std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.saturating_sub(2);
	cores.min(available_decode_workers()).clamp(2, 8)
}

/// How many concurrent decodes the memory free right now would cover.
fn available_decode_workers() -> usize {
	use sysinfo::{MemoryRefreshKind, RefreshKind, System};

	let mut system = System::new_with_specifics(
		RefreshKind::new().with_memory(MemoryRefreshKind::new().with_ram()),
	);
	system.refresh_memory();
	usize::try_from(system.available_memory() / DECODE_BUDGET).unwrap_or(usize::MAX)
}

fn thumbstrip_workers() -> usize {
	std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(2)
		.saturating_sub(2)
		.clamp(1, 2)
}

#[cfg(test)]
pub(super) mod tests {
	use super::*;
	use crate::ops::indexing::{metadata::EntryMetadata, state::EntryKind};
	use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

	struct TestProducer {
		calls: Arc<AtomicUsize>,
		fail: Arc<AtomicBool>,
	}

	impl Producer for TestProducer {
		fn produce(&self, _: &WorkItem, _: u32) -> Result<Tile, sd_bake::Decline> {
			let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
			if self.fail.load(Ordering::SeqCst) {
				return Err(sd_bake::Decline::Unavailable);
			}
			Ok(Tile::solid(2, [call as u8, 0, 0, 255]))
		}
	}

	pub(crate) type Fixture = (
		tempfile::TempDir,
		Arc<ThumbService>,
		PathBuf,
		Arc<AtomicUsize>,
		Arc<AtomicBool>,
	);

	pub(crate) async fn fixture() -> Fixture {
		fixture_with(false).await
	}

	/// The fixture with its directory registered as a source, which is what
	/// gives its tiles somewhere to keep sidecars.
	pub(crate) async fn source_fixture() -> Fixture {
		fixture_with(true).await
	}

	async fn fixture_with(registered: bool) -> Fixture {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("file.png");
		tokio::fs::write(&path, b"test input").await.unwrap();
		let cache = Arc::new(VolumeIndex::with_sources_dir(None).unwrap());
		cache.track_volume(Uuid::new_v4(), temp.path().to_path_buf());
		if registered {
			cache.register_source(temp.path(), None).await.unwrap();
		}
		cache
			.resolve_index(&path)
			.write()
			.await
			.add_entry(
				path.clone(),
				Uuid::new_v4(),
				EntryMetadata {
					path: path.clone(),
					kind: EntryKind::File,
					size: 10,
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
		let calls = Arc::new(AtomicUsize::new(0));
		let fail = Arc::new(AtomicBool::new(false));
		let service = ThumbService::with_producers(
			Some(SourceDirs::under_data_dir(temp.path()).unwrap()),
			cache,
			Arc::new(EventBus::new(64)),
			Arc::new(ExternalTools::new()),
			vec![Box::new(TestProducer {
				calls: calls.clone(),
				fail: fail.clone(),
			})],
		);
		(temp, service, path, calls, fail)
	}

	async fn generate(
		service: &Arc<ThumbService>,
		path: &PathBuf,
		mode: ThumbnailGenerationMode,
	) -> GenerationOutcome {
		tokio::time::timeout(Duration::from_secs(5), service.generate_one(path, mode))
			.await
			.unwrap()
	}

	#[tokio::test]
	async fn thumbnail_modes_wait_for_publication_and_force_overwrites() {
		let (_temp, service, path, calls, _) = fixture().await;
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Missing).await,
			GenerationOutcome::Generated
		);
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let writer = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap();
		assert!(matches!(
			writer
				.lock()
				.unwrap()
				.lookup(identity.uuid, identity.version),
			TileState::Fresh { .. }
		));
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Missing).await,
			GenerationOutcome::Skipped
		);
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Stale).await,
			GenerationOutcome::Skipped
		);
		assert_eq!(calls.load(Ordering::SeqCst), 1);
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Force).await,
			GenerationOutcome::Generated
		);
		assert_eq!(calls.load(Ordering::SeqCst), 2);
		let mut pixels = vec![0; TILE as usize * TILE as usize * 4];
		writer
			.lock()
			.unwrap()
			.get(identity.uuid, identity.version, &mut pixels)
			.unwrap();
		assert_eq!(&pixels[..4], &[2, 0, 0, 255]);
		service.flush_cache(identity.source_id).await.unwrap();
		let mut reopened =
			sd_pvcache::PvcacheReader::open(&service.cache_path(identity.source_id).unwrap())
				.unwrap();
		assert!(matches!(
			reopened.lookup(identity.uuid, identity.version).unwrap(),
			TileState::Fresh { .. }
		));
	}

	/// The sidecars of the fixture's source, once they hold `identity`.
	async fn sidecar_for(
		service: &Arc<ThumbService>,
		path: &PathBuf,
		identity: TileIdentity,
	) -> Arc<SidecarStore> {
		tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				if let Some(store) = service.sidecar_holding(path, identity).await {
					return store;
				}
				tokio::time::sleep(Duration::from_millis(20)).await;
			}
		})
		.await
		.expect("the sidecar lands")
	}

	/// Replace the cached tile with an older version, as a changed recipe or
	/// a lost cache would leave it.
	fn stale(service: &Arc<ThumbService>, identity: TileIdentity) {
		let writer = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap();
		writer
			.lock()
			.unwrap()
			.write(
				identity.uuid,
				identity.version.wrapping_sub(1),
				Frame {
					content_width: 1,
					content_height: 1,
					source_width: 1,
					source_height: 1,
				},
				&[0, 0, 0, 255],
			)
			.unwrap();
	}

	#[tokio::test]
	async fn a_baked_tile_is_kept_and_refills_the_cache_without_baking_again() {
		let (_temp, service, path, calls, _) = source_fixture().await;
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Missing).await,
			GenerationOutcome::Generated
		);
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let store = sidecar_for(&service, &path, identity).await;
		assert_eq!(store.version_of(identity.uuid), Some(identity.version));
		assert_eq!(calls.load(Ordering::SeqCst), 1);

		stale(&service, identity);
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Stale).await,
			GenerationOutcome::Generated
		);
		assert_eq!(calls.load(Ordering::SeqCst), 1, "restored, not baked");
		let mut pixels = vec![0; TILE as usize * TILE as usize * 4];
		let state = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap()
			.lock()
			.unwrap()
			.get(identity.uuid, identity.version, &mut pixels)
			.unwrap();
		let TileState::Fresh { frame } = state else {
			panic!("the cache holds the current version again, got {state:?}");
		};
		assert_eq!((frame.content_width, frame.content_height), (2, 2));
		assert!(pixels[0].abs_diff(1) < 8, "the baked pixels came back");

		stale(&service, identity);
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Force).await,
			GenerationOutcome::Generated
		);
		assert_eq!(
			calls.load(Ordering::SeqCst),
			2,
			"a forced bake reads the file"
		);
	}

	#[tokio::test]
	async fn a_viewport_request_refills_from_the_sidecar() {
		let (_temp, service, path, calls, _) = source_fixture().await;
		generate(&service, &path, ThumbnailGenerationMode::Missing).await;
		let identity = service.thumbnail_identity(&path).await.unwrap();
		sidecar_for(&service, &path, identity).await;
		stale(&service, identity);

		let mut events = service.events.subscribe();
		assert_eq!(service.request(&[path.clone()]).await.len(), 1);
		let announced = tokio::time::timeout(Duration::from_secs(5), events.recv())
			.await
			.expect("the refill is announced")
			.unwrap();
		let Event::ResourceChangedBatch { resources, .. } = announced else {
			panic!("expected a thumbnail batch, got {announced:?}");
		};
		let announced: Vec<Thumbnail> = serde_json::from_value(resources).unwrap();
		assert_eq!(announced[0].version, identity.version);
		assert_eq!(calls.load(Ordering::SeqCst), 1, "restored, not baked");
	}

	#[tokio::test]
	async fn a_tile_cached_before_sidecars_gets_one_from_the_cache() {
		let (_temp, service, path, calls, _) = source_fixture().await;
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let writer = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap();
		let frame = Frame {
			content_width: 2,
			content_height: 2,
			source_width: 2,
			source_height: 2,
		};
		writer
			.lock()
			.unwrap()
			.write(
				identity.uuid,
				identity.version,
				frame,
				&[9, 9, 9, 255].repeat(4),
			)
			.unwrap();

		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Missing).await,
			GenerationOutcome::Skipped
		);
		assert_eq!(calls.load(Ordering::SeqCst), 0);
		let stored = service
			.stored_tile(&path, identity)
			.await
			.expect("the cached tile was kept");
		assert_eq!(stored.frame, frame);
	}

	#[tokio::test]
	async fn fetched_tiles_are_stored_under_the_owners_version_and_announced() {
		let (_temp, service, _, _, _) = fixture().await;
		let mut events = service.events.subscribe();
		let (cache, uuid) = (Uuid::new_v4(), Uuid::new_v4());
		assert_eq!(service.held_versions(&[(cache, uuid)]), [None]);

		let frame = Frame {
			content_width: 1,
			content_height: 1,
			source_width: 4,
			source_height: 4,
		};
		service.write_tiles(vec![(
			cache,
			uuid,
			TilePixels {
				version: 42,
				frame,
				bgra: vec![1, 2, 3, 255],
			},
		)]);
		assert_eq!(service.held_versions(&[(cache, uuid)]), [Some(42)]);

		let Ok(Event::ResourceChangedBatch { resources, .. }) = events.try_recv() else {
			panic!("the fetched tile was not announced");
		};
		let announced: Vec<Thumbnail> = serde_json::from_value(resources).unwrap();
		assert_eq!(announced.len(), 1);
		assert_eq!(
			(
				announced[0].id,
				announced[0].source_id,
				announced[0].version,
				announced[0].ok
			),
			(uuid, cache, 42, true)
		);
	}

	#[tokio::test]
	async fn a_remote_tile_is_asked_again_only_after_its_window_or_a_failure() {
		let (_temp, service, _, _, _) = fixture().await;
		let (cache, uuid) = (Uuid::new_v4(), Uuid::new_v4());
		assert!(service.claim_remote(cache, uuid));
		assert!(
			!service.claim_remote(cache, uuid),
			"asked within the window"
		);

		service.release_remote(&[(cache, uuid)]);
		assert!(service.claim_remote(cache, uuid), "a failed ask goes again");

		if let Some(expired) = Instant::now().checked_sub(REMOTE_REASK) {
			service
				.remote_asked
				.lock()
				.unwrap()
				.insert((cache, uuid), expired);
			assert!(service.claim_remote(cache, uuid), "the window has passed");
		}
	}

	#[tokio::test]
	async fn thumbnail_missing_retains_stale_but_stale_mode_replaces_it() {
		let (_temp, service, path, calls, _) = fixture().await;
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let writer = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap();
		writer
			.lock()
			.unwrap()
			.write(
				identity.uuid,
				identity.version.wrapping_sub(1),
				Frame {
					content_width: 1,
					content_height: 1,
					source_width: 1,
					source_height: 1,
				},
				&[99, 0, 0, 255],
			)
			.unwrap();
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Missing).await,
			GenerationOutcome::Skipped
		);
		assert_eq!(calls.load(Ordering::SeqCst), 0);
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Stale).await,
			GenerationOutcome::Generated
		);
		assert_eq!(calls.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn thumbnail_failed_force_keeps_previous_pixels() {
		let (_temp, service, path, _, fail) = fixture().await;
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Stale).await,
			GenerationOutcome::Generated
		);
		fail.store(true, Ordering::SeqCst);
		assert_eq!(
			generate(&service, &path, ThumbnailGenerationMode::Force).await,
			GenerationOutcome::Failed
		);
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let writer = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap();
		assert!(matches!(
			writer
				.lock()
				.unwrap()
				.lookup(identity.uuid, identity.version),
			TileState::Fresh { .. }
		));
	}

	#[tokio::test]
	async fn thumbnail_unknown_paths_do_not_get_transient_cache_keys() {
		let (temp, service, _, _, _) = fixture().await;
		let unknown = temp.path().join("unknown.png");
		tokio::fs::write(&unknown, b"file").await.unwrap();
		assert!(service.request(&[unknown]).await[0].is_none());
	}

	#[test]
	fn thumbnail_recipe_and_ffmpeg_availability_invalidate_existing_tiles() {
		let version = thumbnail_version(42, 1, false);
		assert_eq!(version, thumbnail_version(42, 1, false));
		assert_ne!(version, 42);
		assert_ne!(version, thumbnail_version(42, 1, true));
		assert_ne!(version, thumbnail_version(42, 2, false));
		assert_ne!(version, thumbnail_version(43, 1, false));
	}

	#[tokio::test]
	async fn thumbnail_obsolete_bake_cannot_overwrite_or_complete_newer_request() {
		let (_temp, service, path, _, _) = fixture().await;
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let old = TileKey {
			source_id: identity.source_id,
			uuid: identity.uuid,
			version: identity.version,
			attempt: Uuid::new_v4(),
			sidecar: None,
		};
		let new = TileKey {
			attempt: Uuid::new_v4(),
			..old
		};
		let (completion, receiver) = watch::channel(None);
		service.pending.lock().unwrap().insert(
			(new.source_id, new.uuid),
			PendingBake {
				key: new,
				completion,
			},
		);
		assert_eq!(
			service.finish_bake(&old, Ok(Tile::solid(2, [1, 0, 0, 255]))),
			None
		);
		assert_eq!(*receiver.borrow(), None);
		assert_eq!(
			service.finish_bake(&new, Ok(Tile::solid(2, [2, 0, 0, 255]))),
			Some(true)
		);
		assert_eq!(*receiver.borrow(), Some(true));
		assert_eq!(
			service.finish_bake(&old, Ok(Tile::solid(2, [1, 0, 0, 255]))),
			None
		);
		let writer = service
			.writer_for(identity.source_id, service.dirs.as_ref().unwrap())
			.unwrap();
		let mut pixels = vec![0; TILE as usize * TILE as usize * 4];
		writer
			.lock()
			.unwrap()
			.get(identity.uuid, identity.version, &mut pixels)
			.unwrap();
		assert_eq!(&pixels[..4], &[2, 0, 0, 255]);
	}

	#[test]
	fn earlier_paths_outrank_later_ones() {
		assert!(priority_for(0, 10) > priority_for(1, 10));
		assert!(priority_for(8, 10) > priority_for(9, 10));
	}

	#[test]
	fn a_full_request_outranks_nothing_it_does_not_contain() {
		// The tail of a long request still carries a priority, so a later
		// request's head can outrank it.
		assert!(priority_for(9, 10) >= 1);
	}
}
