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
//! same stream so payloads are never re-encoded through msgpack.

use crate::context::CoreContext;
use crate::service::network::device::registry::DeviceRegistry;
use async_trait::async_trait;
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
const MAX_REQUEST_FRAME: u32 = 64 * 1024;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSourceInfo {
	pub id: Uuid,
	pub root: PathBuf,
	pub volume_uuid: Option<uuid::Uuid>,
	pub attached: bool,
	pub entry_count: Option<u64>,
	pub total_bytes: Option<u64>,
	/// Version of the snapshot a peer would receive, from its size and
	/// mtime. A replica holding the same generation is already current, so
	/// this is what lets a reconnect skip re-downloading an unchanged index.
	/// Zero means the owner has no snapshot on disk yet.
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
			ByteRangeRequest::ListSources => {
				let cache = self.context.volume_index();
				let mut sources = Vec::new();
				for s in cache.sources() {
					// A source at its volume's mount point travels as the
					// volume's arena snapshot; one nested inside travels as
					// its own database, whose generation is the live database
					// file's rather than any snapshot's.
					let nested = cache
						.volume_root_of(&s.root)
						.is_some_and(|volume_root| volume_root != s.root);
					let generation = if nested {
						cache
							.source_dirs()
							.map(|dirs| database_generation(&dirs.source_dir(s.id)))
							.unwrap_or(0)
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

				// The generation names the live database this export reads,
				// WAL included, taken before the export runs: a write landing
				// mid-export moves the next listing instead of mislabeling
				// this delivery as current.
				let generation = database_generation(&source_dir);

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

/// Version of a source's live database: size and mtime folded over the main
/// file and its WAL, so any committed write moves it and a quiet store holds
/// steady. Zero when no database exists.
fn database_generation(source_dir: &Path) -> u64 {
	let mut len = 0u64;
	let mut latest: Option<std::time::SystemTime> = None;
	for name in ["data.db", "data.db-wal"] {
		if let Ok(meta) = std::fs::metadata(source_dir.join(name)) {
			len = len.saturating_add(meta.len());
			if let Ok(modified) = meta.modified() {
				latest = Some(latest.map_or(modified, |current| current.max(modified)));
			}
		}
	}
	match latest {
		Some(mtime) if len > 0 => crate::infra::source_version::source_version(len, mtime),
		_ => 0,
	}
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
}
