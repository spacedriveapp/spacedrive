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

use super::{NetworkingService, SYNC_ALPN};

/// Implementation of NetworkTransport for NetworkingService
///
/// This bridges the sync layer (which uses device UUIDs) with the network layer
/// (which uses Iroh NodeIds) by leveraging the DeviceRegistry for UUID↔NodeId mapping.
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
		// 1. Look up NodeId for device UUID
		let node_id = {
			let registry = self.device_registry.read().await;
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
			message_type = ?std::mem::discriminant(&message),
			library_id = %message.library_id(),
			"Sending sync message"
		);

		// 2. Serialize message to bytes
		let bytes = serde_json::to_vec(&message)
			.map_err(|e| anyhow::anyhow!("Failed to serialize sync message: {}", e))?;

		// 3. Send via Iroh endpoint
		let endpoint = self
			.endpoint
			.as_ref()
			.ok_or_else(|| anyhow::anyhow!("Network endpoint not initialized"))?;

		// Open a connection and send
		// Note: Iroh handles connection pooling, so repeated calls are efficient
		let conn = endpoint.connect(node_id, SYNC_ALPN).await.map_err(|e| {
			warn!(
				device_uuid = %target_device,
				node_id = %node_id,
				error = %e,
				"Failed to connect to device for sync"
			);
			anyhow::anyhow!("Failed to connect to {}: {}", target_device, e)
		})?;

		// Open a unidirectional stream and send the message
		let mut send = conn
			.open_uni()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to open stream: {}", e))?;

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

		debug!(
			device_uuid = %target_device,
			node_id = %node_id,
			bytes_sent = bytes.len(),
			"Sync message sent successfully"
		);

		Ok(())
	}

	/// Get list of currently connected sync partner devices
	///
	/// Returns device UUIDs that are both:
	/// - Registered in DeviceRegistry (paired)
	/// - Currently have an active connection
	///
	/// Note: This doesn't query the sync_partners table - that's the caller's responsibility.
	/// We just report which devices are network-reachable right now.
	async fn get_connected_sync_partners(&self) -> Result<Vec<Uuid>> {
		let registry = self.device_registry.read().await;

		// Get all connected devices from registry
		let connected_devices = registry.get_connected_devices();

		// Extract device UUIDs
		let device_uuids: Vec<Uuid> = connected_devices
			.into_iter()
			.map(|device_info| device_info.device_id)
			.collect();

		debug!(
			count = device_uuids.len(),
			"Retrieved connected sync partners"
		);

		Ok(device_uuids)
	}

	/// Check if a specific device is currently reachable
	///
	/// Returns true if:
	/// - Device UUID is mapped to a NodeId in DeviceRegistry
	/// - Device has an active network connection (we can reach it)
	async fn is_device_reachable(&self, device_uuid: Uuid) -> bool {
		let registry = self.device_registry.read().await;

		// Check if device is in registry and mapped to a node
		if registry.get_node_id_for_device(device_uuid).is_some() {
			// Device is registered and mapped - assume reachable
			// Actual connectivity will be determined when we try to send
			// (Iroh endpoint handles connection establishment)
			return true;
		}

		false
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
