//! Sync Protocol Multiplexer
//!
//! Routes sync messages to the correct library's SyncProtocolHandler.
//! Solves the problem of multiple libraries trying to register sync handlers
//! when the protocol registry only allows one handler per protocol name.

use super::{handler::SyncProtocolHandler, messages::SyncMessage};
use crate::service::{
	network::{device::DeviceRegistry, protocol::ProtocolEvent, NetworkingError, Result},
	sync::{peer::PeerSync, BackfillManager},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

/// Multiplexes sync messages to the correct library based on library_id
pub struct SyncMultiplexer {
	/// Map of library_id to sync protocol handler
	libraries: Arc<RwLock<HashMap<Uuid, Arc<SyncProtocolHandler>>>>,
	/// Device registry for node_id → device_id mapping
	device_registry: Arc<RwLock<DeviceRegistry>>,
}

impl SyncMultiplexer {
	/// Create a new sync multiplexer
	pub fn new(device_registry: Arc<RwLock<DeviceRegistry>>) -> Self {
		info!("Creating SyncMultiplexer for multi-library sync routing");
		Self {
			libraries: Arc::new(RwLock::new(HashMap::new())),
			device_registry,
		}
	}

	/// Register a library's sync handler
	pub async fn register_library(
		&self,
		library_id: Uuid,
		peer_sync: Arc<PeerSync>,
		backfill_manager: Arc<BackfillManager>,
	) {
		let mut handler = SyncProtocolHandler::new(library_id, self.device_registry.clone());
		handler.set_peer_sync(peer_sync);
		handler.set_backfill_manager(backfill_manager.clone());
		handler.set_metrics(backfill_manager.metrics().clone());

		let mut libraries = self.libraries.write().await;
		libraries.insert(library_id, Arc::new(handler));
		info!("Registered sync handler for library {}", library_id);
	}

	/// Unregister a library's sync handler
	pub async fn unregister_library(&self, library_id: Uuid) {
		let mut libraries = self.libraries.write().await;
		libraries.remove(&library_id);
		info!("Unregistered sync handler for library {}", library_id);
	}

	/// Handle sync message by routing to correct library
	async fn handle_sync_message(
		&self,
		from_device: Uuid,
		message: SyncMessage,
	) -> Result<Option<SyncMessage>> {
		let library_id = message.library_id();

		// Get handler for this library
		let libraries = self.libraries.read().await;
		let handler = libraries.get(&library_id).ok_or_else(|| {
			NetworkingError::Protocol(format!(
				"No sync handler for library {} (message from device {})",
				library_id, from_device
			))
		})?;

		// Delegate to the library's sync protocol handler
		handler.handle_sync_message(from_device, message).await
	}
}

#[async_trait]
impl crate::service::network::protocol::ProtocolHandler for SyncMultiplexer {
	fn protocol_name(&self) -> &str {
		"sync"
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: iroh::NodeId,
	) {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		// Map node_id to device_id
		let from_device = {
			let registry = self.device_registry.read().await;
			registry.get_device_by_node(remote_node_id)
		};

		let from_device = match from_device {
			Some(id) => id,
			None => {
				warn!(
					"SyncMultiplexer: Received stream from unknown node {}, closing",
					remote_node_id
				);
				return;
			}
		};

		// Read request with length prefix
		let mut len_buf = [0u8; 4];
		if let Err(e) = recv.read_exact(&mut len_buf).await {
			tracing::debug!(
				"SyncMultiplexer: Failed to read length (likely connection test): {}",
				e
			);
			return;
		}
		let req_len = u32::from_be_bytes(len_buf) as usize;

		let mut req_buf = vec![0u8; req_len];
		if let Err(e) = recv.read_exact(&mut req_buf).await {
			tracing::error!("SyncMultiplexer: Failed to read request: {}", e);
			return;
		}

		// Deserialize to get library_id
		let message: SyncMessage = match serde_json::from_slice(&req_buf) {
			Ok(msg) => msg,
			Err(e) => {
				tracing::error!("SyncMultiplexer: Failed to deserialize: {}", e);
				return;
			}
		};

		let library_id = message.library_id();
		tracing::info!(
			"SyncMultiplexer: Routing message for library {} from device {}",
			library_id,
			from_device
		);

		// Handle and get response
		let response_opt = match self.handle_sync_message(from_device, message).await {
			Ok(resp) => resp,
			Err(e) => {
				tracing::error!("SyncMultiplexer: Failed to handle message: {}", e);
				return;
			}
		};

		// Send response if needed
		if let Some(response) = response_opt {
			let resp_bytes = match serde_json::to_vec(&response) {
				Ok(bytes) => bytes,
				Err(e) => {
					tracing::error!("SyncMultiplexer: Failed to serialize response: {}", e);
					return;
				}
			};

			let len = resp_bytes.len() as u32;
			if let Err(e) = send.write_all(&len.to_be_bytes()).await {
				tracing::error!("SyncMultiplexer: Failed to send response length: {}", e);
				return;
			}
			if let Err(e) = send.write_all(&resp_bytes).await {
				tracing::error!("SyncMultiplexer: Failed to send response: {}", e);
				return;
			}
			let _ = send.flush().await;
		}
	}

	async fn handle_request(&self, from_device: Uuid, request: Vec<u8>) -> Result<Vec<u8>> {
		let message: SyncMessage =
			serde_json::from_slice(&request).map_err(|e| NetworkingError::Serialization(e))?;

		match self.handle_sync_message(from_device, message).await? {
			Some(response) => {
				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}
			None => Ok(Vec::new()),
		}
	}

	async fn handle_response(
		&self,
		from_device: Uuid,
		_from_node: iroh::NodeId,
		response: Vec<u8>,
	) -> Result<()> {
		if response.is_empty() {
			return Ok(());
		}

		let message: SyncMessage =
			serde_json::from_slice(&response).map_err(|e| NetworkingError::Serialization(e))?;

		// Process response message
		self.handle_sync_message(from_device, message).await?;

		Ok(())
	}

	async fn handle_event(&self, _event: ProtocolEvent) -> Result<()> {
		Ok(())
	}
}
