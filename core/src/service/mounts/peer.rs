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
		read_frame, write_frame, ByteRangeRequest, ByteRangeResponse, MAX_READ_LEN,
		MAX_SNAPSHOT_LEN,
	},
	RemoteSourceInfo,
};
use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
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

/// Open a replica's delivered database read-only, when one is on disk. The
/// caller closes the pool when done; a replica artifact is opened on demand
/// rather than held, since the next delivery replaces the file wholesale.
pub async fn open_replica_db(
	context: &Arc<crate::context::CoreContext>,
	device_id: Uuid,
	source_id: Uuid,
) -> Option<sd_store::SourceDb> {
	let db_path = context
		.data_dir
		.join("mounts-remote")
		.join(device_id.simple().to_string())
		.join(format!("{}.db", source_id.simple()));
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

/// A device's replica inventory as last synced, written beside the artifacts
/// it describes so the inventory survives a restart without the owner. A
/// replica has an owner and a validated generation even when no arena is
/// loaded; losing the list because the owner is unreachable would make a
/// drive's departure erase the knowledge that its copies exist.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicaManifest {
	pub device_id: Uuid,
	pub device_label: String,
	pub sources: Vec<ReplicaEntry>,
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

	let replica_dir = context
		.data_dir
		.join("mounts-remote")
		.join(device_id.simple().to_string());
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

/// Stream a declared number of bytes into a temporary artifact, hashing as
/// they land. A short stream or a checksum mismatch fails the transfer with
/// the temporary file still in place for the caller to remove; nothing here
/// touches the published artifact.
async fn receive_artifact<R: tokio::io::AsyncRead + Unpin + ?Sized>(
	body: &mut R,
	tmp_path: &std::path::Path,
	len: u64,
	expected_checksum: [u8; 32],
) -> anyhow::Result<()> {
	use tokio::io::AsyncWriteExt;
	let mut file = tokio::fs::File::create(tmp_path).await?;
	let mut hasher = blake3::Hasher::new();
	let mut remaining = len;
	let mut buf = vec![0u8; 256 * 1024];
	while remaining > 0 {
		let want = remaining.min(buf.len() as u64) as usize;
		let n = body.read(&mut buf[..want]).await?;
		if n == 0 {
			anyhow::bail!("stream ended {remaining} bytes short of the {len} the header declared");
		}
		hasher.update(&buf[..n]);
		file.write_all(&buf[..n]).await?;
		remaining -= n as u64;
	}
	file.sync_all().await?;

	// A zeroed checksum is an owner that predates the field; the parse
	// validation behind this still stands between the bytes and publication.
	if expected_checksum != [0u8; 32] {
		let received = *hasher.finalize().as_bytes();
		if received != expected_checksum {
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
	replica_dir: &std::path::Path,
	info: &RemoteSourceInfo,
) -> anyhow::Result<()> {
	let started = std::time::Instant::now();
	let (response, mut body) = request(
		context,
		device_id,
		&ByteRangeRequest::FetchDatabase { source_id: info.id },
	)
	.await?;
	let (len, delivered, checksum) = match response {
		ByteRangeResponse::DatabaseHeader {
			len,
			generation,
			checksum,
		} => (len, generation, checksum),
		other => anyhow::bail!("unexpected response: {other:?}"),
	};
	if len > MAX_SNAPSHOT_LEN {
		anyhow::bail!("declared artifact of {len} bytes exceeds the transfer bound");
	}

	let tmp_path = replica_dir.join(format!("{}.tmp", Uuid::now_v7().simple()));
	if let Err(err) = receive_artifact(&mut body, &tmp_path, len, checksum).await {
		let _ = tokio::fs::remove_file(&tmp_path).await;
		return Err(err);
	}

	// The parse-and-rebuild is the gate: the temporary database must open
	// read-only and yield an arena before it may replace anything.
	let opened = sd_store::SourceManager::open_file_read_only(&tmp_path).await;
	let db = match opened {
		Ok(db) => db,
		Err(err) => {
			let _ = tokio::fs::remove_file(&tmp_path).await;
			anyhow::bail!("delivered database refused to open: {err}");
		}
	};
	let index = match arena_from_database(&db, &info.root).await {
		Ok(index) => index,
		Err(err) => {
			let _ = tokio::fs::remove_file(&tmp_path).await;
			return Err(err);
		}
	};
	// The pool must release its handle before the file moves.
	db.pool().close().await;

	let db_path = replica_dir.join(format!("{}.db", info.id.simple()));
	tokio::fs::rename(&tmp_path, &db_path).await?;
	// A database replica supersedes any arena-snapshot artifact the source
	// had before it was delivered this way.
	let _ =
		tokio::fs::remove_file(replica_dir.join(format!("{}.snapshot", info.id.simple()))).await;

	let share = Arc::new(RemoteShare {
		device_id,
		device_label: device_label.to_string(),
		generation: delivered,
		info: info.clone(),
		index: Arc::new(TokioRwLock::new(index)),
		synced_at_secs: now_secs(),
	});
	shares_map().write().await.insert(info.id, share);
	tracing::info!(
		source = %info.id,
		generation = delivered,
		bytes = len,
		elapsed_ms = started.elapsed().as_millis() as u64,
		"replica database received and published"
	);
	Ok(())
}

/// Fetch one source's snapshot into the device's replica directory and
/// publish it. Every failure path removes its temporary file and leaves the
/// previously published artifact and share untouched.
async fn fetch_and_publish(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	device_label: &str,
	replica_dir: &std::path::Path,
	info: &RemoteSourceInfo,
) -> anyhow::Result<()> {
	let started = std::time::Instant::now();
	let (response, mut body) = request(
		context,
		device_id,
		&ByteRangeRequest::FetchSnapshot { source_id: info.id },
	)
	.await?;
	let (len, delivered, checksum) = match response {
		ByteRangeResponse::SnapshotHeader {
			len,
			generation,
			checksum,
		} => (len, generation, checksum),
		other => anyhow::bail!("unexpected response: {other:?}"),
	};
	if len > MAX_SNAPSHOT_LEN {
		anyhow::bail!("declared artifact of {len} bytes exceeds the transfer bound");
	}

	let tmp_path = replica_dir.join(format!("{}.tmp", Uuid::now_v7().simple()));
	if let Err(err) = receive_artifact(&mut body, &tmp_path, len, checksum).await {
		let _ = tokio::fs::remove_file(&tmp_path).await;
		return Err(err);
	}

	// An owner that predates header identity sends zero; the listing's
	// generation is then the only name these bytes have.
	let generation = if delivered != 0 {
		delivered
	} else {
		info.generation
	};
	validate_and_publish(
		replica_dir,
		device_id,
		device_label,
		info,
		generation,
		&tmp_path,
	)
	.await?;
	tracing::info!(
		source = %info.id,
		generation,
		bytes = len,
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

	let replica_dir = context
		.data_dir
		.join("mounts-remote")
		.join(device_id.simple().to_string());
	tokio::fs::create_dir_all(&replica_dir).await?;

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
			// answers on the snapshot path.
			match fetch_database_and_publish(context, device_id, &device_label, &replica_dir, info)
				.await
			{
				Ok(()) => Ok(()),
				Err(err) => {
					tracing::debug!(
						source = %info.id,
						%err,
						"database fetch unavailable; falling back to the arena snapshot"
					);
					fetch_and_publish(context, device_id, &device_label, &replica_dir, info).await
				}
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
}
