use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sd_bake::{BakePool, BakeRequest, ImageProducer, Producer, ScaleMode, Tile, WorkItem};
use sd_pvcache::{Frame, Pvcache, TileState, DEFAULT_INITIAL_CAPACITY};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::domain::resource::Identifiable;
use crate::infra::event::{Event, EventBus, ResourceMetadata};
use crate::infra::source_dirs::SourceDirs;
use crate::infra::source_version::source_version;
use crate::ops::indexing::ephemeral::EphemeralIndexCache;

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
const COMPLETION_FLUSH: Duration = Duration::from_millis(100);

/// Completions per event, so a large fill still lands in bounded batches.
const COMPLETION_BATCH: usize = 256;

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
	/// False when no producer could turn the file into pixels. The slot stays
	/// empty and no retry is coming for this version.
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

/// The key the pool echoes back on a finished bake.
#[derive(Debug, Clone, Copy)]
struct TileKey {
	source_id: Uuid,
	uuid: Uuid,
	version: u64,
}

pub struct ThumbService {
	/// Per-source directory layout; `None` means no persistence, so no cache.
	dirs: Option<SourceDirs>,
	ephemeral: Arc<EphemeralIndexCache>,
	/// One writer per source, opened on first use and held for the process
	/// lifetime. This map is the single-writer guarantee.
	writers: Mutex<HashMap<Uuid, Arc<Mutex<Pvcache>>>>,
	/// Queued or in-flight bakes, so a viewport redrawing every frame does
	/// not resubmit work already on the queue.
	pending: Arc<Mutex<HashSet<(Uuid, Uuid)>>>,
	pool: BakePool<TileKey>,
}

impl ThumbService {
	/// Build the service and start its completion drain.
	pub fn new(
		dirs: Option<SourceDirs>,
		ephemeral: Arc<EphemeralIndexCache>,
		events: Arc<EventBus>,
	) -> Arc<Self> {
		let (pool, baked) = BakePool::<TileKey>::new(producer_chain(), TILE, bake_workers());
		let pending: Arc<Mutex<HashSet<(Uuid, Uuid)>>> = Arc::new(Mutex::new(HashSet::new()));

		let service = Arc::new(Self {
			dirs,
			ephemeral,
			writers: Mutex::new(HashMap::new()),
			pending: pending.clone(),
			pool,
		});

		let drain = service.clone();
		std::thread::Builder::new()
			.name("thumb-drain".into())
			.spawn(move || drain.run_drain(baked, events, pending))
			.expect("failed to spawn thumbnail drain thread");

		service
	}

	/// Resolve `paths` to cache keys and queue a bake for every tile that is
	/// not already fresh. Earlier paths bake first, so a caller sends what it
	/// is drawing in the order it draws it.
	///
	/// A path under no registered source, or one the index does not know, has
	/// no identity and is reported as `None` in place.
	pub async fn request(&self, paths: &[PathBuf]) -> Vec<Option<TileIdentity>> {
		let mut out = Vec::with_capacity(paths.len());
		for (rank, path) in paths.iter().enumerate() {
			out.push(
				self.request_one(path, priority_for(rank, paths.len()))
					.await,
			);
		}
		out
	}

	async fn request_one(&self, path: &PathBuf, priority: u32) -> Option<TileIdentity> {
		let slot = self.ephemeral.resolve(path);
		let source_id = slot.id?;
		let dirs = self.dirs.as_ref()?;

		// The uuid has to come from the index rather than be derived here, so
		// it is the same identity the directory listing already handed the
		// client, and the version has to come from the file so a client's
		// timestamp rounding can never disagree with the writer's.
		let (uuid, version) = {
			let index = slot.index();
			let mut index = index.write().await;
			let uuid = index.get_or_assign_uuid(path);
			let metadata = std::fs::metadata(path).ok()?;
			let mtime = metadata.modified().ok()?;
			(uuid, source_version(metadata.len(), mtime))
		};

		let identity = TileIdentity {
			source_id,
			uuid,
			version,
		};

		let writer = self.writer_for(source_id, dirs)?;
		let fresh = {
			let writer = writer.lock().unwrap_or_else(|e| e.into_inner());
			matches!(writer.lookup(uuid, version), TileState::Fresh { .. })
		};
		if fresh {
			return Some(identity);
		}

		{
			let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
			if !pending.insert((source_id, uuid)) {
				return Some(identity);
			}
		}

		self.pool.submit(BakeRequest {
			key: TileKey {
				source_id,
				uuid,
				version,
			},
			item: WorkItem::file(path),
			priority,
		});
		Some(identity)
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
		pending: Arc<Mutex<HashSet<(Uuid, Uuid)>>>,
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
					pending
						.lock()
						.unwrap_or_else(|e| e.into_inner())
						.remove(&(key.source_id, key.uuid));
					let ok = self.store(&key, done.result);
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
				let mut writer = writer.lock().unwrap_or_else(|e| e.into_inner());
				match writer.write(key.uuid, key.version, frame, tile.bgra()) {
					Ok(()) => true,
					Err(error) => {
						warn!("thumbnail write for {}: {error}", key.uuid);
						false
					}
				}
			}
			Err(declines) => {
				debug!("no producer for {}: {declines:?}", key.uuid);
				false
			}
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
fn priority_for(rank: usize, total: usize) -> u32 {
	(total.saturating_sub(rank)) as u32
}

/// Raster decode, then (on macOS) the platform icon tile so a file no decoder
/// recognizes still gets a real cell instead of a permanent placeholder.
///
/// Decoded content is aspect-fit: the grid draws a thumbnail at the file's own
/// proportions, and a square view crops back out of that. Baking the crop
/// instead would throw away the shape and leave the grid nothing to recover it
/// from. Icon tiles are square by nature and unaffected.
fn producer_chain() -> Vec<Box<dyn Producer>> {
	let mut chain: Vec<Box<dyn Producer>> = vec![Box::new(ImageProducer::new(ScaleMode::Fit))];
	#[cfg(target_os = "macos")]
	chain.push(Box::new(sd_bake::IconProducer::new()));
	chain
}

/// Decode workers: leave headroom for the daemon's own work and the client's
/// frame loop.
fn bake_workers() -> usize {
	std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.saturating_sub(2)
		.clamp(2, 8)
}

#[cfg(test)]
mod tests {
	use super::*;

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
