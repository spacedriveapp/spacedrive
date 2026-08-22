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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSourceInfo {
	pub id: Uuid,
	pub root: PathBuf,
	pub fingerprint: Option<String>,
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
	/// Followed by exactly `len` raw bytes on the stream.
	SnapshotHeader {
		len: u64,
	},
	Error(String),
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
			.ephemeral_cache()
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
				let cache = self.context.ephemeral_cache();
				let dirs =
					crate::infra::source_dirs::SourceDirs::under_data_dir(&self.context.data_dir)
						.ok();
				let mut sources = Vec::new();
				for s in cache.sources() {
					let generation = dirs
						.as_ref()
						.and_then(|dirs| std::fs::metadata(dirs.snapshot_file(s.id)).ok())
						.and_then(|meta| {
							Some(crate::infra::source_version::source_version(
								meta.len(),
								meta.modified().ok()?,
							))
						})
						.unwrap_or(0);
					// Only an attached source can have live changes; a
					// detached one is already only its snapshot.
					let dirty = if s.attached {
						cache.resolve_index(&s.root).read().await.is_dirty()
					} else {
						false
					};
					sources.push(RemoteSourceInfo {
						id: s.id,
						root: s.root,
						fingerprint: s.fingerprint,
						attached: s.attached,
						entry_count: s.entry_count,
						total_bytes: s.total_bytes,
						generation,
						dirty,
					});
				}
				write_frame(send, &ByteRangeResponse::Sources(sources)).await
			}
			ByteRangeRequest::FetchSnapshot { source_id } => {
				let cache = self.context.ephemeral_cache();
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
				let Some(snapshot_path) = source.directory.map(|d| d.join("ephemeral.snapshot"))
				else {
					return write_frame(
						send,
						&ByteRangeResponse::Error("source has no persistent snapshot".into()),
					)
					.await;
				};
				let mut file = match tokio::fs::File::open(&snapshot_path).await {
					Ok(f) => f,
					Err(err) => {
						return write_frame(
							send,
							&ByteRangeResponse::Error(format!("snapshot unavailable: {err}")),
						)
						.await;
					}
				};
				let len = file.metadata().await?.len();
				write_frame(send, &ByteRangeResponse::SnapshotHeader { len }).await?;
				let mut buf = vec![0u8; 256 * 1024];
				loop {
					let n = file.read(&mut buf).await?;
					if n == 0 {
						break;
					}
					send.write_all(&buf[..n]).await?;
				}
				Ok(())
			}
		}
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
