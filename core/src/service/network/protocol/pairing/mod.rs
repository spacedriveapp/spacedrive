//! Pairing protocol handler

pub mod initiator;
pub mod joiner;
pub mod messages;
pub mod persistence;
pub mod security;
pub mod types;

// Re-export main types
pub use messages::PairingMessage;
pub use types::{PairingAdvertisement, PairingCode, PairingRole, PairingSession, PairingState};

use super::{ProtocolEvent, ProtocolHandler};
use crate::service::network::{
	device::{DeviceInfo, DeviceRegistry, SessionKeys},
	utils::{identity::NetworkFingerprint, logging::NetworkLogger, NetworkIdentity},
	NetworkingError, Result,
};
use async_trait::async_trait;
use blake3;
use iroh::{Endpoint, NodeAddr, NodeId};
use persistence::PairingPersistence;
use security::PairingSecurity;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Pairing protocol handler
pub struct PairingProtocolHandler {
	/// Network identity for signing
	identity: NetworkIdentity,

	/// Device registry for state management
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// Active pairing sessions
	active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,

	/// Pairing codes for active sessions (session_id -> pairing_code)
	pairing_codes: Arc<RwLock<HashMap<Uuid, PairingCode>>>,

	/// Logger for structured logging
	logger: Arc<dyn NetworkLogger>,

	/// Command sender for dispatching commands to the NetworkingEventLoop
	command_sender: tokio::sync::mpsc::UnboundedSender<
		crate::service::network::core::event_loop::EventLoopCommand,
	>,

	/// Current pairing role
	role: Option<PairingRole>,

	/// Session persistence manager
	persistence: Option<Arc<PairingPersistence>>,
}

impl PairingProtocolHandler {
	/// Create a new pairing protocol handler
	pub fn new(
		identity: NetworkIdentity,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		logger: Arc<dyn NetworkLogger>,
		command_sender: tokio::sync::mpsc::UnboundedSender<
			crate::service::network::core::event_loop::EventLoopCommand,
		>,
	) -> Self {
		Self {
			identity,
			device_registry,
			active_sessions: Arc::new(RwLock::new(HashMap::new())),
			pairing_codes: Arc::new(RwLock::new(HashMap::new())),
			logger,
			command_sender,
			role: None,
			persistence: None,
		}
	}

	/// Create a new pairing protocol handler with persistence
	pub fn new_with_persistence(
		identity: NetworkIdentity,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		logger: Arc<dyn NetworkLogger>,
		command_sender: tokio::sync::mpsc::UnboundedSender<
			crate::service::network::core::event_loop::EventLoopCommand,
		>,
		data_dir: PathBuf,
	) -> Self {
		let persistence = Arc::new(PairingPersistence::new(data_dir));
		Self {
			identity,
			device_registry,
			active_sessions: Arc::new(RwLock::new(HashMap::new())),
			pairing_codes: Arc::new(RwLock::new(HashMap::new())),
			logger,
			command_sender,
			role: None,
			persistence: Some(persistence),
		}
	}

	/// Initialize sessions from persistence (call after construction)
	pub async fn load_persisted_sessions(&self) -> Result<usize> {
		if let Some(persistence) = &self.persistence {
			let sessions = persistence.load_sessions().await?;
			let count = sessions.len();

			if count > 0 {
				*self.active_sessions.write().await = sessions;
				self.log_info(&format!("Loaded {} persisted pairing sessions", count))
					.await;
			}

			Ok(count)
		} else {
			Ok(0)
		}
	}

	/// Save current sessions to persistence
	async fn save_sessions_to_persistence(&self) -> Result<()> {
		if let Some(persistence) = &self.persistence {
			let sessions = self.active_sessions.read().await;
			persistence.save_sessions(&sessions).await?;
		}
		Ok(())
	}

	/// Log info message with role prefix
	async fn log_info(&self, message: &str) {
		let role_prefix = match &self.role {
			Some(PairingRole::Initiator) => "[INITIATOR]",
			Some(PairingRole::Joiner) => "[JOINER]",
			None => "[PAIRING]",
		};
		self.logger
			.info(&format!("{} {}", role_prefix, message))
			.await;
	}

	/// Log debug message with role prefix
	async fn log_debug(&self, message: &str) {
		let role_prefix = match &self.role {
			Some(PairingRole::Initiator) => "[INITIATOR]",
			Some(PairingRole::Joiner) => "[JOINER]",
			None => "[PAIRING]",
		};
		self.logger
			.debug(&format!("{} {}", role_prefix, message))
			.await;
	}

	/// Log warning message with role prefix
	async fn log_warn(&self, message: &str) {
		let role_prefix = match &self.role {
			Some(PairingRole::Initiator) => "[INITIATOR]",
			Some(PairingRole::Joiner) => "[JOINER]",
			None => "[PAIRING]",
		};
		self.logger
			.warn(&format!("{} {}", role_prefix, message))
			.await;
	}

	/// Log error message with role prefix
	async fn log_error(&self, message: &str) {
		let role_prefix = match &self.role {
			Some(PairingRole::Initiator) => "[INITIATOR]",
			Some(PairingRole::Joiner) => "[JOINER]",
			None => "[PAIRING]",
		};
		self.logger
			.error(&format!("{} {}", role_prefix, message))
			.await;
	}

	/// Start a new pairing session as initiator
	/// Returns the session ID which should be advertised via DHT by the caller
	pub async fn start_pairing_session(&self) -> Result<Uuid> {
		let session_id = Uuid::new_v4();
		let pairing_code = PairingCode::from_session_id(session_id);
		self.start_pairing_session_with_id(session_id, pairing_code)
			.await?;
		Ok(session_id)
	}

	/// Start a new pairing session with a specific session ID and pairing code
	pub async fn start_pairing_session_with_id(
		&self,
		session_id: Uuid,
		pairing_code: PairingCode,
	) -> Result<()> {
		let session = PairingSession {
			id: session_id,
			state: PairingState::WaitingForConnection,
			remote_device_id: None,
			remote_device_info: None,
			remote_public_key: None,
			shared_secret: None,
			created_at: chrono::Utc::now(),
		};

		self.active_sessions
			.write()
			.await
			.insert(session_id, session);

		// Store the pairing code for this session
		self.pairing_codes
			.write()
			.await
			.insert(session_id, pairing_code);

		// Save to persistence
		self.save_sessions_to_persistence().await?;

		self.log_info(&format!("Started pairing session: {}", session_id))
			.await;
		Ok(())
	}

	/// Get the pairing code for the most recent session (for generating QR codes)
	pub async fn get_current_pairing_code(&self) -> Option<PairingCode> {
		let codes = self.pairing_codes.read().await;
		// Get the most recent pairing code (last inserted)
		codes.values().last().cloned()
	}

	/// Join an existing pairing session with a specific session ID and pairing code
	/// This allows a joiner to participate in an initiator's session
	pub async fn join_pairing_session(
		&self,
		session_id: Uuid,
		pairing_code: PairingCode,
	) -> Result<()> {
		// Check if the pairing code has expired
		if pairing_code.is_expired() {
			return Err(NetworkingError::Protocol(
				"Pairing code has expired. Please request a new code from the initiator."
					.to_string(),
			));
		}

		// Check if session already exists to prevent conflicts
		{
			let sessions = self.active_sessions.read().await;
			if let Some(existing_session) = sessions.get(&session_id) {
				return Err(NetworkingError::Protocol(format!(
					"Session {} already exists in state {:?}",
					session_id, existing_session.state
				)));
			}
		}

		// Create new scanning session for the joiner
		let session = PairingSession {
			id: session_id,
			state: PairingState::Scanning, // Joiner starts in scanning state
			remote_device_id: None,
			remote_device_info: None,
			remote_public_key: None,
			shared_secret: None,
			created_at: chrono::Utc::now(),
		};

		// Insert the session
		{
			let mut sessions = self.active_sessions.write().await;
			sessions.insert(session_id, session);
		}

		// Store the pairing code for this session
		self.pairing_codes
			.write()
			.await
			.insert(session_id, pairing_code);

		// Save to persistence
		self.save_sessions_to_persistence().await?;

		self.log_info(&format!(
			"Joined pairing session: {} (state: Scanning)",
			session_id
		))
		.await;

		// Verify session was created correctly
		let sessions = self.active_sessions.read().await;
		if let Some(created_session) = sessions.get(&session_id) {
			if matches!(created_session.state, PairingState::Scanning) {
				self.log_debug(&format!(
					"Pairing session verified in Scanning state: {}",
					session_id
				))
				.await;
			} else {
				return Err(NetworkingError::Protocol(format!(
					"Session {} created in wrong state: {:?}",
					session_id, created_session.state
				)));
			}
		} else {
			return Err(NetworkingError::Protocol(format!(
				"Failed to verify session creation: {}",
				session_id
			)));
		}

		Ok(())
	}

	/// Get device info for advertising in DHT records
	pub async fn get_device_info(&self) -> Result<DeviceInfo> {
		// Get device info from device registry (which uses device manager)
		let mut device_info = self.device_registry.read().await.get_local_device_info()?;

		// Update network fingerprint with current identity
		device_info.network_fingerprint = self.identity.network_fingerprint();
		device_info.last_seen = chrono::Utc::now();

		Ok(device_info)
	}

	/// Cancel a pairing session
	pub async fn cancel_session(&self, session_id: Uuid) -> Result<()> {
		self.active_sessions.write().await.remove(&session_id);
		self.pairing_codes.write().await.remove(&session_id);
		self.save_sessions_to_persistence().await?;
		Ok(())
	}

	/// Get active pairing sessions
	pub async fn get_active_sessions(&self) -> Vec<PairingSession> {
		let sessions = {
			let read_guard = self.active_sessions.read().await;
			read_guard.values().cloned().collect::<Vec<_>>()
		};
		sessions
	}

	/// Clean up expired pairing sessions
	pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
		let now = chrono::Utc::now();
		let timeout_duration = chrono::Duration::minutes(10); // 10 minute timeout

		let mut sessions = self.active_sessions.write().await;
		let mut pairing_codes = self.pairing_codes.write().await;
		let initial_count = sessions.len();

		// Collect session IDs to remove first
		let mut sessions_to_remove = Vec::new();
		for (session_id, session) in sessions.iter() {
			let age = now.signed_duration_since(session.created_at);
			if age > timeout_duration {
				sessions_to_remove.push(*session_id);
			}
		}

		// Remove expired sessions and their pairing codes
		for session_id in &sessions_to_remove {
			sessions.remove(session_id);
			pairing_codes.remove(session_id);
		}

		let cleaned_count = sessions_to_remove.len();
		if cleaned_count > 0 {
			self.log_info(&format!(
				"Cleaned up {} expired pairing sessions",
				cleaned_count
			))
			.await;
		}

		Ok(cleaned_count)
	}

	/// Start a background task to periodically clean up expired sessions
	pub fn start_cleanup_task(handler: Arc<Self>) {
		let logger = handler.logger.clone();
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every minute

			loop {
				interval.tick().await;

				if let Err(e) = handler.cleanup_expired_sessions().await {
					logger
						.error(&format!("Error during session cleanup: {}", e))
						.await;
				}
			}
		});
	}

	/// Start the background task for managing pairing state transitions
	pub fn start_state_machine_task(handler: Arc<Self>) {
		tokio::spawn(async move {
			// Check the state every 200 milliseconds
			let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(200));

			loop {
				interval.tick().await;
				if let Err(e) = handler.process_state_transitions().await {
					handler
						.log_error(&format!("State machine error: {}", e))
						.await;
				}
			}
		});
	}

	/// The core logic of the state machine - processes state transitions for all active sessions
	async fn process_state_transitions(&self) -> Result<()> {
		// Get a write lock because we may need to modify session states
		let mut sessions = self.active_sessions.write().await;

		for session in sessions.values_mut() {
			// Match on the current state to decide the next action
			match &session.state {
				// This is the critical missing logic - handle ResponsePending state
				PairingState::ResponsePending {
					response_data,
					remote_node_id,
					..
				} => {
					if let Some(node_id) = remote_node_id {
						self.log_info(&format!(
                            "State Machine: Found ResponsePending for session {}, sending response to node {}",
                            session.id, node_id
                        )).await;

						// Create the command to send the message
						let command = crate::service::network::core::event_loop::EventLoopCommand::SendMessageToNode {
                            node_id: *node_id,
                            protocol: "pairing".to_string(),
                            data: response_data.clone(),
                        };

						// Send the command to the NetworkingEventLoop
						if self.command_sender.send(command).is_ok() {
							// Transition the state to prevent re-sending
							session.state = PairingState::ResponseSent;
							self.log_info(&format!(
                                "State Machine: Response sent for session {}, transitioned to ResponseSent",
                                session.id
                            )).await;
						} else {
							self.log_error("State Machine: Failed to send command to event loop.")
								.await;
							session.state = PairingState::Failed {
								reason: "Internal channel closed".to_string(),
							};
						}
					} else {
						self.log_error(&format!(
							"State Machine: Session {} in ResponsePending but no remote node ID",
							session.id
						))
						.await;
						session.state = PairingState::Failed {
							reason: "No remote node ID for response".to_string(),
						};
					}
				}

				// Optional: Add logic to time out sessions stuck in scanning for too long
				PairingState::Scanning => {
					let age = chrono::Utc::now().signed_duration_since(session.created_at);
					if age > chrono::Duration::minutes(5) {
						// 5 minute timeout for scanning
						self.log_warn(&format!(
							"State Machine: Session {} timed out while scanning, marking as failed",
							session.id
						))
						.await;
						session.state = PairingState::Failed {
							reason: "Scanning timeout".to_string(),
						};
					}
				}

				// No action needed for other states in this loop
				_ => {
					// Other states are handled elsewhere or don't need periodic processing
				}
			}
		}

		Ok(())
	}

	fn generate_challenge(&self) -> Result<Vec<u8>> {
		use rand::RngCore;
		let mut challenge = vec![0u8; 32];
		rand::thread_rng().fill_bytes(&mut challenge);
		Ok(challenge)
	}

	/// Generate shared secret for a pairing session using the pairing code secret
	async fn generate_shared_secret(&self, session_id: Uuid) -> Result<Vec<u8>> {
		let pairing_codes = self.pairing_codes.read().await;
		let pairing_code = pairing_codes.get(&session_id).ok_or_else(|| {
			NetworkingError::Protocol(format!("No pairing code found for session {}", session_id))
		})?;

		// Use the pairing code secret as the shared secret
		Ok(pairing_code.secret().to_vec())
	}

	/// Handle a pairing message received over stream
	async fn handle_pairing_message(
		&self,
		message: PairingMessage,
		remote_node_id: NodeId,
	) -> Result<Option<Vec<u8>>> {
		match message {
			PairingMessage::PairingRequest {
				session_id,
				device_info,
				public_key,
			} => {
				// Generate a temporary device ID based on node ID
				let from_device = self.get_device_id_for_node(remote_node_id).await;
				let response = self
					.handle_pairing_request(from_device, session_id, device_info, public_key)
					.await?;
				Ok(Some(response))
			}
			PairingMessage::Challenge {
				session_id,
				challenge,
				device_info,
			} => {
				let response = self
					.handle_pairing_challenge(session_id, challenge, device_info)
					.await?;
				Ok(Some(response))
			}
			PairingMessage::Response {
				session_id,
				response,
				device_info,
			} => {
				let from_device = self.get_device_id_for_node(remote_node_id).await;
				let response = self
					.handle_pairing_response(from_device, session_id, response, device_info)
					.await?;
				Ok(Some(response))
			}
			PairingMessage::Complete {
				session_id,
				success,
				reason,
			} => {
				let from_device = self.get_device_id_for_node(remote_node_id).await;
				self.handle_completion(session_id, success, reason, from_device, remote_node_id)
					.await?;
				Ok(None) // No response needed
			}
		}
	}

	/// Get or create a device ID for a node
	async fn get_device_id_for_node(&self, node_id: NodeId) -> Uuid {
		let registry = self.device_registry.read().await;
		registry.get_device_by_node(node_id).unwrap_or_else(|| {
			// Generate a deterministic UUID from the node ID
			let mut hasher = blake3::Hasher::new();
			hasher.update(b"spacedrive-device-id");
			hasher.update(node_id.as_bytes());
			let hash = hasher.finalize();
			let mut uuid_bytes = [0u8; 16];
			uuid_bytes.copy_from_slice(&hash.as_bytes()[..16]);
			Uuid::from_bytes(uuid_bytes)
		})
	}

	/// Send a pairing message to a specific node using Iroh streams
	pub async fn send_pairing_message_to_node(
		&self,
		endpoint: &Endpoint,
		node_id: NodeId,
		message: &PairingMessage,
	) -> Result<Option<PairingMessage>> {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		// Create node address and connect
		let node_addr = NodeAddr::new(node_id);
		let conn = endpoint
			.connect(node_addr, crate::service::network::core::PAIRING_ALPN)
			.await
			.map_err(|e| NetworkingError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

		// Open a bidirectional stream
		let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
			NetworkingError::ConnectionFailed(format!("Failed to open stream: {}", e))
		})?;

		// Serialize the message
		let msg_data =
			serde_json::to_vec(message).map_err(|e| NetworkingError::Serialization(e))?;

		// Send message length
		let len = msg_data.len() as u32;
		send.write_all(&len.to_be_bytes())
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to write length: {}", e)))?;

		// Send message
		send.write_all(&msg_data)
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to write message: {}", e)))?;

		// Flush
		send.flush()
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to flush: {}", e)))?;

		// Read response length
		let mut len_buf = [0u8; 4];
		match recv.read_exact(&mut len_buf).await {
			Ok(_) => {
				let resp_len = u32::from_be_bytes(len_buf) as usize;

				// Read response
				let mut resp_buf = vec![0u8; resp_len];
				recv.read_exact(&mut resp_buf).await.map_err(|e| {
					NetworkingError::Transport(format!("Failed to read response: {}", e))
				})?;

				// Deserialize response
				let response: PairingMessage = serde_json::from_slice(&resp_buf)
					.map_err(|e| NetworkingError::Serialization(e))?;

				Ok(Some(response))
			}
			Err(_) => Ok(None), // No response
		}
	}
}

#[async_trait]
impl ProtocolHandler for PairingProtocolHandler {
	fn protocol_name(&self) -> &str {
		"pairing"
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: NodeId,
	) {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		self.logger
			.info(&format!(
				"handle_stream called from node {}",
				remote_node_id
			))
			.await;

		// Keep the stream alive for multiple message exchanges
		loop {
			// Read the message length (4 bytes)
			let mut len_buf = [0u8; 4];
			match recv.read_exact(&mut len_buf).await {
				Ok(_) => {}
				Err(e) => {
					// Connection closed or error - this is normal when the other side closes
					self.logger
						.debug(&format!(
							"Stream closed or error reading message length: {}",
							e
						))
						.await;
					break;
				}
			}
			let msg_len = u32::from_be_bytes(len_buf) as usize;
			self.logger
				.info(&format!("Read message length: {} bytes", msg_len))
				.await;

			// Read the message
			let mut msg_buf = vec![0u8; msg_len];
			if let Err(e) = recv.read_exact(&mut msg_buf).await {
				self.logger
					.error(&format!("Failed to read message: {}", e))
					.await;
				break;
			}

			// Deserialize and handle the message
			let message: PairingMessage = match serde_json::from_slice(&msg_buf) {
				Ok(msg) => {
					// Log the message type
					let msg_type = match &msg {
						PairingMessage::PairingRequest { .. } => "PairingRequest",
						PairingMessage::Challenge { .. } => "Challenge",
						PairingMessage::Response { .. } => "Response",
						PairingMessage::Complete { .. } => "Complete",
					};
					self.logger
						.info(&format!(
							"Received {} message from {}",
							msg_type, remote_node_id
						))
						.await;
					msg
				}
				Err(e) => {
					self.logger
						.error(&format!("Failed to deserialize pairing message: {}", e))
						.await;
					break;
				}
			};

			// Process the message and get response
			let response = match self
				.handle_pairing_message(message.clone(), remote_node_id)
				.await
			{
				Ok(resp) => resp,
				Err(e) => {
					self.logger
						.error(&format!("Failed to handle pairing message: {}", e))
						.await;
					break;
				}
			};

			// Send response if any
			if let Some(response_data) = response {
				// Write message length
				let len = response_data.len() as u32;
				if let Err(e) = send.write_all(&len.to_be_bytes()).await {
					self.logger
						.error(&format!("Failed to write response length: {}", e))
						.await;
					break;
				}

				// Write message
				if let Err(e) = send.write_all(&response_data).await {
					self.logger
						.error(&format!("Failed to write response: {}", e))
						.await;
					break;
				}

				// Flush the stream
				if let Err(e) = send.flush().await {
					self.logger
						.error(&format!("Failed to flush stream: {}", e))
						.await;
					break;
				}
			}

			// Check if this was a completion message - if so, we can close the stream
			if matches!(message, PairingMessage::Complete { .. }) {
				self.logger
					.info("Received Complete message, closing pairing stream")
					.await;
				break;
			}
		}

		self.logger
			.info(&format!(
				"Pairing stream handler completed for node {}",
				remote_node_id
			))
			.await;
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		let message: PairingMessage =
			serde_json::from_slice(&request_data).map_err(|e| NetworkingError::Serialization(e))?;

		let result = match message {
			// Initiator handles these messages
			PairingMessage::PairingRequest {
				session_id,
				device_info,
				public_key,
			} => {
				self.handle_pairing_request(from_device, session_id, device_info, public_key)
					.await
			}
			PairingMessage::Response {
				session_id,
				response,
				device_info,
			} => {
				self.handle_pairing_response(from_device, session_id, response, device_info)
					.await
			}
			// These are handled by handle_response, not handle_request
			PairingMessage::Challenge { .. } | PairingMessage::Complete { .. } => {
				self.log_warn("Received Challenge or Complete in handle_request - this should be handled by handle_response").await;
				Ok(Vec::new())
			}
		};

		// Handle errors by marking session as failed
		if let Err(ref error) = result {
			// Try to extract session ID from the original message for error tracking
			if let Ok(message) = serde_json::from_slice::<PairingMessage>(&request_data) {
				let session_id = match message {
					PairingMessage::PairingRequest { session_id, .. } => Some(session_id),
					PairingMessage::Challenge { session_id, .. } => Some(session_id),
					PairingMessage::Response { session_id, .. } => Some(session_id),
					PairingMessage::Complete { session_id, .. } => Some(session_id),
				};

				if let Some(session_id) = session_id {
					// Mark session as failed
					if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
						session.state = PairingState::Failed {
							reason: error.to_string(),
						};
						self.log_error(&format!(
							"Marked pairing session {} as failed: {}",
							session_id, error
						))
						.await;
					}
				}
			}
		}

		result
	}

	async fn handle_response(
		&self,
		from_device: Uuid,
		from_node: NodeId,
		response_data: Vec<u8>,
	) -> Result<()> {
		self.log_debug(&format!(
			"handle_response called with {} bytes from device {}",
			response_data.len(),
			from_device
		))
		.await;

		// Parse the response message
		let message: PairingMessage = serde_json::from_slice(&response_data)
			.map_err(|e| NetworkingError::Serialization(e))?;

		self.log_debug("Parsed message type successfully").await;

		// Process the response based on the message type
		match message {
			// Joiner handles these messages
			PairingMessage::Challenge {
				session_id,
				challenge,
				device_info,
			} => {
				self.log_info(&format!(
					"Received challenge for session {} with {} byte challenge",
					session_id,
					challenge.len()
				))
				.await;

				// Check session state before processing
				{
					let sessions = self.active_sessions.read().await;
					if let Some(session) = sessions.get(&session_id) {
						self.log_debug(&format!(
							"Session {} state before challenge processing: {}",
							session_id, session.state
						))
						.await;
					} else {
						self.log_debug(&format!("No session found for {}", session_id))
							.await;
					}
				}

				self.log_debug("About to call handle_pairing_challenge...")
					.await;

				// Call the existing handle_pairing_challenge method
				match self
					.handle_pairing_challenge(session_id, challenge.clone(), device_info)
					.await
				{
					Ok(response_data) => {
						self.log_debug(&format!(
							"handle_pairing_challenge succeeded, generated {} byte response",
							response_data.len()
						))
						.await;

						// Check session state after handle_pairing_challenge
						{
							let sessions = self.active_sessions.read().await;
							if let Some(session) = sessions.get(&session_id) {
								self.log_debug(&format!(
									"Session {} state after handle_pairing_challenge: {}",
									session_id, session.state
								))
								.await;
							}
						}

						// Use the node ID directly from the method parameter (this is Initiator's node ID)
						let remote_node_id = Some(from_node);
						self.log_debug(&format!(
							"Using node ID from method parameter: {:?}",
							from_node
						))
						.await;

						// Instead of using ResponsePending state (which relies on state machine),
						// send the response directly via the command sender
						{
							let mut sessions = self.active_sessions.write().await;
							if let Some(session) = sessions.get_mut(&session_id) {
								// Only mark as ResponseSent if not already completed
								// handle_pairing_challenge may have already set it to Completed
								if !matches!(session.state, PairingState::Completed) {
									session.state = PairingState::ResponseSent;
									self.log_debug(&format!(
										"Session {} marked as ResponseSent",
										session_id
									))
									.await;
								} else {
									self.log_debug(&format!(
										"Session {} already completed, keeping state",
										session_id
									))
									.await;
								}
							} else {
								self.log_error(&format!(
									"ERROR: Session {} not found when trying to update state",
									session_id
								))
								.await;
							}
						}

						// Send the response directly via command sender
						self.log_info(&format!(
							"Sending challenge response directly to node {}",
							from_node
						))
						.await;

						let command = crate::service::network::core::event_loop::EventLoopCommand::SendMessageToNode {
                            node_id: from_node,
                            protocol: "pairing".to_string(),
                            data: response_data.clone(),
                        };

						if let Err(e) = self.command_sender.send(command) {
							self.log_error(&format!("Failed to send response command: {:?}", e))
								.await;
							// Mark session as failed
							let mut sessions = self.active_sessions.write().await;
							if let Some(session) = sessions.get_mut(&session_id) {
								session.state = PairingState::Failed {
									reason: "Failed to send response".to_string(),
								};
							}
						} else {
							self.log_info(&format!(
								"Challenge response sent successfully for session {}",
								session_id
							))
							.await;
						}
					}
					Err(e) => {
						self.log_error(&format!(
							"handle_pairing_challenge FAILED for session {}: {}",
							session_id, e
						))
						.await;
					}
				}
			}
			PairingMessage::Complete {
				session_id,
				success,
				reason,
			} => {
				self.handle_completion(session_id, success, reason, from_device, from_node)
					.await?;
			}
			// These are handled by handle_request, not handle_response
			PairingMessage::PairingRequest { .. } | PairingMessage::Response { .. } => {
				self.log_warn("Received PairingRequest or Response in handle_response - this should be handled by handle_request").await;
			}
		}

		self.log_debug("handle_response completed").await;
		Ok(())
	}

	async fn handle_event(&self, event: ProtocolEvent) -> Result<()> {
		match event {
			ProtocolEvent::DeviceDisconnected { device_id } => {
				// Clean up any active sessions for this device
				let mut sessions = self.active_sessions.write().await;
				sessions.retain(|_, session| session.remote_device_id != Some(device_id));
			}
			_ => {}
		}

		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}
