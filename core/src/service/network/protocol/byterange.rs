//! Byte-range protocol between paired devices.
//!
//! The read side of the mounts design (`docs/core/design/mounts.md`): stat,
//! bounded ranged reads, source enumeration, and snapshot fetch, so a peer
//! can replicate a source's index once and then stream file bytes on demand.
//!
//! One request per bidirectional stream — QUIC streams are cheap, requests
//! carry no session state, and cancelling an in-flight read is dropping the
//! stream. Framing is `[u32 BE length][rmp_serde message]`; `Read` and
//! `FetchSnapshot` responses are followed by exactly `len` raw bytes on the
//! same stream so payloads are never re-encoded through msgpack. `FetchTiles`
//! is the one request answered by many frames: one `Tile` per ask, each with
//! its bytes behind it, in the order the owner finishes them.

use crate::context::CoreContext;
use crate::device::DeviceConfig;
use crate::domain::device::{parse_device_form_factor_from_string, Device};
use crate::domain::volume::{DiskType, FileSystem, Volume, VolumeFingerprint, VolumeType};
use crate::service::network::device::registry::DeviceRegistry;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use iroh::EndpointId;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Longest read a single request may ask for; larger wants are pipelined by
/// the client as consecutive requests.
pub const MAX_READ_LEN: u64 = 8 * 1024 * 1024;
/// Largest snapshot artifact either side will transfer. Real artifacts run
/// tens of megabytes per million records; a length past this bound is a
/// corrupt header or a hostile peer, not an index.
pub const MAX_SNAPSHOT_LEN: u64 = 8 * 1024 * 1024 * 1024;
/// Cap on the encoded request frame — requests are tiny; anything larger is
/// malformed or hostile.
pub const MAX_REQUEST_FRAME: u32 = 64 * 1024;
/// Most sidecars one `FetchSidecars` answer carries, about 2.5 MB of WebP.
pub const MAX_SIDECAR_PAGE: u32 = 256;
/// Largest encoded tile either side will transfer. No encoding of a tile
/// outgrows its raw envelope, so a length past this is a corrupt header or a
/// hostile peer.
pub const MAX_TILE_LEN: u64 =
	crate::service::thumbs::TILE as u64 * crate::service::thumbs::TILE as u64 * 4;

pub const BYTERANGE_PROTOCOL_NAME: &str = "byterange";

#[derive(Debug, Serialize, Deserialize)]
pub enum ByteRangeRequest {
	/// Live size and mtime for a file under a registered source.
	Stat { path: PathBuf },
	/// Up to `len` bytes at `offset`; the response header carries the actual
	/// count (short at EOF or the `MAX_READ_LEN` cap).
	Read {
		path: PathBuf,
		offset: u64,
		len: u64,
	},
	/// The serving device's registered sources.
	ListSources,
	/// The serving device's current snapshot for one source, saved fresh
	/// before sending so the replica starts from live state.
	FetchSnapshot { source_id: Uuid },
	/// The serving device's own library statistics, so peers report the
	/// same fleet-wide totals it does.
	DeviceSummary,
	/// A consistent single-file export of one source's database, for sources
	/// whose replica must carry exactly the source's own records. Appended
	/// after the original variants so their wire indices hold.
	FetchDatabase { source_id: Uuid },
	/// What the serving device publishes about itself: its hardware and the
	/// volumes it has mounted.
	DeviceFacts,
	/// Thumbnail tiles for files under the serving device's sources, for a
	/// replica that lists those files but cannot read them to bake its own.
	FetchTiles { tiles: Vec<TileAsk> },
	/// One source's thumbnail sidecars written after `after`, oldest first,
	/// at most `limit` of them, for a replica keeping a copy.
	FetchSidecars {
		source_id: Uuid,
		after: u64,
		limit: u32,
	},
}

/// One file whose tile a replica wants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileAsk {
	pub path: PathBuf,
	/// The tile version the asker already holds, or zero when it holds none,
	/// so an unchanged tile costs a header instead of its pixels.
	pub held: u64,
}

/// What the serving device has for one [`TileAsk`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TileAnswer {
	/// The asker's version is the current one.
	Current,
	/// A newer tile: `len` bytes of WebP follow the frame, a
	/// `content_width` by `content_height` image baked from a `source_width`
	/// by `source_height` file.
	Tile {
		version: u64,
		content_width: u32,
		content_height: u32,
		source_width: u32,
		source_height: u32,
		len: u64,
	},
	/// No tile: the path is outside every attached source, or nothing on the
	/// serving device can decode the file.
	Missing,
}

/// One device's own accounting of what it holds, computed by the same
/// statistics code its own UI reads. Peers cache this so every device
/// reports fleet-wide totals without re-deriving another machine's numbers
/// from replicas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDeviceSummary {
	pub file_count: u64,
	pub total_size: u64,
	pub unique_content_count: u64,
	pub total_capacity: u64,
	pub available_capacity: u64,
}

/// What a device publishes about itself. Only that device writes these
/// facts, so a peer keeps the latest copy it received and never merges it
/// into records of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteDeviceFacts {
	pub hardware: RemoteHardware,
	/// Volumes the device has mounted and shows a person.
	pub volumes: Vec<RemoteVolumeInfo>,
}

impl RemoteDeviceFacts {
	/// Describe this device from its configuration and its volume manager's
	/// live volumes. Hidden and unmounted volumes stay out, as they do from
	/// the device's own volume list.
	pub fn describe(config: &DeviceConfig, volumes: &[Volume]) -> Self {
		Self {
			hardware: RemoteHardware {
				hardware_model: config.hardware_model.clone(),
				cpu_model: config.cpu_model.clone(),
				cpu_cores_physical: config.cpu_cores_physical,
				cpu_cores_logical: config.cpu_cores_logical,
				memory_total_bytes: config.memory_total_bytes,
				form_factor: config.form_factor.clone(),
				manufacturer: config.manufacturer.clone(),
			},
			volumes: volumes
				.iter()
				.filter(|volume| volume.is_user_visible && volume.is_mounted)
				.map(RemoteVolumeInfo::from)
				.collect(),
		}
	}
}

/// The hardware a device's own detection recorded, limited to what clients
/// read to describe a device.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoteHardware {
	pub hardware_model: Option<String>,
	pub cpu_model: Option<String>,
	pub cpu_cores_physical: Option<u32>,
	pub cpu_cores_logical: Option<u32>,
	pub memory_total_bytes: Option<i64>,
	pub form_factor: Option<String>,
	pub manufacturer: Option<String>,
}

impl RemoteHardware {
	/// Describe `device` with what its owner published, keeping any field
	/// the owner left unreported.
	pub fn apply_to(&self, device: &mut Device) {
		device.hardware_model = self.hardware_model.clone().or(device.hardware_model.take());
		device.cpu_model = self.cpu_model.clone().or(device.cpu_model.take());
		device.cpu_cores_physical = self.cpu_cores_physical.or(device.cpu_cores_physical);
		device.cpu_cores_logical = self.cpu_cores_logical.or(device.cpu_cores_logical);
		device.memory_total_bytes = self.memory_total_bytes.or(device.memory_total_bytes);
		device.form_factor = self
			.form_factor
			.as_deref()
			.map(parse_device_form_factor_from_string)
			.or(device.form_factor);
		device.manufacturer = self.manufacturer.clone().or(device.manufacturer.take());
	}
}

/// One volume as the device that mounts it sees it. `id` is the id that
/// device's sources anchor to, which is how a replica finds the drive it
/// lives on. Classifications travel as names and are parsed on receipt, so
/// one a peer's build does not know reads as unknown instead of failing the
/// whole response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteVolumeInfo {
	pub id: Uuid,
	pub fingerprint: String,
	pub name: String,
	pub display_name: Option<String>,
	pub mount_point: PathBuf,
	pub volume_type: String,
	pub disk_type: String,
	pub file_system: String,
	pub total_capacity: u64,
	pub available_space: u64,
	pub is_read_only: bool,
	pub is_tracked: bool,
	pub read_speed_mbps: Option<u64>,
	pub write_speed_mbps: Option<u64>,
}

impl From<&Volume> for RemoteVolumeInfo {
	fn from(volume: &Volume) -> Self {
		Self {
			id: volume.id,
			fingerprint: volume.fingerprint.0.clone(),
			name: volume.name.clone(),
			display_name: volume.display_name.clone(),
			mount_point: volume.mount_point.clone(),
			volume_type: volume.volume_type.to_string(),
			disk_type: volume.disk_type.to_string(),
			file_system: volume.file_system.to_string(),
			total_capacity: volume.total_capacity,
			available_space: volume.available_space,
			is_read_only: volume.is_read_only,
			is_tracked: volume.is_tracked,
			read_speed_mbps: volume.read_speed_mbps,
			write_speed_mbps: volume.write_speed_mbps,
		}
	}
}

impl RemoteVolumeInfo {
	/// The volume as this device lists it: owned by `owner`, mounted only
	/// while the owner is reachable, and last seen when its facts arrived.
	pub fn to_volume(&self, owner: Uuid, reachable: bool, observed_at: DateTime<Utc>) -> Volume {
		let mut volume = Volume::new(
			owner,
			VolumeFingerprint(self.fingerprint.clone()),
			self.name.clone(),
			self.mount_point.clone(),
		);
		volume.id = self.id;
		volume.display_name = self.display_name.clone();
		volume.volume_type = VolumeType::from_string(&self.volume_type);
		volume.disk_type = DiskType::from_string(&self.disk_type);
		volume.file_system = FileSystem::from_string(&self.file_system);
		volume.total_capacity = self.total_capacity;
		volume.available_space = self.available_space;
		volume.is_read_only = self.is_read_only;
		volume.is_tracked = self.is_tracked;
		volume.is_mounted = reachable;
		volume.auto_track_eligible = false;
		volume.read_speed_mbps = self.read_speed_mbps;
		volume.write_speed_mbps = self.write_speed_mbps;
		volume.created_at = observed_at;
		volume.updated_at = observed_at;
		volume.last_seen_at = observed_at;
		volume
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSourceInfo {
	pub id: Uuid,
	pub root: PathBuf,
	pub volume_uuid: Option<uuid::Uuid>,
	pub attached: bool,
	pub entry_count: Option<u64>,
	pub total_bytes: Option<u64>,
	/// Version of the artifact a peer would receive: the snapshot's size and
	/// mtime, or for a nested source its store's revision. A replica holding
	/// the same generation is already current, so this is what lets a
	/// reconnect skip re-downloading an unchanged index. Zero means the owner
	/// has nothing it can describe yet.
	#[serde(default)]
	pub generation: u64,
	/// The owner's arena has moved since its last save, so the snapshot on
	/// disk is behind the truth. A fetch persists it first, which is why
	/// this is reported rather than saving on every listing.
	#[serde(default)]
	pub dirty: bool,
	/// The source's root sits inside its volume rather than at its mount
	/// point, so its arena snapshot would carry sibling paths that are not
	/// the source's to share. Replicate it as its own database instead.
	#[serde(default)]
	pub nested: bool,
	/// The source's thumbnail sidecar store and how far it has written, so a
	/// replica copies only what it lacks. Nil and zero while it has none.
	#[serde(default)]
	pub sidecar_store: Uuid,
	#[serde(default)]
	pub sidecar_cursor: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ByteRangeResponse {
	Stat {
		size: u64,
		modified_secs: Option<u64>,
	},
	/// Followed by exactly `len` raw bytes on the stream.
	ReadHeader {
		len: u64,
	},
	Sources(Vec<RemoteSourceInfo>),
	DeviceSummary(RemoteDeviceSummary),
	/// Followed by exactly `len` raw bytes on the stream. The identity fields
	/// describe the artifact this response opened, which is routinely newer
	/// than the listing that prompted the fetch: the owner saves live state
	/// before serving. The receiver records these, never the listing's.
	SnapshotHeader {
		len: u64,
		/// Version of the bytes being delivered, from the open file's size
		/// and mtime. Zero when the serving side predates this field.
		#[serde(default)]
		generation: u64,
		/// blake3 of the bytes being delivered, so the receiver can validate
		/// its temporary artifact before publishing it. All zero when the
		/// serving side predates this field.
		#[serde(default)]
		checksum: [u8; 32],
	},
	Error(String),
	/// Followed by exactly `len` raw bytes: one source's database export.
	/// Identity semantics match `SnapshotHeader`. Appended after `Error` so
	/// the original variants keep their wire indices.
	DatabaseHeader {
		len: u64,
		generation: u64,
		checksum: [u8; 32],
	},
	DeviceFacts(RemoteDeviceFacts),
	/// The answer to `FetchTiles` ask number `index`.
	Tile {
		index: u32,
		answer: TileAnswer,
	},
	/// Opens the answer to `FetchSidecars`: the store the rows come from and
	/// how many `Sidecar` frames follow.
	SidecarPage {
		store: Uuid,
		rows: u32,
	},
	/// One sidecar, written at `seq`, followed by `len` bytes of WebP.
	Sidecar {
		seq: u64,
		uuid: Uuid,
		version: u64,
		content_width: u32,
		content_height: u32,
		source_width: u32,
		source_height: u32,
		len: u64,
	},
}

pub async fn write_frame<W, T>(stream: &mut W, msg: &T) -> anyhow::Result<()>
where
	W: AsyncWrite + Unpin,
	T: Serialize,
{
	let payload = rmp_serde::to_vec(msg)?;
	stream.write_u32(payload.len() as u32).await?;
	stream.write_all(&payload).await?;
	Ok(())
}

pub async fn read_frame<R, T>(stream: &mut R) -> anyhow::Result<T>
where
	R: AsyncRead + Unpin,
	T: for<'de> Deserialize<'de>,
{
	let len = stream.read_u32().await?;
	if len > MAX_REQUEST_FRAME {
		anyhow::bail!("frame of {len} bytes exceeds limit");
	}
	let mut buf = vec![0u8; len as usize];
	stream.read_exact(&mut buf).await?;
	Ok(rmp_serde::from_slice(&buf)?)
}

pub struct ByteRangeProtocolHandler {
	context: Arc<CoreContext>,
	device_registry: Arc<RwLock<DeviceRegistry>>,
}

impl ByteRangeProtocolHandler {
	pub fn new(context: Arc<CoreContext>, device_registry: Arc<RwLock<DeviceRegistry>>) -> Self {
		Self {
			context,
			device_registry,
		}
	}

	/// Reads are only served for paths that sit under an attached registered
	/// source and contain no traversal components — the same trust boundary
	/// the index itself observes.
	fn authorize_path(&self, path: &Path) -> Result<(), String> {
		if path.components().any(|c| matches!(c, Component::ParentDir)) {
			return Err("path traversal refused".into());
		}
		let allowed = self
			.context
			.volume_index()
			.sources()
			.into_iter()
			.any(|s| s.attached && path.starts_with(&s.root));
		if allowed {
			Ok(())
		} else {
			Err("path is not under an attached registered source".into())
		}
	}

	/// Answer every ask on one stream, each as soon as its tile is ready
	/// rather than in the order asked, so a slow video bake holds back only
	/// its own answer. An ask outside the attached sources is answered
	/// `Missing` like any file without a tile, so one bad path cannot fail
	/// the rest.
	async fn serve_tiles<W: AsyncWrite + Send + Unpin>(
		&self,
		asks: Vec<TileAsk>,
		send: &mut W,
	) -> anyhow::Result<()> {
		let total = asks.len();
		let mut answers: FuturesUnordered<_> = asks
			.into_iter()
			.enumerate()
			.map(|(index, ask)| {
				let authorized = self.authorize_path(&ask.path).is_ok();
				let thumbs = self.context.thumbs.clone();
				async move {
					let answer = if authorized {
						crate::service::thumbs::remote::answer(&thumbs, ask, index, total).await
					} else {
						(TileAnswer::Missing, Vec::new())
					};
					(index, answer)
				}
			})
			.collect();
		while let Some((index, (answer, bytes))) = answers.next().await {
			write_frame(
				send,
				&ByteRangeResponse::Tile {
					index: index as u32,
					answer,
				},
			)
			.await?;
			send.write_all(&bytes).await?;
		}
		Ok(())
	}

	/// One page of a registered source's sidecars, as its WebP is stored.
	async fn serve_sidecars<W: AsyncWrite + Send + Unpin>(
		&self,
		source_id: Uuid,
		after: u64,
		limit: u32,
		send: &mut W,
	) -> anyhow::Result<()> {
		let registered = self
			.context
			.volume_index()
			.sources()
			.iter()
			.any(|source| source.id == source_id);
		if !registered {
			return write_frame(send, &ByteRangeResponse::Error("no such source".into())).await;
		}
		let (store, rows) = match self
			.context
			.thumbs
			.sidecar_rows(source_id, after, limit.min(MAX_SIDECAR_PAGE))
			.await
		{
			Ok(page) => page,
			Err(err) => {
				return write_frame(send, &ByteRangeResponse::Error(err.to_string())).await;
			}
		};
		write_frame(
			send,
			&ByteRangeResponse::SidecarPage {
				store,
				rows: rows.len() as u32,
			},
		)
		.await?;
		for row in rows {
			let frame = row.tile.frame;
			write_frame(
				send,
				&ByteRangeResponse::Sidecar {
					seq: row.seq,
					uuid: row.uuid,
					version: row.tile.version,
					content_width: frame.content_width,
					content_height: frame.content_height,
					source_width: frame.source_width,
					source_height: frame.source_height,
					len: row.tile.webp.len() as u64,
				},
			)
			.await?;
			send.write_all(&row.tile.webp).await?;
		}
		Ok(())
	}

	async fn respond<W: AsyncWrite + Send + Unpin>(
		&self,
		request: ByteRangeRequest,
		send: &mut W,
	) -> anyhow::Result<()> {
		match request {
			ByteRangeRequest::Stat { path } => {
				if let Err(reason) = self.authorize_path(&path) {
					return write_frame(send, &ByteRangeResponse::Error(reason)).await;
				}
				match tokio::fs::metadata(&path).await {
					Ok(meta) => {
						let modified_secs = meta.modified().ok().and_then(|m| {
							m.duration_since(std::time::UNIX_EPOCH)
								.ok()
								.map(|d| d.as_secs())
						});
						write_frame(
							send,
							&ByteRangeResponse::Stat {
								size: meta.len(),
								modified_secs,
							},
						)
						.await
					}
					Err(err) => write_frame(send, &ByteRangeResponse::Error(err.to_string())).await,
				}
			}
			ByteRangeRequest::Read { path, offset, len } => {
				if let Err(reason) = self.authorize_path(&path) {
					return write_frame(send, &ByteRangeResponse::Error(reason)).await;
				}
				let mut file = match tokio::fs::File::open(&path).await {
					Ok(f) => f,
					Err(err) => {
						return write_frame(send, &ByteRangeResponse::Error(err.to_string())).await;
					}
				};
				let size = file.metadata().await.map(|m| m.len()).unwrap_or(0);
				let available = size.saturating_sub(offset);
				let take = len.min(MAX_READ_LEN).min(available);

				use tokio::io::AsyncSeekExt;
				if offset > 0 {
					file.seek(std::io::SeekFrom::Start(offset)).await?;
				}
				write_frame(send, &ByteRangeResponse::ReadHeader { len: take }).await?;

				let mut remaining = take;
				let mut buf = vec![0u8; 256 * 1024];
				while remaining > 0 {
					let want = remaining.min(buf.len() as u64) as usize;
					let n = file.read(&mut buf[..want]).await?;
					if n == 0 {
						// The file shrank mid-read; the client sees a short
						// stream and treats it as a failed request.
						break;
					}
					send.write_all(&buf[..n]).await?;
					remaining -= n as u64;
				}
				Ok(())
			}
			ByteRangeRequest::FetchTiles { tiles } => self.serve_tiles(tiles, send).await,
			ByteRangeRequest::FetchSidecars {
				source_id,
				after,
				limit,
			} => self.serve_sidecars(source_id, after, limit, send).await,
			ByteRangeRequest::ListSources => {
				let cache = self.context.volume_index();
				let mut sources = Vec::new();
				for s in cache.sources() {
					// A source at its volume's mount point travels as the
					// volume's arena snapshot; one nested inside travels as
					// its own database, whose generation is its store's
					// revision rather than any snapshot's.
					let nested = cache
						.volume_root_of(&s.root)
						.is_some_and(|volume_root| volume_root != s.root);
					let generation = if nested {
						store_generation(cache, s.id).await
					} else {
						cache
							.source_snapshot_path(s.id)
							.and_then(|path| std::fs::metadata(path).ok())
							.and_then(|meta| {
								Some(crate::infra::source_version::source_version(
									meta.len(),
									meta.modified().ok()?,
								))
							})
							.unwrap_or(0)
					};
					// Only an attached source can have live changes; a
					// detached one is already only its snapshot. A database
					// generation already moves with every committed write.
					let dirty = if s.attached && !nested {
						cache.resolve_index(&s.root).read().await.is_dirty()
					} else {
						false
					};
					let (sidecar_store, sidecar_cursor) =
						self.context.thumbs.sidecar_mark(s.id).await;
					sources.push(RemoteSourceInfo {
						id: s.id,
						root: s.root,
						volume_uuid: s.volume_uuid,
						attached: s.attached,
						entry_count: s.entry_count,
						total_bytes: s.total_bytes,
						generation,
						dirty,
						nested,
						sidecar_store,
						sidecar_cursor,
					});
				}
				write_frame(send, &ByteRangeResponse::Sources(sources)).await
			}
			ByteRangeRequest::DeviceSummary => {
				let libraries = self.context.libraries().await.get_open_libraries().await;
				let Some(library) = libraries.first() else {
					return write_frame(send, &ByteRangeResponse::Error("no open library".into()))
						.await;
				};
				let db = library.db().conn();
				let (file_count, total_size) =
					crate::library::Library::calculate_file_statistics_static(db)
						.await
						.unwrap_or((0, 0));
				let unique_content_count =
					crate::library::Library::calculate_unique_content_count_static(db)
						.await
						.unwrap_or(0);
				let live = crate::library::Library::live_capacity_by_fingerprint(
					&self.context.volume_manager,
				)
				.await;
				let (total_capacity, available_capacity) =
					crate::library::Library::calculate_volume_capacity_static(db, &live)
						.await
						.unwrap_or((0, 0));
				write_frame(
					send,
					&ByteRangeResponse::DeviceSummary(RemoteDeviceSummary {
						file_count,
						total_size,
						unique_content_count,
						total_capacity,
						available_capacity,
					}),
				)
				.await
			}
			ByteRangeRequest::FetchSnapshot { source_id } => {
				let cache = self.context.volume_index();
				let Some(source) = cache.sources().into_iter().find(|s| s.id == source_id) else {
					return write_frame(send, &ByteRangeResponse::Error("unknown source".into()))
						.await;
				};
				// Persist live arena state first so the replica isn't stale
				// by a whole session; failure falls back to the on-disk file.
				if source.attached {
					if let Err(err) = cache.save_snapshot(&source.root).await {
						tracing::debug!("pre-fetch snapshot save failed: {err}");
					}
				}
				let Some(snapshot_path) = cache.source_snapshot_path(source_id) else {
					return write_frame(
						send,
						&ByteRangeResponse::Error("source has no persistent snapshot".into()),
					)
					.await;
				};
				let file = match tokio::fs::File::open(&snapshot_path).await {
					Ok(f) => f,
					Err(err) => {
						return write_frame(
							send,
							&ByteRangeResponse::Error(format!("snapshot unavailable: {err}")),
						)
						.await;
					}
				};
				serve_file_with_identity(send, file, None, false).await
			}
			ByteRangeRequest::FetchDatabase { source_id } => {
				let cache = self.context.volume_index();
				if !cache.sources().into_iter().any(|s| s.id == source_id) {
					return write_frame(send, &ByteRangeResponse::Error("unknown source".into()))
						.await;
				}
				let Some(db) = cache.read_store(source_id).await else {
					return write_frame(
						send,
						&ByteRangeResponse::Error("source has no readable store".into()),
					)
					.await;
				};
				let Some(dirs) = cache.source_dirs() else {
					return write_frame(
						send,
						&ByteRangeResponse::Error("no persistent source layout".into()),
					)
					.await;
				};
				let source_dir = dirs.source_dir(source_id);

				// The generation names the store revision this export starts
				// from, read before the export runs: a write landing mid-export
				// moves the next listing instead of mislabeling this delivery
				// as current.
				let generation = match db.revision().await {
					Ok(revision) => revision_generation(revision),
					Err(err) => {
						return write_frame(
							send,
							&ByteRangeResponse::Error(format!("store revision unreadable: {err}")),
						)
						.await;
					}
				};

				// VACUUM INTO produces a consistent, compact single-file copy
				// from one read transaction; walkers keep writing meanwhile.
				let export_path = source_dir.join(format!("export-{}.db", Uuid::now_v7().simple()));
				let exported = sqlx::query("VACUUM INTO ?")
					.bind(export_path.to_string_lossy().into_owned())
					.execute(db.pool())
					.await;
				if let Err(err) = exported {
					let _ = tokio::fs::remove_file(&export_path).await;
					return write_frame(
						send,
						&ByteRangeResponse::Error(format!("export failed: {err}")),
					)
					.await;
				}

				let served = match tokio::fs::File::open(&export_path).await {
					Ok(file) => serve_file_with_identity(send, file, Some(generation), true).await,
					Err(err) => {
						write_frame(
							send,
							&ByteRangeResponse::Error(format!("export unavailable: {err}")),
						)
						.await
					}
				};
				let _ = tokio::fs::remove_file(&export_path).await;
				served
			}
			ByteRangeRequest::DeviceFacts => {
				let config = match self.context.device_manager.config() {
					Ok(config) => config,
					Err(err) => {
						return write_frame(
							send,
							&ByteRangeResponse::Error(format!("device config unavailable: {err}")),
						)
						.await;
					}
				};
				let volumes = self.context.volume_manager.get_all_volumes().await;
				write_frame(
					send,
					&ByteRangeResponse::DeviceFacts(RemoteDeviceFacts::describe(&config, &volumes)),
				)
				.await
			}
		}
	}
}

/// Hash an open file, emit its identity header, and stream exactly the bytes
/// measured. Identity comes from the open handle, not the path: a writer
/// renaming a new file into place cannot change what this stream carries,
/// and the header must name what the stream holds rather than what the
/// listing that prompted the fetch believed.
async fn serve_file_with_identity<W: AsyncWrite + Send + Unpin>(
	send: &mut W,
	file: tokio::fs::File,
	generation: Option<u64>,
	database: bool,
) -> anyhow::Result<()> {
	let meta = file.metadata().await?;
	let len = meta.len();
	if len > MAX_SNAPSHOT_LEN {
		return write_frame(
			send,
			&ByteRangeResponse::Error(format!(
				"artifact of {len} bytes exceeds the transfer bound"
			)),
		)
		.await;
	}
	let generation = generation.unwrap_or_else(|| {
		meta.modified()
			.map(|mtime| crate::infra::source_version::source_version(len, mtime))
			.unwrap_or(0)
	});

	let std_file = file.into_std().await;
	let hashed =
		tokio::task::spawn_blocking(move || -> std::io::Result<(std::fs::File, [u8; 32])> {
			use std::io::{Seek, SeekFrom};
			let mut hasher = blake3::Hasher::new();
			std::io::copy(&mut &std_file, &mut hasher)?;
			(&std_file).seek(SeekFrom::Start(0))?;
			Ok((std_file, *hasher.finalize().as_bytes()))
		})
		.await?;
	let (std_file, checksum) = match hashed {
		Ok(pair) => pair,
		Err(err) => {
			return write_frame(
				send,
				&ByteRangeResponse::Error(format!("artifact unreadable: {err}")),
			)
			.await;
		}
	};
	let mut file = tokio::fs::File::from_std(std_file);

	let header = if database {
		ByteRangeResponse::DatabaseHeader {
			len,
			generation,
			checksum,
		}
	} else {
		ByteRangeResponse::SnapshotHeader {
			len,
			generation,
			checksum,
		}
	};
	write_frame(send, &header).await?;

	let mut buf = vec![0u8; 256 * 1024];
	let mut sent = 0u64;
	while sent < len {
		let want = (len - sent).min(buf.len() as u64) as usize;
		let n = file.read(&mut buf[..want]).await?;
		if n == 0 {
			// Short reads leave a short stream; the client's declared-length
			// accounting fails the transfer.
			break;
		}
		send.write_all(&buf[..n]).await?;
		sent += n as u64;
	}
	Ok(())
}

/// A nested source's generation, from its store's revision. Zero, which
/// always transfers, when the store cannot be read.
async fn store_generation(
	cache: &crate::ops::indexing::volume_index::VolumeIndex,
	source_id: Uuid,
) -> u64 {
	let Some(db) = cache.read_store(source_id).await else {
		return 0;
	};
	match db.revision().await {
		Ok(revision) => revision_generation(revision),
		Err(err) => {
			tracing::debug!(source = %source_id, %err, "store revision unreadable");
			0
		}
	}
}

/// Fold a store revision into a generation word. The store id takes part so
/// a store recreated from scratch never matches a copy of its predecessor.
/// Zero is reserved for "nothing to describe", which always transfers.
fn revision_generation(revision: sd_store::Revision) -> u64 {
	let mut hasher = blake3::Hasher::new();
	hasher.update(revision.store_id.as_bytes());
	hasher.update(&revision.value.to_le_bytes());
	let mut word = [0u8; 8];
	word.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
	u64::from_le_bytes(word).max(1)
}

#[async_trait]
impl super::ProtocolHandler for ByteRangeProtocolHandler {
	fn protocol_name(&self) -> &str {
		BYTERANGE_PROTOCOL_NAME
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn AsyncRead + Send + Unpin>,
		remote_node_id: EndpointId,
	) {
		// Only paired devices are served; the registry is the trust set.
		let paired = {
			let registry = self.device_registry.read().await;
			registry.get_device_by_node(remote_node_id).is_some()
		};
		if !paired {
			let _ = write_frame(
				&mut send,
				&ByteRangeResponse::Error("device not paired".into()),
			)
			.await;
			return;
		}

		let request: ByteRangeRequest = match read_frame(&mut recv).await {
			Ok(r) => r,
			Err(err) => {
				tracing::debug!("byterange: bad request frame: {err}");
				return;
			}
		};

		if let Err(err) = self.respond(request, &mut send).await {
			tracing::debug!("byterange: response failed: {err}");
		}
		let _ = send.flush().await;
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_request(
		&self,
		_from_device: Uuid,
		_request_data: Vec<u8>,
	) -> crate::service::network::Result<Vec<u8>> {
		Err(crate::service::network::NetworkingError::Protocol(
			"byterange is stream-only".into(),
		))
	}

	async fn handle_response(
		&self,
		_from_device: Uuid,
		_from_node: EndpointId,
		_response_data: Vec<u8>,
	) -> crate::service::network::Result<()> {
		Ok(())
	}

	async fn handle_event(
		&self,
		_event: super::ProtocolEvent,
	) -> crate::service::network::Result<()> {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A peer built before the header carried artifact identity sends only a
	/// length. Its frames must keep decoding, with the identity fields
	/// reading as absent rather than the frame failing.
	#[test]
	fn an_old_snapshot_header_still_decodes() {
		#[allow(dead_code)]
		#[derive(Serialize)]
		enum LegacyResponse {
			Stat {
				size: u64,
				modified_secs: Option<u64>,
			},
			ReadHeader {
				len: u64,
			},
			Sources(Vec<RemoteSourceInfo>),
			DeviceSummary(RemoteDeviceSummary),
			SnapshotHeader {
				len: u64,
			},
			Error(String),
		}

		let bytes =
			rmp_serde::to_vec(&LegacyResponse::SnapshotHeader { len: 9 }).expect("serialize");
		let decoded: ByteRangeResponse = rmp_serde::from_slice(&bytes).expect("deserialize");
		match decoded {
			ByteRangeResponse::SnapshotHeader {
				len,
				generation,
				checksum,
			} => {
				assert_eq!(len, 9);
				assert_eq!(generation, 0, "an absent generation reads as zero");
				assert_eq!(checksum, [0u8; 32], "an absent checksum reads as zero");
			}
			other => panic!("decoded to the wrong variant: {other:?}"),
		}
	}

	#[test]
	fn a_revision_generation_follows_the_store_and_its_count() {
		let revision = sd_store::Revision {
			store_id: Uuid::now_v7(),
			value: 7,
		};
		let generation = revision_generation(revision);
		assert_eq!(generation, revision_generation(revision));
		assert_ne!(
			generation,
			revision_generation(sd_store::Revision {
				value: 8,
				..revision
			})
		);
		assert_ne!(
			generation,
			revision_generation(sd_store::Revision {
				store_id: Uuid::now_v7(),
				..revision
			}),
			"a recreated store never matches its predecessor"
		);
		assert_ne!(revision_generation(sd_store::Revision::UNTRACKED), 0);
	}

	fn volume(name: &str, visible: bool, mounted: bool) -> Volume {
		let mut volume = Volume::new(
			Uuid::now_v7(),
			VolumeFingerprint(format!("fingerprint-{name}")),
			name.to_string(),
			PathBuf::from(format!("/mnt/{name}")),
		);
		volume.is_user_visible = visible;
		volume.is_mounted = mounted;
		volume
	}

	/// A device describes the volumes it shows a person, under the ids its
	/// sources anchor to, with its hardware alongside.
	#[test]
	fn facts_describe_what_the_device_shows() {
		let mut config = DeviceConfig::new("titan".to_string(), "Linux".to_string());
		config.cpu_model = Some("AMD EPYC 4464P".to_string());
		config.memory_total_bytes = Some(128 << 30);

		let shown = volume("pool", true, true);
		let facts = RemoteDeviceFacts::describe(
			&config,
			&[
				shown.clone(),
				volume("boot", false, true),
				volume("usb", true, false),
			],
		);

		assert_eq!(
			facts.volumes.iter().map(|v| v.id).collect::<Vec<_>>(),
			vec![shown.id],
			"hidden and unmounted volumes stay out"
		);
		assert_eq!(facts.hardware.cpu_model.as_deref(), Some("AMD EPYC 4464P"));
		assert_eq!(facts.hardware.memory_total_bytes, Some(128 << 30));
	}

	/// A published volume lists on a peer as its owner's, classified the way
	/// the owner classified it, and mounted only while the owner is reachable.
	#[test]
	fn a_published_volume_lists_as_its_owners() {
		let mut original = volume("pool", true, true);
		original.display_name = Some("jamie-nas".to_string());
		original.volume_type = VolumeType::External;
		original.disk_type = DiskType::HDD;
		original.file_system = FileSystem::ZFS;
		original.total_capacity = 40 << 40;
		original.available_space = 4 << 40;

		let owner = Uuid::now_v7();
		let observed_at = DateTime::from_timestamp(1_789_700_000, 0).expect("time");
		let listed = RemoteVolumeInfo::from(&original).to_volume(owner, false, observed_at);

		assert_eq!(listed.id, original.id, "the anchor id survives the trip");
		assert_eq!(listed.device_id, owner);
		assert_eq!(listed.display_name.as_deref(), Some("jamie-nas"));
		assert_eq!(listed.volume_type, VolumeType::External);
		assert_eq!(listed.disk_type, DiskType::HDD);
		assert_eq!(listed.file_system, FileSystem::ZFS);
		assert_eq!(
			(listed.total_capacity, listed.available_space),
			(40 << 40, 4 << 40)
		);
		assert!(
			!listed.is_mounted,
			"an unreachable owner's volume is not mounted here"
		);
		assert_eq!(listed.last_seen_at, observed_at);
	}

	/// Facts cross the frame codec intact, and a classification this build
	/// does not know arrives as unknown instead of failing the response.
	#[tokio::test]
	async fn device_facts_survive_the_frame_codec() {
		let mut info = RemoteVolumeInfo::from(&volume("pool", true, true));
		info.volume_type = "Holographic".to_string();
		let facts = RemoteDeviceFacts {
			hardware: RemoteHardware::default(),
			volumes: vec![info],
		};

		let mut wire = Vec::new();
		write_frame(&mut wire, &ByteRangeResponse::DeviceFacts(facts.clone()))
			.await
			.expect("encode");
		match read_frame(&mut wire.as_slice()).await.expect("decode") {
			ByteRangeResponse::DeviceFacts(received) => {
				assert_eq!(received, facts);
				let listed = received.volumes[0].to_volume(Uuid::now_v7(), true, Utc::now());
				assert_eq!(listed.volume_type, VolumeType::Unknown);
			}
			other => panic!("decoded to the wrong variant: {other:?}"),
		}
	}
}
