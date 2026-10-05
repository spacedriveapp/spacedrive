//! Peer side of mounts: replicate a paired device's source indexes locally
//! and stream file bytes from it on demand.
//!
//! Metadata is replicated, never fetched per-call — the remote source's
//! snapshot is pulled once (and on explicit re-sync) and loaded into a local
//! arena, so browsing a peer's files costs the same as browsing a local
//! detached drive. Only `open`-time reads cross the network, over the
//! byterange protocol.

use crate::context::CoreContext;
use crate::ops::indexing::arena::Arena;
use crate::service::network::core::BYTERANGE_ALPN;
use crate::service::network::protocol::{
	byterange::{
		read_frame, write_frame, ByteRangeRequest, ByteRangeResponse, RemoteDeviceFacts,
		TileAnswer, TileAsk, MAX_READ_LEN, MAX_SIDECAR_PAGE, MAX_SNAPSHOT_LEN, MAX_TILE_LEN,
	},
	RemoteSourceInfo,
};
use crate::service::thumbs::sidecars::{SidecarRow, StoredTile};
use bytes::Bytes;
use futures::Stream;
use sd_pvcache::Frame;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

/// A paired device's source, replicated locally and readable on demand.
pub struct RemoteShare {
	pub device_id: Uuid,
	pub device_label: String,
	pub info: RemoteSourceInfo,
	pub index: Arc<TokioRwLock<Arena>>,
	pub synced_at_secs: u64,
	/// Snapshot generation this replica was built from. Compared against
	/// the owner's on every listing so an unchanged source is not
	/// re-downloaded.
	pub generation: u64,
}

static REMOTE_SHARES: OnceLock<TokioRwLock<HashMap<Uuid, Arc<RemoteShare>>>> = OnceLock::new();

fn shares_map() -> &'static TokioRwLock<HashMap<Uuid, Arc<RemoteShare>>> {
	REMOTE_SHARES.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

pub async fn remote_shares() -> Vec<Arc<RemoteShare>> {
	shares_map().read().await.values().cloned().collect()
}

pub async fn remote_share(source_id: Uuid) -> Option<Arc<RemoteShare>> {
	shares_map().read().await.get(&source_id).cloned()
}

/// The replica that holds `path` on the device `device_slug` names. The
/// deepest root wins, since a share at a volume's mount point also lists the
/// files of sources nested beneath it.
pub async fn share_for(
	context: &Arc<CoreContext>,
	device_slug: &str,
	path: &Path,
) -> Option<Arc<RemoteShare>> {
	remote_shares()
		.await
		.into_iter()
		.filter(|share| {
			path.starts_with(&share.info.root)
				&& context
					.device_manager
					.get_device_slug(share.device_id)
					.is_some_and(|slug| slug == device_slug)
		})
		.max_by_key(|share| share.info.root.components().count())
}

/// Where this device keeps what it replicates from `device_id`.
fn replica_dir(context: &CoreContext, device_id: Uuid) -> PathBuf {
	context
		.data_dir
		.join("mounts-remote")
		.join(device_id.simple().to_string())
}

/// This device's copy of a peer source's thumbnail sidecars.
pub fn sidecar_copy_file(context: &CoreContext, device_id: Uuid, source_id: Uuid) -> PathBuf {
	replica_dir(context, device_id).join(format!("{}.sidecars.db", source_id.simple()))
}

/// Open a replica's delivered database read-only, when one is on disk. The
/// caller closes the pool when done; a replica artifact is opened on demand
/// rather than held, since the next delivery replaces the file wholesale.
pub async fn open_replica_db(
	context: &Arc<crate::context::CoreContext>,
	device_id: Uuid,
	source_id: Uuid,
) -> Option<sd_store::SourceDb> {
	let db_path = replica_dir(context, device_id).join(format!("{}.db", source_id.simple()));
	if !db_path.exists() {
		return None;
	}
	match sd_store::SourceManager::open_file_read_only(&db_path).await {
		Ok(db) => Some(db),
		Err(error) => {
			tracing::warn!(source = %source_id, %error, "replica database would not open");
			None
		}
	}
}

/// A device's replica inventory and published facts as last synced, written
/// beside the artifacts so both survive a restart without the owner. A
/// replica has an owner and a validated generation even when no arena is
/// loaded; losing the list because the owner is unreachable would make a
/// drive's departure erase the knowledge that its copies exist. The facts
/// persist for the same reason: an offline device still owns its drives.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicaManifest {
	pub device_id: Uuid,
	pub device_label: String,
	pub sources: Vec<ReplicaEntry>,
	/// Absent until the owner has answered with facts.
	#[serde(default)]
	pub facts: Option<ObservedFacts>,
}

/// What a device published about itself, stamped with when this device
/// received it. The owner's clock never enters the stamp.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObservedFacts {
	pub facts: RemoteDeviceFacts,
	pub observed_at_secs: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicaEntry {
	pub info: RemoteSourceInfo,
	pub generation: u64,
	pub synced_at_secs: u64,
}

/// Every manifest read this session, kept current with every write so a
/// listing can name known-but-unloaded replicas without touching the disk.
static KNOWN_REPLICAS: OnceLock<TokioRwLock<HashMap<Uuid, ReplicaManifest>>> = OnceLock::new();

fn known_map() -> &'static TokioRwLock<HashMap<Uuid, ReplicaManifest>> {
	KNOWN_REPLICAS.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

/// Replicas known from persisted manifests whose arenas are not loaded right
/// now: the artifact is missing or unreadable. They stay visible as
/// unavailable rather than vanishing from the inventory.
pub async fn known_unloaded() -> Vec<(Uuid, String, ReplicaEntry)> {
	let loaded: std::collections::HashSet<Uuid> =
		shares_map().read().await.keys().copied().collect();
	known_map()
		.read()
		.await
		.values()
		.flat_map(|manifest| {
			manifest.sources.iter().filter_map(|entry| {
				(!loaded.contains(&entry.info.id)).then(|| {
					(
						manifest.device_id,
						manifest.device_label.clone(),
						entry.clone(),
					)
				})
			})
		})
		.collect()
}

/// What every paired device last published about itself. Read from the
/// manifests, so a device that is offline still describes itself as it
/// last did.
pub async fn published_facts() -> HashMap<Uuid, ObservedFacts> {
	known_map()
		.read()
		.await
		.iter()
		.filter_map(|(device_id, manifest)| Some((*device_id, manifest.facts.clone()?)))
		.collect()
}

/// Paired devices' volumes as this device lists them.
pub async fn published_volumes(context: &Arc<CoreContext>) -> Vec<crate::domain::volume::Volume> {
	let connected = match context.networking.read().await.clone() {
		Some(networking) => super::connected_devices(&networking)
			.await
			.into_iter()
			.map(|(device_id, _)| device_id)
			.collect(),
		None => std::collections::HashSet::new(),
	};
	volumes_from(&published_facts().await, &connected)
}

/// A volume reads as mounted only while its owner is connected. Otherwise
/// it stands as the owner last reported it, stamped with when that was.
fn volumes_from(
	facts: &HashMap<Uuid, ObservedFacts>,
	connected: &std::collections::HashSet<Uuid>,
) -> Vec<crate::domain::volume::Volume> {
	facts
		.iter()
		.flat_map(|(device_id, observed)| {
			let reachable = connected.contains(device_id);
			let observed_at = chrono::DateTime::from_timestamp(observed.observed_at_secs as i64, 0)
				.unwrap_or_default();
			observed
				.facts
				.volumes
				.iter()
				.map(move |volume| volume.to_volume(*device_id, reachable, observed_at))
		})
		.collect()
}

fn manifest_path(replica_dir: &std::path::Path) -> PathBuf {
	replica_dir.join("manifest.json")
}

async fn write_manifest(replica_dir: &std::path::Path, manifest: &ReplicaManifest) {
	match serde_json::to_vec_pretty(manifest) {
		Ok(bytes) => {
			if let Err(err) = tokio::fs::write(manifest_path(replica_dir), bytes).await {
				tracing::warn!(
					device = %manifest.device_id,
					%err,
					"replica manifest failed to persist; inventory will not survive a restart"
				);
				return;
			}
			known_map()
				.write()
				.await
				.insert(manifest.device_id, manifest.clone());
		}
		Err(err) => {
			tracing::warn!(device = %manifest.device_id, %err, "replica manifest failed to serialize");
		}
	}
}

/// Restore the known peer inventory from disk without contacting any owner.
///
/// Every manifest is read into the known set; every artifact that loads
/// becomes a live share exactly as a sync would have built it. An artifact
/// that is missing or unreadable leaves its source listed as unavailable,
/// and stays on disk for diagnosis. Returns `(loaded, known)`.
pub async fn restore_known_replicas(context: &Arc<CoreContext>) -> (usize, usize) {
	restore_from(&context.data_dir.join("mounts-remote")).await
}

async fn restore_from(base: &std::path::Path) -> (usize, usize) {
	let mut loaded = 0usize;
	let mut known = 0usize;

	let Ok(mut dirs) = tokio::fs::read_dir(base).await else {
		return (0, 0);
	};

	while let Ok(Some(dir)) = dirs.next_entry().await {
		let replica_dir = dir.path();
		sweep_legacy_temporaries(&replica_dir).await;
		let manifest = match tokio::fs::read(manifest_path(&replica_dir)).await {
			Ok(bytes) => match serde_json::from_slice::<ReplicaManifest>(&bytes) {
				Ok(manifest) => manifest,
				Err(err) => {
					tracing::warn!(path = %replica_dir.display(), %err, "unreadable replica manifest");
					continue;
				}
			},
			Err(_) => continue,
		};

		for entry in &manifest.sources {
			known += 1;
			if shares_map().read().await.contains_key(&entry.info.id) {
				loaded += 1;
				continue;
			}

			let index = match restore_artifact(&replica_dir, &entry.info).await {
				Some(index) => index,
				None => {
					tracing::warn!(
						source = %entry.info.id,
						dir = %replica_dir.display(),
						"replica artifact missing or unreadable; source stays listed as unavailable"
					);
					continue;
				}
			};

			let share = Arc::new(RemoteShare {
				device_id: manifest.device_id,
				device_label: manifest.device_label.clone(),
				generation: entry.generation,
				info: entry.info.clone(),
				index: Arc::new(TokioRwLock::new(index)),
				synced_at_secs: entry.synced_at_secs,
			});

			// A sync may have raced this restore with a fresher replica;
			// the restore never replaces what a sync installed.
			let mut shares = shares_map().write().await;
			if !shares.contains_key(&entry.info.id) {
				shares.insert(entry.info.id, share);
			}
			loaded += 1;
		}

		known_map()
			.write()
			.await
			.insert(manifest.device_id, manifest);
	}

	(loaded, known)
}

/// Builds before resumable transfers wrote to `<random>.tmp`, which nothing
/// can continue; a `.part` file is kept, since the next sync decides from its
/// name whether the owner still serves those bytes.
async fn sweep_legacy_temporaries(replica_dir: &Path) {
	let Ok(mut dir) = tokio::fs::read_dir(replica_dir).await else {
		return;
	};
	while let Ok(Some(entry)) = dir.next_entry().await {
		if entry.path().extension().is_some_and(|ext| ext == "tmp") {
			let _ = tokio::fs::remove_file(entry.path()).await;
		}
	}
}

/// Load a replica's artifact into an arena: the snapshot when one exists,
/// else a delivered database rebuilt entry by entry. Either way the share
/// serves identically; the artifact kind is a transport detail.
async fn restore_artifact(replica_dir: &std::path::Path, info: &RemoteSourceInfo) -> Option<Arena> {
	let snapshot_path = replica_dir.join(format!("{}.snapshot", info.id.simple()));
	if snapshot_path.exists() {
		let restored = {
			let path = snapshot_path.clone();
			tokio::task::spawn_blocking(move || Arena::load_snapshot(&path)).await
		};
		if let Ok(Ok(Some((index, _meta)))) = restored {
			return Some(index);
		}
	}

	let db_path = replica_dir.join(format!("{}.db", info.id.simple()));
	if db_path.exists() {
		if let Ok(db) = sd_store::SourceManager::open_file_read_only(&db_path).await {
			let built = arena_from_database(&db, &info.root).await;
			db.pool().close().await;
			match built {
				Ok(index) => return Some(index),
				Err(err) => {
					tracing::warn!(source = %info.id, %err, "replica database would not rebuild");
				}
			}
		}
	}
	None
}

/// Forget a revoked device's replicas: unload its shares, drop its manifest
/// from the known set, and move its artifacts out of the restore path.
///
/// Revocation removes the permission to serve and refresh this metadata, not
/// the evidence that it existed: the bytes move to `mounts-revoked` rather
/// than being deleted, and re-pairing rebuilds replicas through an ordinary
/// sync. Returns how many loaded shares were dropped.
pub async fn drop_device_replicas(context: &Arc<CoreContext>, device_id: Uuid) -> usize {
	let dropped = {
		let mut shares = shares_map().write().await;
		let ids: Vec<Uuid> = shares
			.iter()
			.filter(|(_, share)| share.device_id == device_id)
			.map(|(id, _)| *id)
			.collect();
		for id in &ids {
			shares.remove(id);
		}
		ids.len()
	};
	known_map().write().await.remove(&device_id);
	// Fleet totals must stop counting a device the library no longer trusts.
	summaries_map().write().await.remove(&device_id);

	let replica_dir = replica_dir(context, device_id);
	if tokio::fs::metadata(&replica_dir).await.is_ok() {
		let holding = context.data_dir.join("mounts-revoked");
		let _ = tokio::fs::create_dir_all(&holding).await;
		let target = holding.join(format!("{}-{}", device_id.simple(), now_secs()));
		if let Err(err) = tokio::fs::rename(&replica_dir, &target).await {
			tracing::warn!(
				device = %device_id,
				%err,
				"revoked replica artifacts could not be moved aside; they will restore next start"
			);
		}
	}
	dropped
}

/// Each paired device's own library statistics, refreshed with every sync.
/// This is what lets every device report the same fleet-wide totals: each
/// machine computes its own numbers and the others add them verbatim.
static DEVICE_SUMMARIES: OnceLock<
	TokioRwLock<HashMap<Uuid, crate::service::network::protocol::byterange::RemoteDeviceSummary>>,
> = OnceLock::new();

fn summaries_map() -> &'static TokioRwLock<
	HashMap<Uuid, crate::service::network::protocol::byterange::RemoteDeviceSummary>,
> {
	DEVICE_SUMMARIES.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

pub async fn device_summaries(
) -> HashMap<Uuid, crate::service::network::protocol::byterange::RemoteDeviceSummary> {
	summaries_map().read().await.clone()
}

/// Add every cached peer summary to a set of library statistics.
///
/// Fleet totals are never persisted: the local figures live in the library
/// config, and every surface that hands statistics to a client adds the peer
/// summaries through this one path — the read in `libraries.info` and the
/// `ResourceChanged` emission after a recalculation alike. A surface that
/// skips it publishes local-only numbers that overwrite the fleet ones in the
/// client's normalized cache.
pub async fn add_device_summaries(statistics: &mut crate::library::LibraryStatistics) {
	for summary in summaries_map().read().await.values() {
		statistics.total_files += summary.file_count;
		statistics.total_size += summary.total_size;
		statistics.unique_content_count += summary.unique_content_count;
		statistics.total_capacity += summary.total_capacity;
		statistics.available_capacity += summary.available_capacity;
	}
}

/// Open a fresh bi-stream to a paired device and send one request.
async fn request(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	req: &ByteRangeRequest,
) -> anyhow::Result<(
	ByteRangeResponse,
	Box<dyn tokio::io::AsyncRead + Send + Unpin>,
)> {
	let networking = context
		.networking
		.read()
		.await
		.clone()
		.ok_or_else(|| anyhow::anyhow!("networking service not available"))?;

	let node_id = {
		let registry = networking.device_registry();
		let registry = registry.read().await;
		registry
			.get_node_by_device(device_id)
			.ok_or_else(|| anyhow::anyhow!("device {device_id} is not connected"))?
	};
	let endpoint = networking
		.endpoint()
		.ok_or_else(|| anyhow::anyhow!("networking endpoint not available"))?
		.clone();

	let connection = endpoint
		.connect(iroh::EndpointAddr::new(node_id), BYTERANGE_ALPN)
		.await
		.map_err(|e| anyhow::anyhow!("connect failed: {e}"))?;
	let (mut send, mut recv) = connection
		.open_bi()
		.await
		.map_err(|e| anyhow::anyhow!("open_bi failed: {e}"))?;

	write_frame(&mut send, req).await?;
	// Half-close tells the server the request is complete.
	let _ = send.finish();

	let response: ByteRangeResponse = read_frame(&mut recv).await?;
	if let ByteRangeResponse::Error(err) = response {
		anyhow::bail!("peer refused: {err}");
	}
	Ok((response, Box::new(recv)))
}

/// Live size and mtime of a file on a peer. The mtime is what lets a cached
/// block be invalidated when the owning device changes the file.
pub async fn stat(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	path: PathBuf,
) -> anyhow::Result<(u64, Option<SystemTime>)> {
	let (response, _) = request(context, device_id, &ByteRangeRequest::Stat { path }).await?;
	match response {
		ByteRangeResponse::Stat {
			size,
			modified_secs,
		} => Ok((
			size,
			modified_secs.map(|secs| UNIX_EPOCH + Duration::from_secs(secs)),
		)),
		other => anyhow::bail!("unexpected response: {other:?}"),
	}
}

/// One bounded range read from a peer. Lengths above the protocol cap come
/// back short; callers loop.
pub async fn read_range(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	path: PathBuf,
	offset: u64,
	len: u64,
) -> anyhow::Result<Bytes> {
	let len = len.min(MAX_READ_LEN);
	let (response, mut body) = request(
		context,
		device_id,
		&ByteRangeRequest::Read { path, offset, len },
	)
	.await?;
	let expected = match response {
		ByteRangeResponse::ReadHeader { len } => len,
		other => anyhow::bail!("unexpected response: {other:?}"),
	};
	let mut buf = vec![0u8; expected as usize];
	body.read_exact(&mut buf).await?;
	Ok(Bytes::from(buf))
}

/// Ask a peer for thumbnail tiles of files under its sources. The owner
/// answers each ask once, in the order it finishes them, so the stream yields
/// `(ask index, answer, WebP bytes)` as they land and ends after the last.
pub async fn fetch_tiles(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	tiles: Vec<TileAsk>,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<(usize, TileAnswer, Vec<u8>)>>> {
	let remaining = tiles.len();
	anyhow::ensure!(remaining > 0, "no tiles asked for");
	let (first, body) =
		request(context, device_id, &ByteRangeRequest::FetchTiles { tiles }).await?;
	Ok(futures::stream::try_unfold(
		(Some(first), body, remaining),
		|(first, mut body, remaining)| async move {
			if remaining == 0 {
				return Ok(None);
			}
			let response = match first {
				Some(response) => response,
				None => read_frame(&mut body).await?,
			};
			let ByteRangeResponse::Tile { index, answer } = response else {
				anyhow::bail!("unexpected response: {response:?}");
			};
			let bytes = match &answer {
				TileAnswer::Tile { len, .. } => {
					anyhow::ensure!(*len <= MAX_TILE_LEN, "tile of {len} bytes exceeds limit");
					let mut bytes = vec![0; *len as usize];
					body.read_exact(&mut bytes).await?;
					bytes
				}
				TileAnswer::Current | TileAnswer::Missing => Vec::new(),
			};
			Ok(Some((
				(index as usize, answer, bytes),
				(None, body, remaining - 1),
			)))
		},
	))
}

/// One page of a source's thumbnail sidecars from its owner: the id of the
/// store they come from, and the rows written after `after`.
pub async fn fetch_sidecars(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	source_id: Uuid,
	after: u64,
	limit: u32,
) -> anyhow::Result<(Uuid, Vec<SidecarRow>)> {
	let (response, mut body) = request(
		context,
		device_id,
		&ByteRangeRequest::FetchSidecars {
			source_id,
			after,
			limit,
		},
	)
	.await?;
	let ByteRangeResponse::SidecarPage { store, rows } = response else {
		anyhow::bail!("unexpected response: {response:?}");
	};
	anyhow::ensure!(rows <= limit, "{rows} sidecars for a page of {limit}");
	let mut page = Vec::with_capacity(rows as usize);
	for _ in 0..rows {
		let ByteRangeResponse::Sidecar {
			seq,
			uuid,
			version,
			content_width,
			content_height,
			source_width,
			source_height,
			len,
		} = read_frame(&mut body).await?
		else {
			anyhow::bail!("expected a sidecar");
		};
		anyhow::ensure!(len <= MAX_TILE_LEN, "sidecar of {len} bytes exceeds limit");
		let mut webp = vec![0; len as usize];
		body.read_exact(&mut webp).await?;
		page.push(SidecarRow {
			seq,
			uuid,
			tile: StoredTile {
				version,
				frame: Frame {
					content_width,
					content_height,
					source_width,
					source_height,
				},
				webp,
			},
		});
	}
	Ok((store, page))
}

/// Sources whose sidecars are being copied right now.
static SIDECAR_PULLS: OnceLock<std::sync::Mutex<HashSet<Uuid>>> = OnceLock::new();

/// Start copying a source's thumbnail sidecars from its owner when the owner
/// has written past what this device holds. One copy per source at a time,
/// run beside the replication pass rather than inside it, since the first one
/// can be a gigabyte.
fn copy_sidecars(context: &Arc<CoreContext>, device_id: Uuid, info: &RemoteSourceInfo) {
	if info.sidecar_cursor == 0 {
		return;
	}
	let pulls = SIDECAR_PULLS.get_or_init(|| std::sync::Mutex::new(HashSet::new()));
	if !pulls.lock().unwrap().insert(info.id) {
		return;
	}
	let context = context.clone();
	let info = info.clone();
	tokio::spawn(async move {
		match pull_sidecars(&context, device_id, &info).await {
			Ok(0) => {}
			Ok(copied) => {
				tracing::info!(source = %info.id, copied, "copied thumbnail sidecars from peer")
			}
			Err(err) => tracing::debug!(source = %info.id, %err, "sidecar copy stopped"),
		}
		pulls.lock().unwrap().remove(&info.id);
	});
}

/// Copy a source's sidecars page by page until this device holds everything
/// its owner has written. Returns how many were copied.
async fn pull_sidecars(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	info: &RemoteSourceInfo,
) -> anyhow::Result<usize> {
	let file = sidecar_copy_file(context, device_id, info.id);
	let copy = context
		.thumbs
		.sidecar_store(&file)
		.await
		.ok_or_else(|| anyhow::anyhow!("cannot open {}", file.display()))?;
	let (held, cursor) = copy.mark().await?;
	if held == info.sidecar_store && cursor >= info.sidecar_cursor {
		return Ok(0);
	}
	// A copy of some other store starts over from the beginning of this one.
	let mut after = if held == info.sidecar_store {
		cursor
	} else {
		0
	};
	let mut copied = 0;
	loop {
		let (store, rows) =
			fetch_sidecars(context, device_id, info.id, after, MAX_SIDECAR_PAGE).await?;
		let count = rows.len();
		after = rows.last().map_or(after, |row| row.seq);
		copy.apply(store, rows).await?;
		copied += count;
		if count < MAX_SIDECAR_PAGE as usize {
			return Ok(copied);
		}
	}
}

/// Reconnection storms and the startup sweep can request the same device
/// several times in one moment; one replication per device per window is
/// enough.
static RECENT_SYNCS: OnceLock<std::sync::Mutex<HashMap<Uuid, std::time::Instant>>> =
	OnceLock::new();
const SYNC_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(30);

/// Clear the debounce for a device so a paced refresh is not swallowed by
/// it. The debounce exists to collapse reconnect storms, not to rate-limit
/// the refresh loop, which has its own interval.
pub(super) fn allow_next_sync(device_id: Uuid) {
	let map = RECENT_SYNCS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
	map.lock().unwrap().remove(&device_id);
}

pub(super) fn should_sync(device_id: Uuid) -> bool {
	let map = RECENT_SYNCS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
	let mut map = map.lock().unwrap();
	let now = std::time::Instant::now();
	match map.get(&device_id) {
		Some(last) if now.duration_since(*last) < SYNC_DEBOUNCE => false,
		_ => {
			map.insert(device_id, now);
			true
		}
	}
}

/// One replication per device at a time. The refresh loop clears the
/// debounce before it runs, so a reconnect landing in that window would
/// otherwise fetch and load the same snapshots concurrently; serializing
/// here lets the loser skip on the generation check instead.
static SYNC_LOCKS: OnceLock<std::sync::Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>> =
	OnceLock::new();

fn sync_lock(device_id: Uuid) -> Arc<tokio::sync::Mutex<()>> {
	let map = SYNC_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
	map.lock()
		.unwrap()
		.entry(device_id)
		.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
		.clone()
}

fn now_secs() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0)
}

/// Consecutive failed fetches per source. An artifact the owner cannot
/// currently produce would otherwise be re-requested and re-warned on every
/// refresh pass; the wait doubles per failure up to half an hour, and one
/// success clears it. Listing and metadata refresh are not delayed by this,
/// only the transfer itself.
static FETCH_BACKOFF: OnceLock<std::sync::Mutex<HashMap<Uuid, (u32, std::time::Instant)>>> =
	OnceLock::new();

fn backoff_map() -> &'static std::sync::Mutex<HashMap<Uuid, (u32, std::time::Instant)>> {
	FETCH_BACKOFF.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn fetch_due(source_id: Uuid) -> bool {
	match backoff_map().lock().unwrap().get(&source_id) {
		Some((failures, last)) => {
			let wait = SYNC_DEBOUNCE
				.saturating_mul(1u32 << (*failures).min(6))
				.min(std::time::Duration::from_secs(1800));
			last.elapsed() >= wait
		}
		None => true,
	}
}

fn note_fetch_outcome(source_id: Uuid, ok: bool) {
	let mut map = backoff_map().lock().unwrap();
	if ok {
		map.remove(&source_id);
	} else {
		let entry = map
			.entry(source_id)
			.or_insert((0, std::time::Instant::now()));
		entry.0 = entry.0.saturating_add(1);
		entry.1 = std::time::Instant::now();
	}
}

/// How long a replica may lag behind an owner that keeps writing. A dirty
/// listing over an unchanged artifact means the owner's arena moved past its
/// last save; fetching on every such listing turns one busy machine into a
/// full artifact transfer per refresh interval, which a development home
/// directory sustains indefinitely. A moved generation still transfers
/// immediately — this pace only bounds what dirtiness alone can cost.
const DIRTY_REFRESH_SECS: u64 = 300;

/// Whether a listing justifies transferring the artifact.
fn transfer_due(existing: &RemoteShare, info: &RemoteSourceInfo, now_secs: u64) -> bool {
	if info.generation == 0 || existing.generation != info.generation {
		return true;
	}
	info.dirty && now_secs >= existing.synced_at_secs.saturating_add(DIRTY_REFRESH_SECS)
}

/// An unchanged artifact still rides with fresh facts: the owner's listing
/// carries attachment, counts and its display name on every pass, and
/// freezing them alongside the generation would leave a detached drive
/// reading as attached until its next content change.
fn refresh_share_facts(
	existing: &Arc<RemoteShare>,
	info: &RemoteSourceInfo,
	device_label: &str,
) -> Option<Arc<RemoteShare>> {
	if existing.info == *info && existing.device_label == device_label {
		return None;
	}
	Some(Arc::new(RemoteShare {
		device_id: existing.device_id,
		device_label: device_label.to_string(),
		info: info.clone(),
		index: existing.index.clone(),
		synced_at_secs: existing.synced_at_secs,
		generation: existing.generation,
	}))
}

/// A transfer in progress on disk, named
/// `<source>.<generation>.<blake3 prefix>.part`. The name carries what the
/// next header must match for the bytes to be worth keeping, so a restart
/// decides without the owner and without a sidecar file.
#[derive(Debug, Clone, PartialEq)]
struct PartFile {
	path: PathBuf,
	generation: u64,
	prefix: [u8; 8],
	have: u64,
}

fn part_path(replica_dir: &Path, source_id: Uuid, generation: u64, checksum: &[u8; 32]) -> PathBuf {
	replica_dir.join(format!(
		"{}.{generation}.{}.part",
		source_id.simple(),
		hex::encode(&checksum[..8])
	))
}

/// The source, generation and checksum prefix a `.part` file name encodes.
fn parse_part_name(name: &str) -> Option<(Uuid, u64, [u8; 8])> {
	let rest = name.strip_suffix(".part")?;
	let (source, rest) = rest.split_once('.')?;
	let (generation, prefix) = rest.split_once('.')?;
	let source_id = Uuid::try_parse(source).ok()?;
	let generation = generation.parse().ok()?;
	let prefix: [u8; 8] = hex::decode(prefix).ok()?.try_into().ok()?;
	Some((source_id, generation, prefix))
}

/// The partial transfer on disk for `source_id`, if any. Two attempts can
/// only leave two parts when the owner's generation moved between them, so
/// everything but the most recently written one is removed.
async fn find_part(replica_dir: &Path, source_id: Uuid) -> Option<PartFile> {
	let mut dir = tokio::fs::read_dir(replica_dir).await.ok()?;
	let mut found: Option<(PartFile, SystemTime)> = None;
	while let Ok(Some(entry)) = dir.next_entry().await {
		let name = entry.file_name();
		let Some((id, generation, prefix)) = name.to_str().and_then(parse_part_name) else {
			continue;
		};
		if id != source_id {
			continue;
		}
		let Ok(meta) = entry.metadata().await else {
			continue;
		};
		let part = PartFile {
			path: entry.path(),
			generation,
			prefix,
			have: meta.len(),
		};
		let modified = meta.modified().unwrap_or(UNIX_EPOCH);
		match &found {
			Some((_, at)) if *at >= modified => {
				let _ = tokio::fs::remove_file(&part.path).await;
			}
			Some((older, _)) => {
				let _ = tokio::fs::remove_file(&older.path).await;
				found = Some((part, modified));
			}
			None => found = Some((part, modified)),
		}
	}
	found.map(|(part, _)| part)
}

/// Drop partial transfers the owner's listing no longer describes: a part
/// whose source is gone, or whose generation is not the one the owner holds
/// now, can never complete against the bytes the owner would serve. A part
/// that still matches stays, however long the owner has been away.
async fn prune_parts(replica_dir: &Path, sources: &[RemoteSourceInfo]) {
	let Ok(mut dir) = tokio::fs::read_dir(replica_dir).await else {
		return;
	};
	while let Ok(Some(entry)) = dir.next_entry().await {
		let name = entry.file_name();
		let Some((source_id, generation, _)) = name.to_str().and_then(parse_part_name) else {
			continue;
		};
		let current = sources
			.iter()
			.find(|info| info.id == source_id)
			.is_some_and(|info| info.generation == generation);
		if !current {
			tracing::debug!(
				source = %source_id,
				generation,
				"partial transfer no longer matches the owner's generation; removed"
			);
			let _ = tokio::fs::remove_file(entry.path()).await;
		}
	}
}

/// What an owner answers an artifact request with: the whole artifact's
/// identity, whatever offset the bytes behind it start at.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ArtifactHeader {
	len: u64,
	generation: u64,
	checksum: [u8; 32],
}

impl ArtifactHeader {
	/// Whether a partial file was cut from these same bytes. An owner that
	/// predates header identity names nothing, so nothing of its resumes.
	fn continues(&self, part: &PartFile) -> bool {
		self.generation != 0
			&& self.checksum != [0u8; 32]
			&& part.generation == self.generation
			&& part.prefix[..] == self.checksum[..8]
			&& part.have <= self.len
	}
}

/// Why a fetch did not publish. A request the owner refused or could not
/// open moved no bytes, so the caller may try another artifact kind; a
/// transfer that failed partway has its partial file on disk and should
/// simply be retried.
#[derive(Debug)]
enum FetchError {
	Unavailable(anyhow::Error),
	Failed(anyhow::Error),
}

impl std::fmt::Display for FetchError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FetchError::Unavailable(err) | FetchError::Failed(err) => write!(f, "{err}"),
		}
	}
}

type ArtifactBody = Box<dyn tokio::io::AsyncRead + Send + Unpin>;

/// Bring one artifact's bytes onto disk, continuing a `.part` from an
/// earlier attempt when the owner still serves the same bytes.
///
/// `open(offset)` asks the owner for the artifact's tail from `offset`; the
/// header it returns describes the whole artifact. A part the header does
/// not continue is discarded and the fetch starts over from zero, as does a
/// resumed request the owner cannot open, since an owner too old to know the
/// offset variants still serves a whole artifact. Every other failure leaves
/// the partial file in place for the next attempt. On success the file holds
/// every byte the header declared and matches its checksum.
async fn transfer_artifact<F, Fut>(
	replica_dir: &Path,
	source_id: Uuid,
	mut open: F,
) -> Result<(PathBuf, ArtifactHeader), FetchError>
where
	F: FnMut(u64) -> Fut,
	Fut: std::future::Future<Output = anyhow::Result<(ArtifactHeader, ArtifactBody)>>,
{
	let mut part = find_part(replica_dir, source_id).await;
	let mut opened = match &part {
		Some(existing) => match open(existing.have).await {
			Ok(opened) => Some(opened),
			Err(err) => {
				tracing::debug!(source = %source_id, %err, "resumed request refused; starting over");
				None
			}
		},
		None => None,
	};
	if let Some(existing) = part.take() {
		match &opened {
			Some((header, _)) if header.continues(&existing) => part = Some(existing),
			_ => {
				tracing::info!(
					source = %source_id,
					had = existing.have,
					"partial transfer does not match what the owner serves now; starting over"
				);
				let _ = tokio::fs::remove_file(&existing.path).await;
				opened = None;
			}
		}
	}
	let (header, mut body) = match opened {
		Some(opened) => opened,
		None => open(0).await.map_err(FetchError::Unavailable)?,
	};
	if header.len > MAX_SNAPSHOT_LEN {
		return Err(FetchError::Unavailable(anyhow::anyhow!(
			"declared artifact of {} bytes exceeds the transfer bound",
			header.len
		)));
	}

	let (path, offset) = match part {
		Some(part) => (part.path, part.have),
		None => (
			part_path(replica_dir, source_id, header.generation, &header.checksum),
			0,
		),
	};
	receive_artifact(&mut body, &path, offset, header.len, header.checksum)
		.await
		.map_err(FetchError::Failed)?;
	Ok((path, header))
}

/// Append a stream to a partial artifact from `offset` to `len`, hashing the
/// bytes already on disk and the new ones alike. A short stream fails the
/// transfer with the file in place for the next attempt; a checksum
/// mismatch removes it, since no continuation can mend it.
async fn receive_artifact<R: tokio::io::AsyncRead + Unpin + ?Sized>(
	body: &mut R,
	part_path: &Path,
	offset: u64,
	len: u64,
	expected_checksum: [u8; 32],
) -> anyhow::Result<()> {
	use tokio::io::AsyncWriteExt;

	let mut hasher = blake3::Hasher::new();
	let mut file = if offset > 0 {
		let mut file = tokio::fs::OpenOptions::new()
			.read(true)
			.append(true)
			.open(part_path)
			.await?;
		let mut hashed = 0u64;
		let mut buf = vec![0u8; 256 * 1024];
		while hashed < offset {
			let want = (offset - hashed).min(buf.len() as u64) as usize;
			let n = file.read(&mut buf[..want]).await?;
			if n == 0 {
				anyhow::bail!("partial artifact is {hashed} bytes, not the {offset} expected");
			}
			hasher.update(&buf[..n]);
			hashed += n as u64;
		}
		file
	} else {
		tokio::fs::File::create(part_path).await?
	};

	let mut remaining = len - offset;
	let mut buf = vec![0u8; 256 * 1024];
	let streamed = async {
		while remaining > 0 {
			let want = remaining.min(buf.len() as u64) as usize;
			let n = body.read(&mut buf[..want]).await?;
			if n == 0 {
				anyhow::bail!(
					"stream ended {remaining} bytes short of the {len} the header declared"
				);
			}
			super::replication::throttle().acquire(n as u64).await;
			hasher.update(&buf[..n]);
			file.write_all(&buf[..n]).await?;
			remaining -= n as u64;
		}
		anyhow::Ok(())
	}
	.await;
	// Whatever landed is kept for the next attempt, so it must reach the
	// file before the handle goes: a dropped tokio file loses pending writes.
	let flushed = file.flush().await;
	streamed?;
	flushed?;
	file.sync_all().await?;

	// A zeroed checksum is an owner that predates the field; the parse
	// validation behind this still stands between the bytes and publication.
	if expected_checksum != [0u8; 32] {
		let received = *hasher.finalize().as_bytes();
		if received != expected_checksum {
			let _ = tokio::fs::remove_file(part_path).await;
			anyhow::bail!("delivered bytes do not match the checksum the header declared");
		}
	}
	Ok(())
}

/// Validate a delivered artifact and publish it as the live share.
///
/// The parse is the gate: the temporary file must load as a snapshot before
/// it may replace an artifact that already does, so a corrupt delivery costs
/// only its transfer. `generation` names the bytes the owner actually
/// served, from the response header; the listing that prompted the fetch is
/// routinely older, because the owner saves live state before serving.
async fn validate_and_publish(
	replica_dir: &std::path::Path,
	device_id: Uuid,
	device_label: &str,
	info: &RemoteSourceInfo,
	generation: u64,
	tmp_path: &std::path::Path,
) -> anyhow::Result<()> {
	let loaded = {
		let path = tmp_path.to_path_buf();
		tokio::task::spawn_blocking(move || Arena::load_snapshot(&path)).await?
	};
	let index = match loaded {
		Ok(Some((index, _meta))) => index,
		Ok(None) | Err(_) => {
			let _ = tokio::fs::remove_file(tmp_path).await;
			anyhow::bail!(
				"delivered artifact does not parse as a snapshot; keeping the previous one"
			);
		}
	};

	let snapshot_path = replica_dir.join(format!("{}.snapshot", info.id.simple()));
	tokio::fs::rename(tmp_path, &snapshot_path).await?;

	let share = Arc::new(RemoteShare {
		device_id,
		device_label: device_label.to_string(),
		generation,
		info: info.clone(),
		index: Arc::new(TokioRwLock::new(index)),
		synced_at_secs: now_secs(),
	});
	shares_map().write().await.insert(info.id, share);
	Ok(())
}

/// Build a replica arena from a delivered database: every filesystem row
/// becomes an entry at the share root, ancestors synthesized by the arena
/// itself, content kinds derived from extensions the way a fresh walk
/// derives them. This is what makes a database artifact browsable and
/// searchable through the same paths an arena snapshot is.
async fn arena_from_database(
	db: &sd_store::SourceDb,
	share_root: &std::path::Path,
) -> anyhow::Result<Arena> {
	use crate::ops::indexing::metadata::EntryMetadata;
	use crate::ops::indexing::state::EntryKind;

	let mut index = Arena::new()?;
	let mut after_rowid = 0i64;
	loop {
		let (entries, last) = sd_store::read::all_entries_page(db.pool(), after_rowid, 2_000)
			.await
			.map_err(|e| anyhow::anyhow!("database page failed: {e}"))?;
		let done = entries.len() < 2_000;
		after_rowid = last;

		for entry in entries {
			let path = share_root.join(&entry.relative_path);
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
		}

		if done {
			break;
		}
	}
	Ok(index)
}

/// Fetch one source's database into the device's replica directory, rebuild
/// its arena from it, and publish the share. The artifact carries exactly
/// the source's own records — a nested source no longer ships its volume
/// siblings' metadata inside an arena snapshot.
async fn fetch_database_and_publish(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	device_label: &str,
	replica_dir: &Path,
	info: &RemoteSourceInfo,
) -> Result<(), FetchError> {
	let started = std::time::Instant::now();
	let source_id = info.id;
	let (part, header) = transfer_artifact(replica_dir, source_id, |offset| async move {
		let req = if offset == 0 {
			ByteRangeRequest::FetchDatabase { source_id }
		} else {
			ByteRangeRequest::FetchDatabaseFrom { source_id, offset }
		};
		let (response, body) = request(context, device_id, &req).await?;
		match response {
			ByteRangeResponse::DatabaseHeader {
				len,
				generation,
				checksum,
			} => Ok((
				ArtifactHeader {
					len,
					generation,
					checksum,
				},
				body,
			)),
			other => anyhow::bail!("unexpected response: {other:?}"),
		}
	})
	.await?;

	// The parse-and-rebuild is the gate: the complete database must open
	// read-only and yield an arena before it may replace anything.
	let db = match sd_store::SourceManager::open_file_read_only(&part).await {
		Ok(db) => db,
		Err(err) => {
			let _ = tokio::fs::remove_file(&part).await;
			return Err(FetchError::Failed(anyhow::anyhow!(
				"delivered database refused to open: {err}"
			)));
		}
	};
	let index = match arena_from_database(&db, &info.root).await {
		Ok(index) => index,
		Err(err) => {
			db.pool().close().await;
			let _ = tokio::fs::remove_file(&part).await;
			return Err(FetchError::Failed(err));
		}
	};
	// The pool must release its handle before the file moves.
	db.pool().close().await;

	let db_path = replica_dir.join(format!("{}.db", info.id.simple()));
	tokio::fs::rename(&part, &db_path)
		.await
		.map_err(|err| FetchError::Failed(err.into()))?;
	// A database replica supersedes any arena-snapshot artifact the source
	// had before it was delivered this way.
	let _ =
		tokio::fs::remove_file(replica_dir.join(format!("{}.snapshot", info.id.simple()))).await;

	let share = Arc::new(RemoteShare {
		device_id,
		device_label: device_label.to_string(),
		generation: header.generation,
		info: info.clone(),
		index: Arc::new(TokioRwLock::new(index)),
		synced_at_secs: now_secs(),
	});
	shares_map().write().await.insert(info.id, share);
	tracing::info!(
		source = %info.id,
		generation = header.generation,
		bytes = header.len,
		elapsed_ms = started.elapsed().as_millis() as u64,
		"replica database received and published"
	);
	Ok(())
}

/// Fetch one source's snapshot into the device's replica directory and
/// publish it. A failed transfer leaves its partial file for the next
/// attempt and the previously published artifact and share untouched.
async fn fetch_and_publish(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	device_label: &str,
	replica_dir: &Path,
	info: &RemoteSourceInfo,
) -> Result<(), FetchError> {
	let started = std::time::Instant::now();
	let source_id = info.id;
	let (part, header) = transfer_artifact(replica_dir, source_id, |offset| async move {
		let req = if offset == 0 {
			ByteRangeRequest::FetchSnapshot { source_id }
		} else {
			ByteRangeRequest::FetchSnapshotFrom { source_id, offset }
		};
		let (response, body) = request(context, device_id, &req).await?;
		match response {
			ByteRangeResponse::SnapshotHeader {
				len,
				generation,
				checksum,
			} => Ok((
				ArtifactHeader {
					len,
					generation,
					checksum,
				},
				body,
			)),
			other => anyhow::bail!("unexpected response: {other:?}"),
		}
	})
	.await?;

	// An owner that predates header identity sends zero; the listing's
	// generation is then the only name these bytes have.
	let generation = if header.generation != 0 {
		header.generation
	} else {
		info.generation
	};
	validate_and_publish(
		replica_dir,
		device_id,
		device_label,
		info,
		generation,
		&part,
	)
	.await
	.map_err(FetchError::Failed)?;
	tracing::info!(
		source = %info.id,
		generation,
		bytes = header.len,
		elapsed_ms = started.elapsed().as_millis() as u64,
		"replica artifact received and published"
	);
	Ok(())
}

/// Pull a connected device's source list and replicate every snapshot it can
/// provide. Returns how many sources are now served for that device.
pub async fn sync_device(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	device_label: String,
) -> anyhow::Result<usize> {
	let lock = sync_lock(device_id);
	let _guard = lock.lock().await;
	let (response, _) = request(context, device_id, &ByteRangeRequest::ListSources).await?;
	let sources = match response {
		ByteRangeResponse::Sources(s) => s,
		other => anyhow::bail!("unexpected response: {other:?}"),
	};

	// The owner's own accounting rides along with every sync, so fleet
	// totals stay as fresh as the replicas. Fetched before the snapshot
	// loop because one failing source aborts it, and the device's totals
	// are still true when a snapshot is not. A peer running an older build
	// answers with an error; totals then just omit that device.
	match request(context, device_id, &ByteRangeRequest::DeviceSummary).await {
		Ok((ByteRangeResponse::DeviceSummary(summary), _)) => {
			summaries_map().write().await.insert(device_id, summary);
		}
		Ok((other, _)) => {
			tracing::debug!("device summary from {device_label}: unexpected {other:?}");
		}
		Err(err) => {
			tracing::debug!("device summary from {device_label} failed: {err}");
		}
	}

	// What the owner publishes about itself refreshes on the same pass. An
	// owner that does not answer leaves whatever an earlier pass recorded.
	let facts = match request(context, device_id, &ByteRangeRequest::DeviceFacts).await {
		Ok((ByteRangeResponse::DeviceFacts(facts), _)) => Some(ObservedFacts {
			facts,
			observed_at_secs: now_secs(),
		}),
		Ok((other, _)) => {
			tracing::debug!("device facts from {device_label}: unexpected {other:?}");
			None
		}
		Err(err) => {
			tracing::debug!("device facts from {device_label} failed: {err}");
			None
		}
	};

	let replica_dir = replica_dir(context, device_id);
	tokio::fs::create_dir_all(&replica_dir).await?;
	prune_parts(&replica_dir, &sources).await;

	let mut synced = 0usize;
	for info in &sources {
		// A replica built from the same snapshot the owner still holds is
		// current. A moved generation always transfers; dirtiness alone is
		// paced, since a machine that keeps writing would otherwise cost a
		// full artifact per refresh interval.
		if let Some(existing) = remote_share(info.id).await {
			if !transfer_due(&existing, info, now_secs()) {
				if let Some(refreshed) = refresh_share_facts(&existing, info, &device_label) {
					shares_map().write().await.insert(info.id, refreshed);
				}
				tracing::debug!(source = %info.id, generation = info.generation, "replica current; no transfer");
				synced += 1;
				continue;
			}
		}
		if !fetch_due(info.id) {
			continue;
		}

		let fetched = if info.nested {
			// A nested source travels as its own database, which carries
			// exactly its records. An owner too old to export one still
			// answers on the snapshot path; a transfer that broke partway
			// is retried as a database, since its partial file is waiting.
			match fetch_database_and_publish(context, device_id, &device_label, &replica_dir, info)
				.await
			{
				Ok(()) => Ok(()),
				Err(FetchError::Unavailable(err)) => {
					tracing::debug!(
						source = %info.id,
						%err,
						"database fetch unavailable; falling back to the arena snapshot"
					);
					fetch_and_publish(context, device_id, &device_label, &replica_dir, info).await
				}
				Err(err) => Err(err),
			}
		} else {
			fetch_and_publish(context, device_id, &device_label, &replica_dir, info).await
		};
		match fetched {
			Ok(()) => {
				note_fetch_outcome(info.id, true);
				synced += 1;
			}
			Err(err) => {
				note_fetch_outcome(info.id, false);
				tracing::warn!(
					source = %info.id,
					%err,
					"artifact fetch failed; the previous replica, if any, stays published"
				);
			}
		}
	}

	for info in &sources {
		copy_sidecars(context, device_id, info);
	}

	// Persist the inventory beside its artifacts. A source whose fetch
	// failed this round keeps its previous entry while an older artifact
	// still exists, so one failed transfer does not erase a replica from
	// the known set; a source the owner's listing no longer carries is
	// dropped, since a successful enumeration is removal evidence.
	let previous = known_map().read().await.get(&device_id).cloned();
	let mut entries = Vec::new();
	for info in &sources {
		if let Some(share) = remote_share(info.id).await {
			entries.push(ReplicaEntry {
				info: share.info.clone(),
				generation: share.generation,
				synced_at_secs: share.synced_at_secs,
			});
		} else if let Some(prior) = previous
			.as_ref()
			.and_then(|manifest| manifest.sources.iter().find(|e| e.info.id == info.id))
		{
			if replica_dir
				.join(format!("{}.snapshot", info.id.simple()))
				.exists()
			{
				entries.push(prior.clone());
			}
		}
	}
	write_manifest(
		&replica_dir,
		&ReplicaManifest {
			device_id,
			device_label: device_label.clone(),
			sources: entries,
			facts: facts.or_else(|| previous.and_then(|manifest| manifest.facts)),
		},
	)
	.await;

	tracing::info!("peer mounts: {synced} source(s) replicated from {device_label} ({device_id})");
	Ok(synced)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::indexing::metadata::EntryMetadata;
	use crate::ops::indexing::state::EntryKind;

	fn info(id: Uuid, root: &str, count: u64) -> RemoteSourceInfo {
		RemoteSourceInfo {
			id,
			root: PathBuf::from(root),
			volume_uuid: None,
			attached: true,
			entry_count: Some(count),
			total_bytes: Some(count * 10),
			generation: 7,
			dirty: false,
			nested: false,
			sidecar_store: Uuid::nil(),
			sidecar_cursor: 0,
		}
	}

	/// A restart with every owner offline restores the inventory from disk:
	/// artifacts that load become live shares, and a source whose artifact is
	/// gone stays known and listed as unavailable instead of vanishing.
	#[tokio::test]
	async fn a_cold_restore_rebuilds_the_inventory_without_the_owner() {
		let base = tempfile::tempdir().expect("dir");
		let device_id = Uuid::now_v7();
		let replica_dir = base.path().join(device_id.simple().to_string());
		std::fs::create_dir_all(&replica_dir).expect("replica dir");

		// One source with a real artifact on disk.
		let cached = Uuid::now_v7();
		let mut index = Arena::new().expect("index");
		let file = PathBuf::from("/mnt/pool/kept/file.txt");
		index
			.add_entry(
				file.clone(),
				Uuid::now_v7(),
				EntryMetadata {
					path: file,
					kind: EntryKind::File,
					size: 4,
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
			.expect("entry");
		index
			.save_snapshot(
				&replica_dir.join(format!("{}.snapshot", cached.simple())),
				cached,
				&PathBuf::from("/mnt/pool/kept"),
			)
			.expect("artifact");

		// One source whose artifact never made it here.
		let missing = Uuid::now_v7();

		let manifest = ReplicaManifest {
			device_id,
			device_label: "away-drive".to_string(),
			sources: vec![
				ReplicaEntry {
					info: info(cached, "/mnt/pool/kept", 2),
					generation: 7,
					synced_at_secs: 1,
				},
				ReplicaEntry {
					info: info(missing, "/mnt/pool/lost", 9),
					generation: 3,
					synced_at_secs: 1,
				},
			],
			facts: None,
		};
		std::fs::write(
			manifest_path(&replica_dir),
			serde_json::to_vec(&manifest).expect("serialize"),
		)
		.expect("manifest");

		let (loaded, known) = restore_from(base.path()).await;
		assert_eq!((loaded, known), (1, 2));

		let share = remote_share(cached).await.expect("cached share restored");
		assert_eq!(share.device_label, "away-drive");
		assert_eq!(share.generation, 7);
		assert_eq!(
			share.index.read().await.find_by_name("file.txt").len(),
			1,
			"the restored arena answers by content, not just by existing"
		);

		assert!(remote_share(missing).await.is_none());
		let unloaded = known_unloaded().await;
		assert!(
			unloaded.iter().any(|(device, label, entry)| {
				*device == device_id && label == "away-drive" && entry.info.id == missing
			}),
			"the missing artifact's source stays known and unavailable"
		);
	}

	fn snapshot_bytes_for(source_id: Uuid, root: &str, at: &std::path::Path) {
		let mut index = Arena::new().expect("index");
		let file = PathBuf::from(root).join("file.txt");
		index
			.add_entry(
				file.clone(),
				Uuid::now_v7(),
				EntryMetadata {
					path: file,
					kind: EntryKind::File,
					size: 4,
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
			.expect("entry");
		index
			.save_snapshot(at, source_id, &PathBuf::from(root))
			.expect("artifact");
	}

	/// The owner saves live state before serving, so the delivered artifact
	/// is routinely newer than the listing that prompted the fetch. What gets
	/// recorded is the header's generation for the bytes received, never the
	/// listing's.
	#[tokio::test]
	async fn the_generation_recorded_names_the_delivered_bytes() {
		let base = tempfile::tempdir().expect("dir");
		let source_id = Uuid::now_v7();
		let tmp = base.path().join("incoming.tmp");
		snapshot_bytes_for(source_id, "/mnt/pool/kept", &tmp);

		let listing = info(source_id, "/mnt/pool/kept", 2);
		assert_eq!(
			listing.generation, 7,
			"the listing advertises an older generation"
		);
		validate_and_publish(base.path(), Uuid::now_v7(), "owner", &listing, 42, &tmp)
			.await
			.expect("publish");

		let share = remote_share(source_id).await.expect("share");
		assert_eq!(
			share.generation, 42,
			"the delivered generation is recorded, not the listing's"
		);
		assert!(base
			.path()
			.join(format!("{}.snapshot", source_id.simple()))
			.exists());
	}

	/// A delivery that does not parse as a snapshot is discarded with its
	/// temporary file; the previously published artifact and share survive.
	#[tokio::test]
	async fn a_corrupt_delivery_never_replaces_a_good_artifact() {
		let base = tempfile::tempdir().expect("dir");
		let source_id = Uuid::now_v7();
		let device_id = Uuid::now_v7();

		let good_tmp = base.path().join("good.tmp");
		snapshot_bytes_for(source_id, "/mnt/pool/kept", &good_tmp);
		let listing = info(source_id, "/mnt/pool/kept", 2);
		validate_and_publish(base.path(), device_id, "owner", &listing, 7, &good_tmp)
			.await
			.expect("publish the good artifact");

		let bad_tmp = base.path().join("bad.tmp");
		std::fs::write(&bad_tmp, b"not a snapshot").expect("junk");
		let refused =
			validate_and_publish(base.path(), device_id, "owner", &listing, 9, &bad_tmp).await;
		assert!(refused.is_err(), "junk must not publish");
		assert!(!bad_tmp.exists(), "the failed temporary is cleaned up");

		let share = remote_share(source_id).await.expect("share survives");
		assert_eq!(share.generation, 7, "the good generation stays published");
		let artifact = base.path().join(format!("{}.snapshot", source_id.simple()));
		let reloaded = Arena::load_snapshot(&artifact).expect("readable");
		assert!(reloaded.is_some(), "the good artifact is untouched");
	}

	/// A delivered database restores into a replica exactly as an arena
	/// snapshot does: the tree browses, names answer search, and record
	/// identities survive the trip. The artifact kind is a transport detail.
	#[tokio::test]
	async fn a_database_artifact_restores_into_a_browsable_replica() {
		use sd_store::file::{FileKind, FileWrite, Ledger, Observation};

		let store_dir = tempfile::tempdir().expect("store dir");
		let manager = sd_store::SourceManager::new(store_dir.path().to_path_buf());
		let source_id = Uuid::now_v7();
		let id_str = source_id.simple().to_string();
		manager
			.create(&id_str, &sd_store::filesystem_schema())
			.await
			.expect("create");
		let db = manager.open(&id_str).await.expect("open");
		db.begin_sync().await.expect("epoch");
		let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
		let mut writes = Vec::new();
		for (path, kind) in [
			("tools", FileKind::Directory),
			("tools/zig.bin", FileKind::File),
			("readme.txt", FileKind::File),
		] {
			let observation = Observation {
				external_id: path.to_string(),
				kind,
				name: path.rsplit('/').next().unwrap_or(path).to_string(),
				size: 7,
				mtime: 1_700_000_000_000,
				created: None,
				accessed: None,
				inode: None,
				mode: Some(0o644),
				uid: None,
				gid: None,
				link_target: None,
				extension: path.rsplit_once('.').map(|(_, e)| e.to_string()),
				is_hidden: false,
				identity: None,
			};
			let resolution = ledger.resolve(&observation);
			let parent_uuid = path
				.rsplit_once('/')
				.and_then(|(parent, _)| ledger.uuid_of(parent));
			writes.push(FileWrite {
				resolution,
				parent_uuid,
				observation,
			});
		}
		db.apply_files(&writes, &[], &[], None)
			.await
			.expect("apply");
		let stored_uuid = db
			.resolve_path("tools/zig.bin")
			.await
			.expect("resolve")
			.expect("row exists");

		// Deliver the way the owner does. VACUUM INTO writes one consistent
		// file; copying data.db would miss whatever the WAL still holds.
		let base = tempfile::tempdir().expect("base");
		let device_id = Uuid::now_v7();
		let replica_dir = base.path().join(device_id.simple().to_string());
		std::fs::create_dir_all(&replica_dir).expect("replica dir");
		sqlx::query("VACUUM INTO ?")
			.bind(
				replica_dir
					.join(format!("{}.db", source_id.simple()))
					.to_string_lossy()
					.into_owned(),
			)
			.execute(db.pool())
			.await
			.expect("artifact");
		db.pool().close().await;

		let root = PathBuf::from("/mnt/pool/kept");
		let mut source_info = info(source_id, "/mnt/pool/kept", 3);
		source_info.nested = true;
		let manifest = ReplicaManifest {
			device_id,
			device_label: "owner".to_string(),
			sources: vec![ReplicaEntry {
				info: source_info,
				generation: 11,
				synced_at_secs: 1,
			}],
			facts: None,
		};
		std::fs::write(
			manifest_path(&replica_dir),
			serde_json::to_vec(&manifest).expect("serialize"),
		)
		.expect("manifest");

		let (loaded, known) = restore_from(base.path()).await;
		assert_eq!((loaded, known), (1, 1));

		let share = remote_share(source_id).await.expect("share restored");
		assert_eq!(share.generation, 11);
		let index = share.index.read().await;
		let children = index
			.list_directory(&root.join("tools"))
			.expect("directory covered");
		assert_eq!(children, vec![root.join("tools/zig.bin")]);
		assert_eq!(index.find_by_name("zig.bin").len(), 1);
		assert_eq!(
			index.get_entry_uuid(&root.join("tools/zig.bin")),
			Some(stored_uuid),
			"record identity survives the database trip"
		);
	}

	/// A moved generation always transfers. Dirtiness over the same artifact
	/// paces at the dirty-refresh interval instead of costing one full
	/// transfer per refresh pass, and a clean matching listing costs nothing.
	#[test]
	fn dirtiness_alone_paces_while_a_moved_generation_transfers() {
		let source_id = Uuid::now_v7();
		let share = RemoteShare {
			device_id: Uuid::now_v7(),
			device_label: "owner".to_string(),
			info: info(source_id, "/mnt/pool/kept", 2),
			index: Arc::new(TokioRwLock::new(Arena::new().expect("index"))),
			synced_at_secs: 1_000,
			generation: 7,
		};

		let mut listing = info(source_id, "/mnt/pool/kept", 2);
		assert!(
			!transfer_due(&share, &listing, 1_030),
			"clean and matching transfers nothing"
		);

		listing.dirty = true;
		assert!(
			!transfer_due(&share, &listing, 1_030),
			"dirtiness inside the pace waits"
		);
		assert!(
			transfer_due(&share, &listing, 1_000 + DIRTY_REFRESH_SECS),
			"dirtiness past the pace refreshes"
		);

		listing.dirty = false;
		listing.generation = 8;
		assert!(
			transfer_due(&share, &listing, 1_030),
			"a moved generation transfers immediately"
		);

		listing.generation = 0;
		assert!(
			transfer_due(&share, &listing, 1_030),
			"an owner with no snapshot yet always transfers"
		);
	}

	/// An unchanged generation skips the transfer without freezing the
	/// owner's facts: attachment, counts and the display name still refresh,
	/// while the arena, generation and sync time stay what the artifact is.
	#[tokio::test]
	async fn an_unchanged_generation_still_refreshes_the_owner_facts() {
		let source_id = Uuid::now_v7();
		let existing = Arc::new(RemoteShare {
			device_id: Uuid::now_v7(),
			device_label: "old-name".to_string(),
			info: info(source_id, "/mnt/pool/kept", 2),
			index: Arc::new(TokioRwLock::new(Arena::new().expect("index"))),
			synced_at_secs: 5,
			generation: 7,
		});

		let mut fresh = info(source_id, "/mnt/pool/kept", 3);
		fresh.attached = false;

		let refreshed = refresh_share_facts(&existing, &fresh, "new-name").expect("facts changed");
		assert!(!refreshed.info.attached, "attachment observation refreshed");
		assert_eq!(refreshed.info.entry_count, Some(3));
		assert_eq!(refreshed.device_label, "new-name");
		assert_eq!(
			refreshed.generation, 7,
			"the artifact's generation is untouched"
		);
		assert_eq!(
			refreshed.synced_at_secs, 5,
			"sync time still names the artifact"
		);
		assert!(
			Arc::ptr_eq(&refreshed.index, &existing.index),
			"the loaded arena is shared, not rebuilt"
		);

		assert!(
			refresh_share_facts(&refreshed, &fresh, "new-name").is_none(),
			"identical facts rebuild nothing"
		);
	}

	/// What an owner published persists with its manifest. After a restart
	/// with the owner offline its volumes still list under it, unmounted and
	/// stamped with when they were observed, and mount again on reconnect.
	#[tokio::test]
	async fn published_facts_outlive_the_owners_connection() {
		use crate::service::network::protocol::byterange::{RemoteHardware, RemoteVolumeInfo};

		let base = tempfile::tempdir().expect("dir");
		let device_id = Uuid::now_v7();
		let replica_dir = base.path().join(device_id.simple().to_string());
		std::fs::create_dir_all(&replica_dir).expect("replica dir");

		let volume_id = Uuid::now_v7();
		let observed = ObservedFacts {
			facts: RemoteDeviceFacts {
				hardware: RemoteHardware {
					cpu_model: Some("AMD EPYC 4464P".to_string()),
					..Default::default()
				},
				volumes: vec![RemoteVolumeInfo {
					id: volume_id,
					fingerprint: "pool-fingerprint".to_string(),
					name: "pool".to_string(),
					display_name: Some("jamie-nas".to_string()),
					mount_point: PathBuf::from("/mnt/pool/jamie-nas"),
					volume_type: "External".to_string(),
					disk_type: "HDD".to_string(),
					file_system: "ZFS".to_string(),
					total_capacity: 40 << 40,
					available_space: 4 << 40,
					is_read_only: false,
					is_tracked: true,
					read_speed_mbps: None,
					write_speed_mbps: None,
				}],
			},
			observed_at_secs: 1_789_700_000,
		};
		let manifest = ReplicaManifest {
			device_id,
			device_label: "titan".to_string(),
			sources: Vec::new(),
			facts: Some(observed.clone()),
		};
		std::fs::write(
			manifest_path(&replica_dir),
			serde_json::to_vec(&manifest).expect("serialize"),
		)
		.expect("manifest");

		restore_from(base.path()).await;
		let published = published_facts().await;
		assert_eq!(published.get(&device_id), Some(&observed));

		let offline = volumes_from(&published, &std::collections::HashSet::new());
		let pool = offline
			.iter()
			.find(|volume| volume.id == volume_id)
			.expect("the volume lists while its owner is away");
		assert_eq!(pool.device_id, device_id);
		assert!(!pool.is_mounted);
		assert_eq!(pool.last_seen_at.timestamp(), 1_789_700_000);

		let online = volumes_from(&published, &std::collections::HashSet::from([device_id]));
		assert!(
			online
				.iter()
				.any(|volume| volume.id == volume_id && volume.is_mounted),
			"a connected owner's volume reads as mounted"
		);
	}

	/// An owner that answers `transfer_artifact` from a byte buffer, cutting
	/// the stream after `cut` bytes of each answer when asked to, the way a
	/// killed daemon or a dropped link does.
	struct ScriptedOwner {
		bytes: Vec<u8>,
		generation: u64,
		cuts: std::collections::VecDeque<Option<usize>>,
		offsets: Vec<u64>,
	}

	impl ScriptedOwner {
		fn header(&self) -> ArtifactHeader {
			ArtifactHeader {
				len: self.bytes.len() as u64,
				generation: self.generation,
				checksum: *blake3::hash(&self.bytes).as_bytes(),
			}
		}

		fn open(&mut self, offset: u64) -> anyhow::Result<(ArtifactHeader, ArtifactBody)> {
			self.offsets.push(offset);
			let cut = self.cuts.pop_front().flatten();
			let mut tail = self.bytes[offset as usize..].to_vec();
			if let Some(cut) = cut {
				tail.truncate(cut);
			}
			Ok((self.header(), Box::new(std::io::Cursor::new(tail))))
		}
	}

	fn scripted_bytes(len: usize) -> Vec<u8> {
		(0..len).map(|i| (i % 251) as u8).collect()
	}

	/// A transfer cut partway leaves its `.part` on disk; the next attempt
	/// asks for the tail from that length and the finished file matches the
	/// header's checksum byte for byte.
	#[tokio::test]
	async fn an_interrupted_transfer_resumes_from_the_part_length() {
		let base = tempfile::tempdir().expect("dir");
		let source_id = Uuid::now_v7();
		let owner = std::sync::Arc::new(std::sync::Mutex::new(ScriptedOwner {
			bytes: scripted_bytes(3_000_000),
			generation: 42,
			cuts: [Some(1_000_000), None].into_iter().collect(),
			offsets: Vec::new(),
		}));
		let expected = owner.lock().unwrap().header();

		let first = transfer_artifact(base.path(), source_id, |offset| {
			let owner = owner.clone();
			async move { owner.lock().unwrap().open(offset) }
		})
		.await;
		assert!(
			matches!(first, Err(FetchError::Failed(_))),
			"a cut stream fails the transfer: {first:?}"
		);
		let part = find_part(base.path(), source_id)
			.await
			.expect("the partial file survives the failure");
		assert_eq!(part.have, 1_000_000);
		assert_eq!(part.generation, 42);
		assert_eq!(part.prefix[..], expected.checksum[..8]);

		let (path, header) = transfer_artifact(base.path(), source_id, |offset| {
			let owner = owner.clone();
			async move { owner.lock().unwrap().open(offset) }
		})
		.await
		.expect("the second attempt completes");
		assert_eq!(header, expected);
		assert_eq!(
			owner.lock().unwrap().offsets,
			vec![0, 1_000_000],
			"the resumed request starts at the part's length"
		);
		let delivered = std::fs::read(&path).expect("complete artifact");
		assert_eq!(blake3::hash(&delivered).as_bytes(), &expected.checksum);
		assert_eq!(path, part.path, "the part file is the finished artifact");
	}

	/// A part cut from an older generation is discarded and the fetch starts
	/// from zero, even though the resumed request was answered.
	#[tokio::test]
	async fn a_part_from_another_generation_starts_over() {
		let base = tempfile::tempdir().expect("dir");
		let source_id = Uuid::now_v7();
		let stale = part_path(base.path(), source_id, 7, &[9u8; 32]);
		std::fs::write(&stale, scripted_bytes(500)).expect("stale part");

		let owner = std::sync::Arc::new(std::sync::Mutex::new(ScriptedOwner {
			bytes: scripted_bytes(2_000),
			generation: 8,
			cuts: std::collections::VecDeque::new(),
			offsets: Vec::new(),
		}));
		let (path, header) = transfer_artifact(base.path(), source_id, |offset| {
			let owner = owner.clone();
			async move { owner.lock().unwrap().open(offset) }
		})
		.await
		.expect("transfer");
		assert_eq!(owner.lock().unwrap().offsets, vec![500, 0]);
		assert!(!stale.exists(), "the stale part is removed");
		assert_eq!(
			std::fs::read(&path).expect("artifact"),
			scripted_bytes(2_000)
		);
		assert_eq!(header.generation, 8);
	}

	/// The owner's listing is what retires a part: one whose generation the
	/// owner still holds stays through any number of syncs, one it has moved
	/// past goes, and one for a source the owner no longer lists goes too.
	#[tokio::test]
	async fn parts_are_pruned_against_the_owners_listing() {
		let base = tempfile::tempdir().expect("dir");
		let kept = Uuid::now_v7();
		let moved = Uuid::now_v7();
		let gone = Uuid::now_v7();
		for (id, generation) in [(kept, 7), (moved, 3), (gone, 1)] {
			std::fs::write(part_path(base.path(), id, generation, &[1u8; 32]), b"x").expect("part");
		}
		let mut moved_info = info(moved, "/mnt/pool/moved", 1);
		moved_info.generation = 4;
		prune_parts(base.path(), &[info(kept, "/mnt/pool/kept", 1), moved_info]).await;
		assert!(find_part(base.path(), kept).await.is_some());
		assert!(find_part(base.path(), moved).await.is_none());
		assert!(find_part(base.path(), gone).await.is_none());
	}

	#[test]
	fn a_part_name_round_trips_its_identity() {
		let source_id = Uuid::now_v7();
		let mut checksum = [0u8; 32];
		checksum[..8].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3]);
		let path = part_path(Path::new("/r"), source_id, 99, &checksum);
		let name = path.file_name().and_then(|n| n.to_str()).expect("name");
		assert_eq!(
			parse_part_name(name),
			Some((source_id, 99, [0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3]))
		);
		assert_eq!(parse_part_name("abc.snapshot"), None);
		assert_eq!(parse_part_name("0195.tmp"), None);
	}

	/// A resumed database fetch depends on the export being the same bytes
	/// while the store has not changed; two exports of one store must hash
	/// identically or no partial copy could ever continue.
	#[tokio::test]
	async fn an_unchanged_store_exports_identical_bytes() {
		let store_dir = tempfile::tempdir().expect("store dir");
		let manager = sd_store::SourceManager::new(store_dir.path().to_path_buf());
		let id = Uuid::now_v7().simple().to_string();
		manager
			.create(&id, &sd_store::filesystem_schema())
			.await
			.expect("create");
		let db = manager.open(&id).await.expect("open");
		let out = tempfile::tempdir().expect("out");
		let mut hashes = Vec::new();
		for n in 0..2 {
			let path = out.path().join(format!("export-{n}.db"));
			sqlx::query("VACUUM INTO ?")
				.bind(path.to_string_lossy().into_owned())
				.execute(db.pool())
				.await
				.expect("export");
			hashes.push(blake3::hash(&std::fs::read(&path).expect("read")));
		}
		db.pool().close().await;
		assert_eq!(hashes[0], hashes[1]);
	}

	/// A manifest without facts restores; the device has none until its
	/// owner answers.
	#[tokio::test]
	async fn a_manifest_without_facts_still_restores() {
		let base = tempfile::tempdir().expect("dir");
		let device_id = Uuid::now_v7();
		let replica_dir = base.path().join(device_id.simple().to_string());
		std::fs::create_dir_all(&replica_dir).expect("replica dir");
		std::fs::write(
			manifest_path(&replica_dir),
			serde_json::json!({ "device_id": device_id, "device_label": "titan", "sources": [] })
				.to_string(),
		)
		.expect("manifest");

		restore_from(base.path()).await;
		assert!(known_map().read().await.contains_key(&device_id));
		assert!(!published_facts().await.contains_key(&device_id));
	}
}
