//! NetworkTransport implementation for NetworkingService
//!
//! Implements the sync layer's NetworkTransport trait, enabling PeerSync to send
//! sync messages over the network without circular dependencies.

use crate::{
	infra::sync::NetworkTransport,
	service::network::{protocol::sync::messages::SyncMessage, NetworkingError},
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::service::network::core::{NetworkingService, SYNC_ALPN};

/// Implementation of NetworkTransport for NetworkingService
///
/// This bridges the sync layer (which uses device UUIDs) with the network layer
/// (which uses Iroh NodeIds) by leveraging the DeviceRegistry for UUIDNodeId mapping.
#[async_trait::async_trait]
impl NetworkTransport for NetworkingService {
	/// Send a sync message to a target device
	///
	/// # Implementation Details
	///
	/// 1. Look up NodeId for device UUID via DeviceRegistry
	/// 2. Serialize the SyncMessage to JSON bytes
	/// 3. Send via Iroh endpoint using the sync protocol ALPN
	/// 4. Handle errors gracefully (device may be offline)
	async fn send_sync_message(&self, target_device: Uuid, message: SyncMessage) -> Result<()> {
		// 1. Look up NodeId for device UUID via public getter
		let device_registry_arc = self.device_registry();
		let node_id = {
			let registry = device_registry_arc.read().await;
			registry
				.get_node_id_for_device(target_device)
				.ok_or_else(|| {
					anyhow::anyhow!(
						"Device {} not found in registry (not paired or offline)",
						target_device
					)
				})?
		};

		tracing::info!(
			"Sending sync message to device {} (node {}), type: {:?}, library: {}",
			target_device,
			node_id,
			std::mem::discriminant(&message),
			message.library_id()
		);

		// 2. Serialize message to bytes
		let bytes = serde_json::to_vec(&message)
			.map_err(|e| anyhow::anyhow!("Failed to serialize sync message: {}", e))?;

		// 3. Get or create connection (with caching for massive performance improvement)
		let endpoint = self
			.endpoint()
			.ok_or_else(|| anyhow::anyhow!("Network endpoint not initialized"))?;

		let active_connections = self.active_connections();
		let cache_key = (node_id, SYNC_ALPN.to_vec());

		// Check cache first - reuse existing connection if alive
		let conn = {
			let connections = active_connections.read().await;
			if let Some(cached_conn) = connections.get(&cache_key) {
				if cached_conn.close_reason().is_none() {
					tracing::debug!(
						device_uuid = %target_device,
						"Reusing cached connection (avoids TLS handshake)"
					);
					Some(cached_conn.clone())
				} else {
					None // Connection closed, need new one
				}
			} else {
				None
			}
		};

		// Create new connection only if cache miss
		let conn = if let Some(conn) = conn {
			conn
		} else {
			tracing::debug!(
				device_uuid = %target_device,
				node_id = %node_id,
				"Creating new connection (cache miss)"
			);

			let new_conn = endpoint.connect(node_id, SYNC_ALPN).await.map_err(|e| {
				warn!(
					device_uuid = %target_device,
					node_id = %node_id,
					error = %e,
					"Failed to connect to device for sync"
				);
				anyhow::anyhow!("Failed to connect to {}: {}", target_device, e)
			})?;

			// Add to cache
			{
				let mut connections = active_connections.write().await;
				connections.insert(cache_key, new_conn.clone());
			}

			// Track outbound connection so we can receive incoming streams on it
			if let Some(cmd_sender) = self.command_sender() {
				use crate::service::network::core::event_loop::EventLoopCommand;
				let _ = cmd_sender.send(EventLoopCommand::TrackOutboundConnection {
					node_id,
					conn: new_conn.clone(),
				});
			}

			new_conn
		};

		// Open a unidirectional stream and send the message
		let mut send = conn
			.open_uni()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to open stream: {}", e))?;

		// Write length prefix (required by multiplexer)
		let len = bytes.len() as u32;
		send.write_all(&len.to_be_bytes())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to write length prefix: {}", e))?;

		// Write message bytes
		send.write_all(&bytes).await.map_err(|e| {
			warn!(
				device_uuid = %target_device,
				error = %e,
				"Failed to write sync message to stream"
			);
			anyhow::anyhow!("Failed to write message: {}", e)
		})?;

		send.finish()
			.map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

		tracing::info!(
			"Sync message sent successfully to device {} ({} bytes via uni stream)",
			target_device,
			bytes.len()
		);

		Ok(())
	}

	/// Send a sync request and wait for response
	///
	/// Uses bidirectional streams for proper request/response pattern (Iroh best practice)
	async fn send_sync_request(
		&self,
		target_device: Uuid,
		request: SyncMessage,
	) -> Result<SyncMessage> {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};
		use tokio::time::{timeout, Duration};

		// Look up NodeId for device UUID
		let device_registry_arc = self.device_registry();
		let node_id = {
			let registry = device_registry_arc.read().await;
			registry
				.get_node_id_for_device(target_device)
				.ok_or_else(|| {
					anyhow::anyhow!(
						"Device {} not found in registry (not paired or offline)",
						target_device
					)
				})?
		};

		debug!(
			device_uuid = %target_device,
			node_id = %node_id,
			message_type = ?std::mem::discriminant(&request),
			library_id = %request.library_id(),
			"Sending sync request"
		);

		// Get or create connection (with caching)
		let endpoint = self
			.endpoint()
			.ok_or_else(|| anyhow::anyhow!("Network endpoint not initialized"))?;

		let active_connections = self.active_connections();
		let cache_key = (node_id, SYNC_ALPN.to_vec());

		// Check cache first
		let conn = {
			let connections = active_connections.read().await;
			if let Some(cached_conn) = connections.get(&cache_key) {
				if cached_conn.close_reason().is_none() {
					tracing::debug!(
						device_uuid = %target_device,
						"Reusing cached connection for request"
					);
					Some(cached_conn.clone())
				} else {
					None
				}
			} else {
				None
			}
		};

		// Create if needed
		let conn = if let Some(conn) = conn {
			conn
		} else {
			tracing::debug!(
				device_uuid = %target_device,
				"Creating new connection for request"
			);

			let new_conn = endpoint.connect(node_id, SYNC_ALPN).await.map_err(|e| {
				warn!(
					device_uuid = %target_device,
					node_id = %node_id,
					error = %e,
					"Failed to connect to device for sync request"
				);
				anyhow::anyhow!("Failed to connect to {}: {}", target_device, e)
			})?;

			// Cache it
			{
				let mut connections = active_connections.write().await;
				connections.insert(cache_key, new_conn.clone());
			}

			// Track it
			if let Some(cmd_sender) = self.command_sender() {
				use crate::service::network::core::event_loop::EventLoopCommand;
				let _ = cmd_sender.send(EventLoopCommand::TrackOutboundConnection {
					node_id,
					conn: new_conn.clone(),
				});
			}

			new_conn
		};

		// Open bidirectional stream
		let (mut send, mut recv) = conn
			.open_bi()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to open bidirectional stream: {}", e))?;

		// Serialize and send request
		let req_bytes = serde_json::to_vec(&request)
			.map_err(|e| anyhow::anyhow!("Failed to serialize sync request: {}", e))?;

		let len = req_bytes.len() as u32;
		send.write_all(&len.to_be_bytes())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to send length: {}", e))?;
		send.write_all(&req_bytes)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to send request: {}", e))?;

		// Properly close send stream
		send.finish()
			.map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

		debug!("Sync request sent, waiting for response...");

		// Read response with timeout
		let result = timeout(Duration::from_secs(60), async {
			let mut len_buf = [0u8; 4];
			recv.read_exact(&mut len_buf)
				.await
				.map_err(|e| anyhow::anyhow!("Failed to read response length: {}", e))?;
			let resp_len = u32::from_be_bytes(len_buf) as usize;

			debug!("Receiving sync response of {} bytes", resp_len);

			let mut resp_buf = vec![0u8; resp_len];
			recv.read_exact(&mut resp_buf)
				.await
				.map_err(|e| anyhow::anyhow!("Failed to read response: {}", e))?;
			Ok::<_, anyhow::Error>(resp_buf)
		})
		.await;

		let resp_buf = match result {
			Ok(Ok(buf)) => buf,
			Ok(Err(e)) => return Err(e),
			Err(_) => {
				return Err(anyhow::anyhow!(
					"Sync request timed out after 60s - peer {} not responding",
					target_device
				))
			}
		};

		// Deserialize response
		let response: SyncMessage = serde_json::from_slice(&resp_buf)
			.map_err(|e| anyhow::anyhow!("Failed to deserialize sync response: {}", e))?;

		debug!(
			device_uuid = %target_device,
			response_type = ?std::mem::discriminant(&response),
			"Received sync response"
		);

		Ok(response)
	}

	/// Get list of currently connected sync partner devices FOR THIS LIBRARY
	///
	/// Returns device UUIDs that are:
	/// 1. Members of this specific library (in devices table)
	/// 2. Have sync_enabled=true in this library
	/// 3. Currently network-connected (according to Iroh)
	async fn get_connected_sync_partners(
		&self,
		library_id: Uuid,
		db: &sea_orm::DatabaseConnection,
	) -> Result<Vec<Uuid>> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// 1. Query devices table for THIS library with sync_enabled=true
		let library_devices = entities::device::Entity::find()
			.filter(entities::device::Column::SyncEnabled.eq(true))
			.all(db)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query library devices: {}", e))?;

		// 2. Get Iroh endpoint for checking connection state
		let endpoint = self
			.endpoint()
			.ok_or_else(|| anyhow::anyhow!("Network endpoint not initialized"))?;

		// 3. Get DeviceRegistry to check Iroh connection state
		let device_registry_arc = self.device_registry();
		let registry = device_registry_arc.read().await;

		// 4. Filter to only devices that Iroh reports as connected
		let sync_partners: Vec<Uuid> = library_devices
			.iter()
			.filter(|device| registry.is_node_connected(endpoint, device.uuid))
			.map(|device| device.uuid)
			.collect();

		// tracing::info!(
		// 	"Library-scoped sync partners: library={}, lib_devs={}, iroh_connected={}, partners={}",
		// 	library_id,
		// 	library_devices.len(),
		// 	sync_partners.len(),
		// 	sync_partners.len()
		// );

		Ok(sync_partners)
	}

	/// Check if a specific device is currently reachable
	///
	/// Returns true if:
	/// - Device UUID is mapped to a NodeId in DeviceRegistry
	/// - Iroh reports an active connection to the device
	async fn is_device_reachable(&self, device_uuid: Uuid) -> bool {
		let endpoint = match self.endpoint() {
			Some(ep) => ep,
			None => return false,
		};

		let device_registry_arc = self.device_registry();
		let registry = device_registry_arc.read().await;

		registry.is_node_connected(endpoint, device_uuid)
	}

	fn transport_name(&self) -> &'static str {
		"NetworkingService"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_transport_trait_implemented() {
		// This test just verifies that the trait is properly implemented
		// Actual functionality tests would require setting up Iroh endpoints
		// and device registries, which is better done as integration tests

		// Verify trait bound is satisfied
		fn assert_network_transport<T: NetworkTransport>() {}
		assert_network_transport::<NetworkingService>();
	}
}
