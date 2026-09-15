//! Remote operation dispatch between paired devices.
//!
//! Carries one Wire method call — the same `{method, library_id, payload}`
//! envelope the local daemon socket speaks — to a paired device, executes it
//! there through the same operation registries, and returns the JSON result.
//! This is what lets a client say `--device titan` and have the op run where
//! the data lives.
//!
//! One request per bidirectional stream, `[u32 BE length][rmp_serde]` frames,
//! mirroring the byterange protocol. Only paired devices are served: pairing
//! is the trust boundary, and a paired device already reads source bytes over
//! byterange, so operating the daemon is the same trust tier made explicit.

use crate::context::CoreContext;
use crate::service::network::core::REMOTE_OPS_ALPN;
use crate::service::network::device::registry::DeviceRegistry;
use async_trait::async_trait;
use iroh::EndpointId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::RwLock;
use uuid::Uuid;

pub const REMOTE_OPS_PROTOCOL_NAME: &str = "remote_ops";

/// Requests are method names and inputs; responses are whole query results.
/// A source listing with hundreds of rows fits easily, a runaway frame does
/// not.
const MAX_FRAME: u32 = 16 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteOpRequest {
	pub method: String,
	/// A library on the *serving* device. `None` lets the server resolve its
	/// own open library, since a caller cannot know a peer's library ids
	/// without asking first.
	pub library_id: Option<Uuid>,
	pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RemoteOpResponse {
	Ok(serde_json::Value),
	Err(String),
}

async fn write_frame<W, T>(stream: &mut W, msg: &T) -> anyhow::Result<()>
where
	W: AsyncWrite + Unpin,
	T: Serialize,
{
	let payload = rmp_serde::to_vec(msg)?;
	if payload.len() as u32 > MAX_FRAME {
		anyhow::bail!("frame of {} bytes exceeds limit", payload.len());
	}
	stream.write_u32(payload.len() as u32).await?;
	stream.write_all(&payload).await?;
	Ok(())
}

async fn read_frame<R, T>(stream: &mut R) -> anyhow::Result<T>
where
	R: AsyncRead + Unpin,
	T: for<'de> Deserialize<'de>,
{
	let len = stream.read_u32().await?;
	if len > MAX_FRAME {
		anyhow::bail!("frame of {len} bytes exceeds limit");
	}
	let mut buf = vec![0u8; len as usize];
	stream.read_exact(&mut buf).await?;
	Ok(rmp_serde::from_slice(&buf)?)
}

/// Execute one operation on a paired device and return its JSON result.
pub async fn call(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	method: &str,
	library_id: Option<Uuid>,
	payload: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
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
		.connect(iroh::EndpointAddr::new(node_id), REMOTE_OPS_ALPN)
		.await
		.map_err(|e| anyhow::anyhow!("connect failed: {e}"))?;
	let (mut send, mut recv) = connection
		.open_bi()
		.await
		.map_err(|e| anyhow::anyhow!("open_bi failed: {e}"))?;

	write_frame(
		&mut send,
		&RemoteOpRequest {
			method: method.to_string(),
			library_id,
			payload,
		},
	)
	.await?;
	let _ = send.finish();

	match read_frame(&mut recv).await? {
		RemoteOpResponse::Ok(value) => Ok(value),
		RemoteOpResponse::Err(err) => Err(anyhow::anyhow!("{err}")),
	}
}

pub struct RemoteOpsProtocolHandler {
	context: Arc<CoreContext>,
	device_registry: Arc<RwLock<DeviceRegistry>>,
}

impl RemoteOpsProtocolHandler {
	pub fn new(context: Arc<CoreContext>, device_registry: Arc<RwLock<DeviceRegistry>>) -> Self {
		Self {
			context,
			device_registry,
		}
	}

	async fn respond(&self, request: RemoteOpRequest) -> RemoteOpResponse {
		// A caller that names no library means "your library": resolve the
		// first open one so library ops work without the caller learning the
		// serving device's library ids first. Core ops ignore the value.
		let library_id = match request.library_id {
			Some(id) => Some(id),
			None => self
				.context
				.libraries()
				.await
				.get_open_libraries()
				.await
				.first()
				.map(|library| library.id()),
		};

		match crate::infra::daemon::rpc::execute_json_operation_with_context(
			&request.method,
			library_id,
			request.payload,
			&self.context,
		)
		.await
		{
			Ok(value) => RemoteOpResponse::Ok(value),
			Err(err) => RemoteOpResponse::Err(err),
		}
	}
}

#[async_trait]
impl super::ProtocolHandler for RemoteOpsProtocolHandler {
	fn protocol_name(&self) -> &str {
		REMOTE_OPS_PROTOCOL_NAME
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn AsyncRead + Send + Unpin>,
		remote_node_id: EndpointId,
	) {
		// Only paired devices are served; the registry is the trust set.
		let device_id = {
			let registry = self.device_registry.read().await;
			registry.get_device_by_node(remote_node_id)
		};
		let Some(device_id) = device_id else {
			let _ = write_frame(
				&mut send,
				&RemoteOpResponse::Err("device not paired".into()),
			)
			.await;
			return;
		};

		let request: RemoteOpRequest = match read_frame(&mut recv).await {
			Ok(request) => request,
			Err(err) => {
				tracing::debug!("remote_ops: bad request frame: {err}");
				return;
			}
		};

		tracing::info!("remote_ops: {} from device {device_id}", request.method);
		let response = self.respond(request).await;
		if let Err(err) = write_frame(&mut send, &response).await {
			tracing::debug!("remote_ops: response failed: {err}");
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
			"remote_ops is stream-only".into(),
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
