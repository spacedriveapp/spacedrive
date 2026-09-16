//! Peer side of mounts: replicate a paired device's source indexes locally
//! and stream file bytes from it on demand.
//!
//! Metadata is replicated, never fetched per-call — the remote source's
//! snapshot is pulled once (and on explicit re-sync) and loaded into a local
//! arena, so browsing a peer's files costs the same as browsing a local
//! detached drive. Only `open`-time reads cross the network, over the
//! byterange protocol.

use crate::context::CoreContext;
use crate::ops::indexing::ephemeral::index::EphemeralIndex;
use crate::service::network::core::BYTERANGE_ALPN;
use crate::service::network::protocol::{
	byterange::{read_frame, write_frame, ByteRangeRequest, ByteRangeResponse, MAX_READ_LEN},
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
	pub index: Arc<TokioRwLock<EphemeralIndex>>,
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

			let snapshot_path = replica_dir.join(format!("{}.snapshot", entry.info.id.simple()));
			let restored = {
				let path = snapshot_path.clone();
				tokio::task::spawn_blocking(move || EphemeralIndex::load_snapshot(&path)).await
			};
			let index = match restored {
				Ok(Ok(Some((index, _meta)))) => index,
				Ok(Ok(None)) | Ok(Err(_)) | Err(_) => {
					tracing::warn!(
						source = %entry.info.id,
						path = %snapshot_path.display(),
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
		// current. Only a moved generation, or unsaved changes on the
		// owner's side, are worth a transfer.
		if let Some(existing) = remote_share(info.id).await {
			let unchanged =
				!info.dirty && info.generation != 0 && existing.generation == info.generation;
			if unchanged {
				synced += 1;
				continue;
			}
		}

		let (response, mut body) = match request(
			context,
			device_id,
			&ByteRangeRequest::FetchSnapshot { source_id: info.id },
		)
		.await
		{
			Ok(r) => r,
			Err(err) => {
				tracing::warn!("snapshot fetch for {} failed: {err}", info.id);
				continue;
			}
		};
		let len = match response {
			ByteRangeResponse::SnapshotHeader { len } => len,
			other => {
				tracing::warn!("snapshot fetch for {}: unexpected {other:?}", info.id);
				continue;
			}
		};

		let snapshot_path = replica_dir.join(format!("{}.snapshot", info.id.simple()));
		let tmp_path = replica_dir.join(format!("{}.tmp", Uuid::now_v7().simple()));
		{
			let mut file = tokio::fs::File::create(&tmp_path).await?;
			let mut remaining = len;
			let mut buf = vec![0u8; 256 * 1024];
			use tokio::io::AsyncWriteExt;
			while remaining > 0 {
				let want = remaining.min(buf.len() as u64) as usize;
				let n = body.read(&mut buf[..want]).await?;
				if n == 0 {
					anyhow::bail!("snapshot stream ended early");
				}
				file.write_all(&buf[..n]).await?;
				remaining -= n as u64;
			}
			file.sync_all().await?;
		}
		tokio::fs::rename(&tmp_path, &snapshot_path).await?;

		// Loading is blocking (mmap arena rebuild); keep it off the runtime.
		let loaded = {
			let path = snapshot_path.clone();
			tokio::task::spawn_blocking(move || EphemeralIndex::load_snapshot(&path)).await??
		};
		let Some((index, _meta)) = loaded else {
			tracing::warn!("snapshot for {} was unreadable after fetch", info.id);
			continue;
		};

		let share = Arc::new(RemoteShare {
			device_id,
			device_label: device_label.clone(),
			generation: info.generation,
			info: info.clone(),
			index: Arc::new(TokioRwLock::new(index)),
			synced_at_secs: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.map(|d| d.as_secs())
				.unwrap_or(0),
		});
		shares_map().write().await.insert(info.id, share);
		synced += 1;
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
		let mut index = EphemeralIndex::new().expect("index");
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
}
