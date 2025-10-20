//! Network transport abstraction for sync messages
//!
//! Provides a trait-based abstraction layer between the sync system and networking layer,
//! solving the circular dependency problem and enabling testability.

use anyhow::Result;
use uuid::Uuid;

// Import sync message from network protocol module
// (Network layer will implement this trait, but sync layer defines it)
use crate::service::network::protocol::sync::messages::SyncMessage;

/// Abstraction for sending sync messages over the network
///
/// This trait decouples the sync system from the networking implementation:
/// - Sync layer (PeerSync) depends on this trait
/// - Network layer (NetworkingService) implements this trait
/// - Breaks circular dependency: Library → SyncService → NetworkTransport ← NetworkingService
///
/// # Example
///
/// ```rust,ignore
/// // In PeerSync
/// async fn broadcast_state_change(&self, change: StateChangeMessage) {
///     let partners = self.get_sync_partners().await?;
///
///     for partner_uuid in partners {
///         // NetworkTransport handles UUID→NodeId mapping internally
///         self.network.send_sync_message(partner_uuid, message.clone()).await?;
///     }
/// }
/// ```
///
/// # Implementation Notes
///
/// The implementer (NetworkingService) must:
/// 1. Map device UUID to network NodeId using DeviceRegistry
/// 2. Serialize the SyncMessage
/// 3. Send via Iroh endpoint
/// 4. Handle connection errors gracefully (devices may be offline)
#[async_trait::async_trait]
pub trait NetworkTransport: Send + Sync {
	/// Send a sync message to a specific device
	///
	/// # Arguments
	///
	/// * `target_device` - UUID of the target device (from devices table where sync_enabled=true)
	/// * `message` - The sync message to send (StateChange, SharedChange, etc.)
	///
	/// # Returns
	///
	/// - `Ok(())` if message was sent successfully
	/// - `Err(...)` if:
	///   - Target device UUID is not mapped to a NodeId (device not paired/connected)
	///   - Network send fails (connection error, device offline)
	///   - Serialization fails
	///
	/// # Implementation
	///
	/// The implementer should:
	/// 1. Look up NodeId via `device_registry.get_node_id_for_device(target_device)`
	/// 2. If NodeId not found, return error (device not connected)
	/// 3. Serialize message to bytes
	/// 4. Send via `endpoint.send_message(node_id, "sync", bytes)`
	///
	/// # Error Handling
	///
	/// Callers should handle errors gracefully - devices may go offline mid-broadcast.
	/// Consider logging warnings rather than failing the entire operation.
	async fn send_sync_message(&self, target_device: Uuid, message: SyncMessage) -> Result<()>;

	/// Send a sync request and wait for response (request/response pattern)
	///
	/// Use for requests that expect responses: StateRequest, SharedChangeRequest, etc.
	/// Uses bidirectional streams to receive the response.
	///
	/// # Arguments
	///
	/// * `target_device` - UUID of the target device
	/// * `request` - The sync request message
	///
	/// # Returns
	///
	/// The response message from the peer
	///
	/// # Errors
	///
	/// Returns error if:
	/// - Device is not reachable
	/// - Network transport fails
	/// - Response timeout (60s)
	/// - Response is malformed
	async fn send_sync_request(
		&self,
		target_device: Uuid,
		request: SyncMessage,
	) -> Result<SyncMessage>;

	/// Get list of currently connected sync partner devices
	///
	/// Returns UUIDs of devices that are:
	/// - Listed in devices table with sync_enabled=true
	/// - Currently connected (have active network connection)
	///
	/// This is used to optimize broadcasting - only send to devices that can receive.
	///
	/// # Returns
	///
	/// Vector of device UUIDs that are currently reachable for sync messages.
	/// Empty vector if no sync partners are connected.
	///
	/// # Implementation Note
	///
	/// This should query:
	/// 1. `devices` table for sync_enabled=true devices
	/// 2. `device_registry` for connection status
	/// 3. Return intersection of (enabled) AND (connected)
	async fn get_connected_sync_partners(&self) -> Result<Vec<Uuid>>;

	/// Check if a specific device is currently reachable
	///
	/// Useful before attempting to send, to avoid unnecessary errors.
	///
	/// # Arguments
	///
	/// * `device_uuid` - UUID of the device to check
	///
	/// # Returns
	///
	/// `true` if:
	/// - Device is mapped to a NodeId in DeviceRegistry
	/// - Device has an active network connection
	///
	/// `false` otherwise (device offline, not paired, etc.)
	async fn is_device_reachable(&self, device_uuid: Uuid) -> bool {
		// Default implementation: can be overridden for more efficient checks
		false
	}

	/// Get transport name for debugging
	fn transport_name(&self) -> &'static str {
		"UnknownTransport"
	}
}

/// Mock implementation for testing - collects messages without sending
#[cfg(test)]
pub struct MockNetworkTransport {
	/// Track which devices received which messages
	pub sent_messages: std::sync::Arc<std::sync::Mutex<Vec<(Uuid, SyncMessage)>>>,
}

#[cfg(test)]
impl MockNetworkTransport {
	pub fn new() -> Self {
		Self {
			sent_messages: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
		}
	}

	pub fn get_sent_messages(&self) -> Vec<(Uuid, SyncMessage)> {
		self.sent_messages.lock().unwrap().clone()
	}
}

// NOTE: This isn't actually used, I think
#[cfg(test)]
#[async_trait::async_trait]
impl NetworkTransport for MockNetworkTransport {
	async fn send_sync_message(&self, target_device: Uuid, message: SyncMessage) -> Result<()> {
		self.sent_messages
			.lock()
			.unwrap()
			.push((target_device, message));
		Ok(())
	}

	async fn send_sync_request(
		&self,
		target_device: Uuid,
		request: SyncMessage,
	) -> Result<SyncMessage> {
		// Mock implementation: record the request and return a mock response
		self.sent_messages
			.lock()
			.unwrap()
			.push((target_device, request.clone()));

		// Return appropriate mock response based on request type
		match request {
			SyncMessage::StateRequest { library_id, .. } => Ok(SyncMessage::StateResponse {
				library_id,
				model_type: "device".to_string(),
				device_id: target_device,
				records: vec![],
				checkpoint: None,
				has_more: false,
			}),
			SyncMessage::SharedChangeRequest { library_id, .. } => {
				Ok(SyncMessage::SharedChangeResponse {
					library_id,
					entries: vec![],
					current_state: None,
					has_more: false,
				})
			}
			_ => Err(anyhow::anyhow!("Mock: unexpected request type")),
		}
	}

	async fn get_connected_sync_partners(&self) -> Result<Vec<Uuid>> {
		// For tests, return empty list
		Ok(vec![])
	}

	fn transport_name(&self) -> &'static str {
		"MockNetworkTransport"
	}
}
