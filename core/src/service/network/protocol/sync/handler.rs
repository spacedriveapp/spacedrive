//! Sync protocol handler
//!
//! Handles push-based sync communication between leader and follower devices.

use super::messages::SyncMessage;
use crate::infra::sync::{SyncLogDb, SyncLogError, SyncRole};
use crate::service::network::{
	device::registry::DeviceRegistry, protocol::ProtocolEvent, protocol::ProtocolHandler,
	NetworkingError, Result,
};
use async_trait::async_trait;
use iroh::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB max message

/// Sync protocol handler
///
/// Manages sync communication between leader and follower devices
/// for a specific library.
pub struct SyncProtocolHandler {
	/// Library this handler is for
	library_id: Uuid,

	/// Sync log database
	sync_log_db: Arc<SyncLogDb>,

	/// Device registry for connection management
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// This device's role in the library (Leader or Follower)
	role: Arc<RwLock<SyncRole>>,

	/// Connected followers (leader only) - maps device_id to last known sequence
	followers: Arc<RwLock<HashMap<Uuid, u64>>>,
}

impl SyncProtocolHandler {
	/// Create a new sync protocol handler
	pub fn new(
		library_id: Uuid,
		sync_log_db: Arc<SyncLogDb>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		initial_role: SyncRole,
	) -> Self {
		Self {
			library_id,
			sync_log_db,
			device_registry,
			role: Arc::new(RwLock::new(initial_role)),
			followers: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Get the current role
	pub async fn role(&self) -> SyncRole {
		*self.role.read().await
	}

	/// Update the role (called when leadership changes)
	pub async fn set_role(&self, new_role: SyncRole) {
		let mut role = self.role.write().await;
		*role = new_role;
		info!(
			library_id = %self.library_id,
			role = ?new_role,
			"Sync role updated"
		);
	}

	/// Leader: Notify all followers of new entries
	///
	/// Called by the leader when new sync log entries are created.
	pub async fn notify_followers(
		&self,
		from_sequence: u64,
		to_sequence: u64,
	) -> Result<Vec<Uuid>> {
		// Verify we're the leader
		if *self.role.read().await != SyncRole::Leader {
			return Err(NetworkingError::Protocol(
				"Only leader can notify followers".to_string(),
			));
		}

		let message = SyncMessage::NewEntries {
			library_id: self.library_id,
			from_sequence,
			to_sequence,
			entry_count: (to_sequence - from_sequence + 1) as usize,
		};

		let payload =
			serde_json::to_vec(&message).map_err(|e| NetworkingError::Serialization(e))?;

		// Get all follower devices
		let followers = self.followers.read().await;
		let follower_ids: Vec<Uuid> = followers.keys().copied().collect();

		debug!(
			library_id = %self.library_id,
			from_seq = from_sequence,
			to_seq = to_sequence,
			follower_count = follower_ids.len(),
			"Notifying followers of new entries"
		);

		// Send to all followers (in parallel in production)
		// For now, just return the list
		Ok(follower_ids)
	}

	/// Follower: Request entries from leader
	///
	/// Called by follower to fetch sync log entries.
	pub async fn request_entries(
		&self,
		leader_device_id: Uuid,
		since_sequence: u64,
		limit: usize,
	) -> Result<Vec<crate::infra::sync::SyncLogEntry>> {
		let message = SyncMessage::FetchEntries {
			library_id: self.library_id,
			since_sequence,
			limit: limit.min(1000), // Cap at 1000
		};

		// In a real implementation, this would send via networking service
		// For now, return empty (networking integration in Phase 2.5)
		warn!("request_entries not fully implemented yet - networking integration pending");
		Ok(Vec::new())
	}

	/// Register a follower device (leader only)
	pub async fn register_follower(&self, device_id: Uuid, current_sequence: u64) {
		let mut followers = self.followers.write().await;
		followers.insert(device_id, current_sequence);
		info!(
			library_id = %self.library_id,
			device_id = %device_id,
			sequence = current_sequence,
			"Registered follower device"
		);
	}

	/// Update follower's last known sequence (leader only)
	pub async fn update_follower_sequence(&self, device_id: Uuid, sequence: u64) {
		let mut followers = self.followers.write().await;
		if let Some(last_seq) = followers.get_mut(&device_id) {
			*last_seq = sequence;
		}
	}

	/// Handle incoming sync message
	async fn handle_message(
		&self,
		message: SyncMessage,
		stream: &mut (impl AsyncWrite + Unpin),
		from_device: Uuid,
	) -> Result<()> {
		match message {
			SyncMessage::NewEntries {
				from_sequence,
				to_sequence,
				entry_count,
				..
			} => {
				self.handle_new_entries(from_device, from_sequence, to_sequence, entry_count)
					.await?;
				Ok(())
			}

			SyncMessage::FetchEntries {
				since_sequence,
				limit,
				..
			} => {
				let response = self
					.handle_fetch_entries(from_device, since_sequence, limit)
					.await?;
				let payload =
					serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))?;

				// Write response
				stream
					.write_u32(payload.len() as u32)
					.await
					.map_err(NetworkingError::Io)?;
				stream
					.write_all(&payload)
					.await
					.map_err(NetworkingError::Io)?;
				stream.flush().await.map_err(NetworkingError::Io)?;

				Ok(())
			}

			SyncMessage::EntriesResponse { entries, .. } => {
				self.handle_entries_response(from_device, entries).await?;
				Ok(())
			}

			SyncMessage::Acknowledge {
				up_to_sequence,
				applied_count,
				..
			} => {
				self.handle_acknowledge(from_device, up_to_sequence, applied_count)
					.await?;
				Ok(())
			}

			SyncMessage::Heartbeat {
				current_sequence,
				role,
				..
			} => {
				self.handle_heartbeat(from_device, current_sequence, role)
					.await?;
				Ok(())
			}

			SyncMessage::SyncRequired { reason, .. } => {
				warn!(
					library_id = %self.library_id,
					reason = %reason,
					"Leader says full sync required"
				);
				Ok(())
			}

			SyncMessage::Error { message, .. } => {
				error!(
					library_id = %self.library_id,
					from_device = %from_device,
					error = %message,
					"Received sync error"
				);
				Ok(())
			}
		}
	}

	/// Handle NewEntries notification (follower only)
	async fn handle_new_entries(
		&self,
		from_device: Uuid,
		from_sequence: u64,
		to_sequence: u64,
		entry_count: usize,
	) -> Result<()> {
		if *self.role.read().await != SyncRole::Follower {
			debug!("Ignoring NewEntries notification (not a follower)");
			return Ok(());
		}

		info!(
			library_id = %self.library_id,
			from_device = %from_device,
			from_seq = from_sequence,
			to_seq = to_sequence,
			count = entry_count,
			"Received new entries notification"
		);

		// TODO: Queue a fetch request
		// This will be implemented when we add the sync service
		Ok(())
	}

	/// Handle FetchEntries request (leader only)
	async fn handle_fetch_entries(
		&self,
		from_device: Uuid,
		since_sequence: u64,
		limit: usize,
	) -> Result<SyncMessage> {
		if *self.role.read().await != SyncRole::Leader {
			return Ok(SyncMessage::Error {
				library_id: self.library_id,
				message: "This device is not the leader".to_string(),
			});
		}

		debug!(
			library_id = %self.library_id,
			from_device = %from_device,
			since_seq = since_sequence,
			limit = limit,
			"Fetching entries for follower"
		);

		// Fetch entries from sync log
		let entries = self
			.sync_log_db
			.fetch_since(since_sequence, Some(limit.min(1000)))
			.await
			.map_err(|e| {
				NetworkingError::Protocol(format!("Failed to fetch sync entries: {}", e))
			})?;

		let latest_sequence = self.sync_log_db.latest_sequence().await.map_err(|e| {
			NetworkingError::Protocol(format!("Failed to get latest sequence: {}", e))
		})?;

		let has_more = entries.len() >= limit && latest_sequence > since_sequence + limit as u64;

		Ok(SyncMessage::EntriesResponse {
			library_id: self.library_id,
			entries,
			latest_sequence,
			has_more,
		})
	}

	/// Handle EntriesResponse (follower only)
	async fn handle_entries_response(
		&self,
		from_device: Uuid,
		entries: Vec<crate::infra::sync::SyncLogEntry>,
	) -> Result<()> {
		if *self.role.read().await != SyncRole::Follower {
			debug!("Ignoring EntriesResponse (not a follower)");
			return Ok(());
		}

		info!(
			library_id = %self.library_id,
			from_device = %from_device,
			entry_count = entries.len(),
			"Received entries from leader"
		);

		// TODO: Apply entries (will be implemented in sync service)
		// For now, just log that we received them
		Ok(())
	}

	/// Handle Acknowledge from follower (leader only)
	async fn handle_acknowledge(
		&self,
		from_device: Uuid,
		up_to_sequence: u64,
		applied_count: usize,
	) -> Result<()> {
		if *self.role.read().await != SyncRole::Leader {
			return Ok(());
		}

		debug!(
			library_id = %self.library_id,
			from_device = %from_device,
			sequence = up_to_sequence,
			count = applied_count,
			"Follower acknowledged sync"
		);

		// Update follower's position
		self.update_follower_sequence(from_device, up_to_sequence)
			.await;

		Ok(())
	}

	/// Handle Heartbeat
	async fn handle_heartbeat(
		&self,
		from_device: Uuid,
		current_sequence: u64,
		remote_role: SyncRole,
	) -> Result<()> {
		debug!(
			library_id = %self.library_id,
			from_device = %from_device,
			sequence = current_sequence,
			role = ?remote_role,
			"Received heartbeat"
		);

		// Update follower's position if we're the leader
		if *self.role.read().await == SyncRole::Leader && remote_role == SyncRole::Follower {
			self.update_follower_sequence(from_device, current_sequence)
				.await;
		}

		Ok(())
	}

	/// Read a message from a stream
	async fn read_message(&self, stream: &mut (impl AsyncRead + Unpin)) -> Result<SyncMessage> {
		// Read message length (4 bytes)
		let len = stream.read_u32().await.map_err(NetworkingError::Io)?;

		if len as usize > MAX_MESSAGE_SIZE {
			return Err(NetworkingError::Protocol(format!(
				"Message too large: {} bytes",
				len
			)));
		}

		// Read message payload
		let mut buffer = vec![0u8; len as usize];
		stream
			.read_exact(&mut buffer)
			.await
			.map_err(NetworkingError::Io)?;

		// Deserialize message
		serde_json::from_slice(&buffer).map_err(|e| NetworkingError::Serialization(e))
	}

	/// Write a message to a stream
	async fn write_message(
		&self,
		stream: &mut (impl AsyncWrite + Unpin),
		message: &SyncMessage,
	) -> Result<()> {
		let payload = serde_json::to_vec(message).map_err(|e| NetworkingError::Serialization(e))?;

		// Write message length
		stream
			.write_u32(payload.len() as u32)
			.await
			.map_err(NetworkingError::Io)?;

		// Write message payload
		stream
			.write_all(&payload)
			.await
			.map_err(NetworkingError::Io)?;

		stream.flush().await.map_err(NetworkingError::Io)?;

		Ok(())
	}
}

#[async_trait]
impl ProtocolHandler for SyncProtocolHandler {
	fn protocol_name(&self) -> &str {
		"sync"
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn AsyncRead + Send + Unpin>,
		remote_node_id: NodeId,
	) {
		// Look up device ID from node ID
		let device_id = {
			let registry = self.device_registry.read().await;
			// For now, use a placeholder until DeviceRegistry has node_id lookup
			// TODO: Add get_device_by_node_id to DeviceRegistry
			match registry.get_paired_devices().first() {
				Some(device) => device.device_id,
				None => {
					warn!(
						node_id = ?remote_node_id,
						"No paired devices, cannot handle sync stream"
					);
					return;
				}
			}
		};

		info!(
			library_id = %self.library_id,
			device_id = %device_id,
			"Handling sync protocol stream"
		);

		// Handle multiple messages on this stream
		loop {
			match self.read_message(&mut recv).await {
				Ok(message) => {
					debug!(
						library_id = %self.library_id,
						device_id = %device_id,
						message_type = ?message,
						"Received sync message"
					);

					if let Err(e) = self.handle_message(message, &mut send, device_id).await {
						error!(
							library_id = %self.library_id,
							device_id = %device_id,
							error = %e,
							"Error handling sync message"
						);

						// Send error response
						let error_msg = SyncMessage::Error {
							library_id: self.library_id,
							message: e.to_string(),
						};
						let _ = self.write_message(&mut send, &error_msg).await;
						break;
					}
				}
				Err(e) => {
					// Connection closed or error
					debug!(
						library_id = %self.library_id,
						device_id = %device_id,
						error = %e,
						"Sync stream ended"
					);
					break;
				}
			}
		}

		info!(
			library_id = %self.library_id,
			device_id = %device_id,
			"Sync stream closed"
		);
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		// Deserialize request
		let message: SyncMessage =
			serde_json::from_slice(&request_data).map_err(|e| NetworkingError::Serialization(e))?;

		debug!(
			library_id = %self.library_id,
			from_device = %from_device,
			message_type = ?message,
			"Handling sync request"
		);

		// Handle the message and generate response
		match message {
			SyncMessage::FetchEntries {
				since_sequence,
				limit,
				..
			} => {
				let response = self
					.handle_fetch_entries(from_device, since_sequence, limit)
					.await?;
				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}
			SyncMessage::Heartbeat {
				current_sequence,
				role,
				..
			} => {
				self.handle_heartbeat(from_device, current_sequence, role)
					.await?;
				// Return heartbeat response
				let response = SyncMessage::Heartbeat {
					library_id: self.library_id,
					current_sequence: self.sync_log_db.latest_sequence().await.unwrap_or(0),
					role: *self.role.read().await,
					timestamp: chrono::Utc::now(),
				};
				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}
			_ => {
				// For notifications, just return empty response
				Ok(Vec::new())
			}
		}
	}

	async fn handle_response(
		&self,
		from_device: Uuid,
		_from_node: NodeId,
		response_data: Vec<u8>,
	) -> Result<()> {
		// Deserialize response
		let message: SyncMessage = serde_json::from_slice(&response_data)
			.map_err(|e| NetworkingError::Serialization(e))?;

		debug!(
			library_id = %self.library_id,
			from_device = %from_device,
			message_type = ?message,
			"Handling sync response"
		);

		// Handle response messages (EntriesResponse, etc.)
		match message {
			SyncMessage::EntriesResponse { entries, .. } => {
				self.handle_entries_response(from_device, entries).await
			}
			_ => Ok(()),
		}
	}

	async fn handle_event(&self, event: ProtocolEvent) -> Result<()> {
		match event {
			ProtocolEvent::DeviceConnected { device_id } => {
				info!(
					library_id = %self.library_id,
					device_id = %device_id,
					"Device connected to sync protocol"
				);

				// If we're the leader, register this as a potential follower
				if *self.role.read().await == SyncRole::Leader {
					self.register_follower(device_id, 0).await;
				}
			}
			ProtocolEvent::DeviceDisconnected { device_id } => {
				info!(
					library_id = %self.library_id,
					device_id = %device_id,
					"Device disconnected from sync protocol"
				);

				// Remove from followers list
				self.followers.write().await.remove(&device_id);
			}
			_ => {}
		}
		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::sync::SyncLogDb;
	use crate::service::network::{utils::logging::SilentLogger, DeviceRegistry};
	use tempfile::tempdir;

	#[tokio::test]
	async fn test_protocol_handler_creation() {
		let temp_dir = tempdir().unwrap();
		let library_id = Uuid::new_v4();

		let sync_log_db = Arc::new(SyncLogDb::open(library_id, temp_dir.path()).await.unwrap());

		// Create minimal DeviceRegistry for testing
		let device_manager = Arc::new(
			crate::device::DeviceManager::init_with_path_and_name(
				&temp_dir.path().to_path_buf(),
				Some("TestDevice".to_string()),
			)
			.unwrap(),
		);
		let logger = Arc::new(SilentLogger);
		let registry = DeviceRegistry::new(device_manager, temp_dir.path(), logger).unwrap();
		let device_registry = Arc::new(RwLock::new(registry));

		let handler =
			SyncProtocolHandler::new(library_id, sync_log_db, device_registry, SyncRole::Leader);

		assert_eq!(handler.protocol_name(), "sync");
		assert_eq!(handler.role().await, SyncRole::Leader);
	}

	#[tokio::test]
	async fn test_role_change() {
		let temp_dir = tempdir().unwrap();
		let library_id = Uuid::new_v4();

		let sync_log_db = Arc::new(SyncLogDb::open(library_id, temp_dir.path()).await.unwrap());

		let device_manager = Arc::new(
			crate::device::DeviceManager::init_with_path_and_name(
				&temp_dir.path().to_path_buf(),
				Some("TestDevice".to_string()),
			)
			.unwrap(),
		);
		let logger = Arc::new(SilentLogger);
		let registry = DeviceRegistry::new(device_manager, temp_dir.path(), logger).unwrap();
		let device_registry = Arc::new(RwLock::new(registry));

		let handler =
			SyncProtocolHandler::new(library_id, sync_log_db, device_registry, SyncRole::Follower);

		assert_eq!(handler.role().await, SyncRole::Follower);

		handler.set_role(SyncRole::Leader).await;
		assert_eq!(handler.role().await, SyncRole::Leader);
	}
}
