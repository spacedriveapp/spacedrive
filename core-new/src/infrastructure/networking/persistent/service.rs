//! Networking service with protocol handler system
//!
//! Provides the main service interface for persistent device connections,
//! integrating with the core Spacedrive system and routing messages to
//! appropriate protocol handlers.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use super::{
	identity::SessionKeys,
	manager::{NetworkEvent, PersistentConnectionManager},
	messages::DeviceMessage,
	pairing_bridge::{PairingBridge, PairingSession, PairingStatus},
};
use crate::device::DeviceManager;
use crate::networking::{DeviceInfo, NetworkError, Result};

/// Trait for handling specific protocol messages
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
	/// Handle incoming message from a device
	async fn handle_message(
		&self,
		device_id: Uuid,
		message: DeviceMessage,
	) -> Result<Option<DeviceMessage>>;

	/// Get protocol name for registration
	fn protocol_name(&self) -> &str;

	/// Get supported message types
	fn supported_messages(&self) -> Vec<&str>;
}

/// Lightweight reference to core networking service components (cloneable)
#[derive(Clone)]
pub struct NetworkingServiceRef {
	/// Persistent connection manager
	connection_manager: Arc<RwLock<PersistentConnectionManager>>,
	/// Device manager reference
	device_manager: Arc<DeviceManager>,
}

impl NetworkingServiceRef {
	/// Add a paired device to the network
	pub async fn add_paired_device(
		&self,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
	) -> Result<()> {
		let mut manager = self.connection_manager.write().await;
		manager.add_paired_device(device_info, session_keys).await
	}
}

/// Integration with the core Spacedrive system
pub struct NetworkingService {
	/// Persistent connection manager
	connection_manager: Arc<RwLock<PersistentConnectionManager>>,

	/// Event receiver for network events
	event_receiver: mpsc::UnboundedReceiver<NetworkEvent>,

	/// Event sender for network events (clone for spawning tasks)
	event_sender: mpsc::UnboundedSender<NetworkEvent>,

	/// Protocol handlers for different data types
	protocol_handlers: HashMap<String, Arc<dyn ProtocolHandler>>,

	/// Device manager reference
	device_manager: Arc<DeviceManager>,

	/// Pairing bridge for device pairing operations
	pairing_bridge: Option<Arc<PairingBridge>>,

	/// Service state
	is_running: bool,
}

/// Database sync handler for real-time library synchronization
pub struct DatabaseSyncHandler {
	// TODO: Add database reference when available
	// database: Arc<Database>,
}

/// File transfer handler for efficient file streaming
pub struct FileTransferHandler {
	// TODO: Add file operations reference when available
	// file_ops: Arc<FileOperations>,
}

/// Spacedrop handler for peer-to-peer file sharing
pub struct SpacedropHandler {
	// TODO: Add spacedrop operations when available
	// spacedrop_ops: Arc<SpacedropOperations>,
}

/// Real-time sync handler for live updates
pub struct RealtimeSyncHandler {
	// TODO: Add real-time sync operations when available
	// realtime_ops: Arc<RealtimeOperations>,
}

impl NetworkingService {
	/// Initialize networking service
	pub async fn new(device_manager: Arc<DeviceManager>, password: &str) -> Result<Self> {
		let connection_manager =
			PersistentConnectionManager::new(&device_manager, password).await?;

		let connection_manager = Arc::new(RwLock::new(connection_manager));
		let (event_sender, event_receiver) = mpsc::unbounded_channel();

		Ok(Self {
			connection_manager,
			event_receiver,
			event_sender,
			protocol_handlers: HashMap::new(),
			device_manager,
			pairing_bridge: None, // Will be initialized when networking is started
			is_running: false,
		})
	}

	/// Register handlers for different protocols
	pub fn register_protocol_handler(&mut self, handler: Arc<dyn ProtocolHandler>) {
		let protocol_name = handler.protocol_name().to_string();
		tracing::info!("Registering protocol handler: {}", protocol_name);
		self.protocol_handlers.insert(protocol_name, handler);
	}

	/// Start the networking service
	pub async fn start(&mut self) -> Result<()> {
		if self.is_running {
			return Ok(());
		}

		self.is_running = true;

		// Register default protocol handlers
		self.register_default_handlers().await?;

		// Initialize connection manager without blocking
		// The actual event loop will start when events are processed

		Ok(())
	}

	/// Start event processing (call this after start() to begin processing events in background)
	pub async fn start_event_processing(&mut self) -> Result<()> {
		// Process network events in event loop
		self.process_events().await
	}

	/// Register default protocol handlers
	async fn register_default_handlers(&mut self) -> Result<()> {
		// Database sync handler is disabled until database sync messages are uncommented
		// let db_handler = Arc::new(DatabaseSyncHandler::new());
		// self.register_protocol_handler(db_handler);

		// Register file transfer handler
		let file_handler = Arc::new(FileTransferHandler::new());
		self.register_protocol_handler(file_handler);

		// Register Spacedrop handler
		let spacedrop_handler = Arc::new(SpacedropHandler::new());
		self.register_protocol_handler(spacedrop_handler);

		// Register real-time sync handler
		let realtime_handler = Arc::new(RealtimeSyncHandler::new());
		self.register_protocol_handler(realtime_handler);

		tracing::info!(
			"Registered {} default protocol handlers",
			self.protocol_handlers.len()
		);
		Ok(())
	}

	/// Process network events and integrate with core
	async fn process_events(&mut self) -> Result<()> {
		while let Some(event) = self.event_receiver.recv().await {
			match event {
				NetworkEvent::DeviceConnected { device_id } => {
					tracing::info!("Device connected: {}", device_id);
					// Notify other services that device is available
					// Could trigger sync, file sharing, etc.
				}

				NetworkEvent::DeviceDisconnected { device_id } => {
					tracing::info!("Device disconnected: {}", device_id);
					// Handle graceful disconnect
				}

				NetworkEvent::DevicePaired {
					device_id,
					device_info,
				} => {
					tracing::info!(
						"New device paired: {} ({})",
						device_info.device_name,
						device_id
					);
					// Could trigger initial sync, welcome message, etc.
				}

				NetworkEvent::MessageReceived { device_id, message } => {
					// Route message to appropriate handler
					if let Err(e) = self.handle_device_message(device_id, message).await {
						tracing::error!(
							"Failed to handle message from device {}: {}",
							device_id,
							e
						);
					}
				}

				NetworkEvent::ConnectionError { device_id, error } => {
					tracing::error!("Connection error for {:?}: {}", device_id, error);
					// Could trigger retry logic, user notification
				}

				NetworkEvent::ConnectionAttempt { device_id, attempt } => {
					tracing::debug!("Connection attempt {} for device {}", attempt, device_id);
				}

				NetworkEvent::RetryScheduled {
					device_id,
					retry_at,
				} => {
					tracing::debug!("Retry scheduled for device {} at {}", device_id, retry_at);
				}

				NetworkEvent::DeviceRevoked { device_id } => {
					tracing::info!("Device revoked: {}", device_id);
					// Handle device revocation cleanup
				}
			}
		}

		Ok(())
	}

	/// Route incoming message to appropriate protocol handler
	async fn handle_device_message(&self, device_id: Uuid, message: DeviceMessage) -> Result<()> {
		let message_type = message.message_type();

		// Find appropriate handler based on message type
		let handler = match message_type {
			// Database sync messages are currently commented out in messages.rs
			// "database_sync" | "database_sync_response" => {
			//     self.protocol_handlers.get("database_sync")
			// }
			"file_transfer_request"
			| "file_transfer_response"
			| "file_chunk"
			| "file_chunk_ack"
			| "file_transfer_complete"
			| "file_transfer_cancel" => self.protocol_handlers.get("file_transfer"),
			"spacedrop_request" | "spacedrop_response" | "spacedrop_progress" => {
				self.protocol_handlers.get("spacedrop")
			}
			"location_update" | "indexer_progress" | "fs_event" => {
				self.protocol_handlers.get("realtime_sync")
			}
			_ => {
				// Try to handle with custom protocol handler
				if let DeviceMessage::Custom { protocol, .. } = &message {
					self.protocol_handlers.get(protocol)
				} else {
					None
				}
			}
		};

		if let Some(handler) = handler {
			// Handle message and get optional response
			match handler.handle_message(device_id, message).await {
				Ok(Some(response)) => {
					// Send response back to device
					self.send_to_device(device_id, response).await?;
				}
				Ok(None) => {
					// No response needed
				}
				Err(e) => {
					tracing::error!("Handler failed for message type {}: {}", message_type, e);

					// Send error response
					let error_msg = DeviceMessage::Error {
						request_id: None,
						error_code: "HANDLER_ERROR".to_string(),
						message: format!("Failed to handle {}: {}", message_type, e),
						details: None,
					};
					self.send_to_device(device_id, error_msg).await.ok();
				}
			}
		} else {
			tracing::warn!("No handler found for message type: {}", message_type);

			// Send error response for unknown message type
			let error_msg = DeviceMessage::Error {
				request_id: None,
				error_code: "UNKNOWN_MESSAGE_TYPE".to_string(),
				message: format!("No handler for message type: {}", message_type),
				details: None,
			};
			self.send_to_device(device_id, error_msg).await.ok();
		}

		Ok(())
	}

	// High-level API for database sync (disabled until database sync messages are implemented)
	// pub async fn send_database_sync(
	//     &self,
	//     device_id: Uuid,
	//     library_id: Uuid,
	//     operation: SyncOperation,
	// ) -> Result<()> {
	//     let message = DeviceMessage::DatabaseSync {
	//         library_id,
	//         operation,
	//         data: vec![], // Actual data would be serialized here
	//         timestamp: chrono::Utc::now(),
	//     };
	//
	//     self.send_to_device(device_id, message).await
	// }

	/// High-level API for file transfers
	pub async fn initiate_file_transfer(
		&self,
		device_id: Uuid,
		file_path: &str,
		file_size: u64,
	) -> Result<Uuid> {
		let transfer_id = Uuid::new_v4();
		let message = DeviceMessage::FileTransferRequest {
			transfer_id,
			file_path: file_path.to_string(),
			file_size,
			checksum: None, // Would be computed elsewhere
			metadata: super::messages::FileMetadata {
				name: file_path.split('/').last().unwrap_or("unknown").to_string(),
				size: file_size,
				mime_type: None,
				modified_at: None,
				created_at: None,
				is_directory: false,
				permissions: None,
				checksum: None,
				extended_attributes: HashMap::new(),
			},
		};

		self.send_to_device(device_id, message).await?;
		Ok(transfer_id)
	}

	/// High-level API for Spacedrop
	pub async fn send_spacedrop_request(
		&self,
		device_id: Uuid,
		file_metadata: super::messages::FileMetadata,
		sender_name: String,
		message: Option<String>,
	) -> Result<Uuid> {
		let transfer_id = Uuid::new_v4();
		let spacedrop_msg = DeviceMessage::SpacedropRequest {
			transfer_id,
			file_metadata,
			sender_name,
			message,
		};

		self.send_to_device(device_id, spacedrop_msg).await?;
		Ok(transfer_id)
	}

	/// Send message to specific device
	pub async fn send_to_device(&self, device_id: Uuid, message: DeviceMessage) -> Result<()> {
		let mut manager = self.connection_manager.write().await;

		tracing::debug!(
			"Sending {} message to device {}",
			message.message_type(),
			device_id
		);

		// Use the manager's send_to_device method which handles the connection properly
		manager.send_to_device(device_id, message).await
			.map_err(|e| match e {
				crate::networking::NetworkError::DeviceNotFound(id) => {
					crate::networking::NetworkError::DeviceNotConnected(id)
				}
				other => other,
			})?;
		
		tracing::info!("Message sent successfully to device {}", device_id);
		Ok(())
	}

	/// Get list of connected devices
	pub async fn get_connected_devices(&self) -> Result<Vec<Uuid>> {
		let manager = self.connection_manager.read().await;
		Ok(manager.get_connected_devices())
	}

	/// Initialize pairing bridge with network identity and password
	pub async fn init_pairing(&mut self, password: String) -> Result<()> {
		if self.pairing_bridge.is_some() {
			return Ok(()); // Already initialized
		}

		// Get network identity from connection manager
		let network_identity = {
			let manager = self.connection_manager.read().await;
			manager.get_network_identity().await?
		};

		// Create pairing bridge
		let networking_service_ref = Arc::new(NetworkingServiceRef {
			connection_manager: self.connection_manager.clone(),
			device_manager: self.device_manager.clone(),
		});
		
		let pairing_bridge = Arc::new(PairingBridge::new(
			networking_service_ref,
			network_identity,
			password,
		));

		self.pairing_bridge = Some(pairing_bridge);
		tracing::info!("Pairing bridge initialized successfully");
		Ok(())
	}

	/// Start pairing as initiator with persistence integration
	pub async fn start_pairing_as_initiator(&self, auto_accept: bool) -> Result<PairingSession> {
		let bridge = self.pairing_bridge.as_ref()
			.ok_or_else(|| NetworkError::NotInitialized("Pairing bridge not initialized. Call init_pairing() first.".to_string()))?;

		bridge.start_pairing_as_initiator(auto_accept).await
	}

	/// Join pairing session with persistence integration  
	pub async fn join_pairing_session(&self, code: String) -> Result<()> {
		let bridge = self.pairing_bridge.as_ref()
			.ok_or_else(|| NetworkError::NotInitialized("Pairing bridge not initialized. Call init_pairing() first.".to_string()))?;

		bridge.join_pairing_session(code).await
	}

	/// Get status of active pairing sessions
	pub async fn get_pairing_status(&self) -> Vec<PairingSession> {
		if let Some(bridge) = &self.pairing_bridge {
			bridge.get_pairing_status().await
		} else {
			Vec::new()
		}
	}

	/// Cancel active pairing session
	pub async fn cancel_pairing(&self, session_id: Uuid) -> Result<()> {
		let bridge = self.pairing_bridge.as_ref()
			.ok_or_else(|| NetworkError::NotInitialized("Pairing bridge not initialized. Call init_pairing() first.".to_string()))?;

		bridge.cancel_pairing(session_id).await
	}

	/// Add a paired device to the network
	pub async fn add_paired_device(
		&self,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
	) -> Result<()> {
		let mut manager = self.connection_manager.write().await;
		manager.add_paired_device(device_info, session_keys).await
	}

	/// Revoke a paired device
	pub async fn revoke_device(&self, device_id: Uuid) -> Result<()> {
		let mut manager = self.connection_manager.write().await;
		manager.revoke_device(device_id).await
	}
}

// Protocol Handler Implementations

impl DatabaseSyncHandler {
	pub fn new() -> Self {
		Self {
            // TODO: Initialize with database reference
        }
	}
}

#[async_trait]
impl ProtocolHandler for DatabaseSyncHandler {
	async fn handle_message(
		&self,
		_device_id: Uuid,
		_message: DeviceMessage,
	) -> Result<Option<DeviceMessage>> {
		// Database sync handler is disabled until database sync messages are implemented
		Ok(None)
	}

	fn protocol_name(&self) -> &str {
		"database_sync"
	}

	fn supported_messages(&self) -> Vec<&str> {
		// Database sync messages are currently disabled
		vec![]
	}
}

impl FileTransferHandler {
	pub fn new() -> Self {
		Self {
            // TODO: Initialize with file operations reference
        }
	}
}

#[async_trait]
impl ProtocolHandler for FileTransferHandler {
	async fn handle_message(
		&self,
		device_id: Uuid,
		message: DeviceMessage,
	) -> Result<Option<DeviceMessage>> {
		match message {
			DeviceMessage::FileTransferRequest {
				transfer_id,
				file_path,
				..
			} => {
				tracing::info!(
					"File transfer request from device {} for {}",
					device_id,
					file_path
				);

				// TODO: Validate file access permissions and path
				// TODO: Start chunked file transfer

				Ok(Some(DeviceMessage::FileTransferResponse {
					transfer_id,
					accepted: true,
					reason: None,
				}))
			}
			DeviceMessage::FileChunk {
				transfer_id,
				chunk_index,
				data,
				is_final,
				..
			} => {
				tracing::debug!(
					"Received file chunk {} for transfer {}",
					chunk_index,
					transfer_id
				);

				// TODO: Receive and assemble file chunks

				Ok(Some(DeviceMessage::FileChunkAck {
					transfer_id,
					chunk_index,
					received: true,
				}))
			}
			_ => Ok(None),
		}
	}

	fn protocol_name(&self) -> &str {
		"file_transfer"
	}

	fn supported_messages(&self) -> Vec<&str> {
		vec![
			"file_transfer_request",
			"file_transfer_response",
			"file_chunk",
			"file_chunk_ack",
			"file_transfer_complete",
			"file_transfer_cancel",
		]
	}
}

impl SpacedropHandler {
	pub fn new() -> Self {
		Self {
            // TODO: Initialize with spacedrop operations reference
        }
	}
}

#[async_trait]
impl ProtocolHandler for SpacedropHandler {
	async fn handle_message(
		&self,
		device_id: Uuid,
		message: DeviceMessage,
	) -> Result<Option<DeviceMessage>> {
		match message {
			DeviceMessage::SpacedropRequest {
				transfer_id,
				file_metadata,
				sender_name,
				message: msg,
			} => {
				tracing::info!(
					"Spacedrop request from {} (device {}): {} - {}",
					sender_name,
					device_id,
					file_metadata.name,
					msg.as_deref().unwrap_or("no message")
				);

				// TODO: Show user notification and get approval
				// TODO: For now, auto-accept all Spacedrop requests

				Ok(Some(DeviceMessage::SpacedropResponse {
					transfer_id,
					accepted: true,
					save_path: Some(format!("/tmp/{}", file_metadata.name)),
				}))
			}
			DeviceMessage::SpacedropProgress {
				transfer_id,
				bytes_transferred,
				total_bytes,
				..
			} => {
				let progress = (bytes_transferred as f64 / total_bytes as f64) * 100.0;
				tracing::debug!("Spacedrop progress for {}: {:.1}%", transfer_id, progress);

				// TODO: Update UI with progress
				Ok(None)
			}
			_ => Ok(None),
		}
	}

	fn protocol_name(&self) -> &str {
		"spacedrop"
	}

	fn supported_messages(&self) -> Vec<&str> {
		vec![
			"spacedrop_request",
			"spacedrop_response",
			"spacedrop_progress",
		]
	}
}

impl RealtimeSyncHandler {
	pub fn new() -> Self {
		Self {
            // TODO: Initialize with real-time sync operations reference
        }
	}
}

#[async_trait]
impl ProtocolHandler for RealtimeSyncHandler {
	async fn handle_message(
		&self,
		device_id: Uuid,
		message: DeviceMessage,
	) -> Result<Option<DeviceMessage>> {
		match message {
			DeviceMessage::LocationUpdate {
				location_id,
				changes,
				..
			} => {
				tracing::info!(
					"Location update from device {} for location {}: {} changes",
					device_id,
					location_id,
					changes.len()
				);

				// TODO: Apply location changes to local state
				Ok(None)
			}
			DeviceMessage::IndexerProgress {
				location_id,
				progress,
				..
			} => {
				tracing::debug!(
					"Indexer progress from device {} for location {}: {}/{} files",
					device_id,
					location_id,
					progress.processed_files,
					progress.total_files
				);

				// TODO: Update UI with indexer progress
				Ok(None)
			}
			DeviceMessage::FileSystemEvent {
				location_id, event, ..
			} => {
				tracing::debug!(
					"File system event from device {} for location {}: {:?}",
					device_id,
					location_id,
					event
				);

				// TODO: Handle file system events
				Ok(None)
			}
			_ => Ok(None),
		}
	}

	fn protocol_name(&self) -> &str {
		"realtime_sync"
	}

	fn supported_messages(&self) -> Vec<&str> {
		vec!["location_update", "indexer_progress", "fs_event"]
	}
}
