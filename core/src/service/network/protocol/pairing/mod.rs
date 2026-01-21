//! Pairing protocol handler

pub mod initiator;
pub mod joiner;
pub mod messages;
pub mod persistence;
pub mod proxy;
pub mod security;
pub mod types;
pub mod vouching_queue;

/// Maximum message size for pairing protocol (1MB)
/// Prevents DoS attacks via oversized message claims
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

// Re-export main types
pub use messages::PairingMessage;
pub use proxy::{
	AcceptedDevice, RejectedDevice, VouchPayload, VouchState, VouchStatus, VouchingSession,
	VouchingSessionState,
};
pub use types::{PairingAdvertisement, PairingCode, PairingRole, PairingSession, PairingState};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use blake3;
use iroh::{endpoint::Connection, Endpoint, NodeAddr, NodeId, Watcher};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{ProtocolEvent, ProtocolHandler};
use crate::{
	config::ProxyPairingConfig,
	infra::event::{Event, EventBus, ResourceMetadata},
	service::network::{
		device::{DeviceInfo, DeviceRegistry, SessionKeys},
		utils::{self, identity::NetworkFingerprint, logging::NetworkLogger, NetworkIdentity},
		NetworkingError, Result,
	},
};
use persistence::PairingPersistence;
use security::PairingSecurity;
use vouching_queue::{VouchQueueStatus, VouchingQueue, VouchingQueueEntry};

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

	/// Event sender for broadcasting network events (e.g., PairingConfirmationRequired)
	event_sender: tokio::sync::broadcast::Sender<crate::service::network::core::NetworkEvent>,

	/// Current pairing role
	role: Option<PairingRole>,

	/// Session persistence manager
	persistence: Option<Arc<PairingPersistence>>,

	/// Endpoint for creating and managing connections
	endpoint: Option<Endpoint>,

	/// Cached connections to remote nodes (keyed by NodeId and ALPN)
	connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,

	/// Event bus for emitting pairing events
	event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,

	/// Proxy pairing configuration
	proxy_config: Arc<RwLock<ProxyPairingConfig>>,

	/// Active proxy vouching sessions
	vouching_sessions: Arc<RwLock<HashMap<Uuid, VouchingSession>>>,

	/// Pending proxy confirmations awaiting user action
	pending_proxy_confirmations: Arc<RwLock<HashMap<Uuid, PendingProxyConfirmation>>>,

	/// Persistent queue for offline vouches
	vouching_queue: Arc<RwLock<Option<Arc<VouchingQueue>>>>,

	/// Cached vouchee session keys for proxy pairing completion
	vouching_keys: Arc<RwLock<HashMap<(Uuid, Uuid), SessionKeys>>>,
}

#[derive(Debug, Clone)]
struct PendingProxyConfirmation {
	session_id: Uuid,
	voucher_device_id: Uuid,
	voucher_device_name: String,
	vouchee_device_info: DeviceInfo,
	vouchee_public_key: Vec<u8>,
	proxied_session_keys: SessionKeys,
	created_at: chrono::DateTime<chrono::Utc>,
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
		event_sender: tokio::sync::broadcast::Sender<crate::service::network::core::NetworkEvent>,
		endpoint: Option<Endpoint>,
		active_connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
	) -> Self {
		Self {
			identity,
			device_registry,
			active_sessions: Arc::new(RwLock::new(HashMap::new())),
			pairing_codes: Arc::new(RwLock::new(HashMap::new())),
			logger,
			command_sender,
			event_sender,
			role: None,
			persistence: None,
			endpoint,
			connections: active_connections,
			event_bus: Arc::new(RwLock::new(None)),
			proxy_config: Arc::new(RwLock::new(ProxyPairingConfig::default())),
			vouching_sessions: Arc::new(RwLock::new(HashMap::new())),
			pending_proxy_confirmations: Arc::new(RwLock::new(HashMap::new())),
			vouching_queue: Arc::new(RwLock::new(None)),
			vouching_keys: Arc::new(RwLock::new(HashMap::new())),
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
		event_sender: tokio::sync::broadcast::Sender<crate::service::network::core::NetworkEvent>,
		data_dir: PathBuf,
		endpoint: Option<Endpoint>,
		active_connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
	) -> Self {
		let persistence = Arc::new(PairingPersistence::new(data_dir));
		Self {
			identity,
			device_registry,
			active_sessions: Arc::new(RwLock::new(HashMap::new())),
			pairing_codes: Arc::new(RwLock::new(HashMap::new())),
			logger,
			command_sender,
			event_sender,
			role: None,
			persistence: Some(persistence),
			endpoint,
			connections: active_connections,
			event_bus: Arc::new(RwLock::new(None)),
			proxy_config: Arc::new(RwLock::new(ProxyPairingConfig::default())),
			vouching_sessions: Arc::new(RwLock::new(HashMap::new())),
			pending_proxy_confirmations: Arc::new(RwLock::new(HashMap::new())),
			vouching_queue: Arc::new(RwLock::new(None)),
			vouching_keys: Arc::new(RwLock::new(HashMap::new())),
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

	pub async fn set_event_bus(&self, event_bus: Arc<EventBus>) {
		let mut guard = self.event_bus.write().await;
		*guard = Some(event_bus);
	}

	pub async fn set_proxy_config(&self, config: ProxyPairingConfig) {
		let mut guard = self.proxy_config.write().await;
		*guard = config;
	}

	pub async fn init_vouching_queue(&self, data_dir: PathBuf) -> Result<()> {
		let queue = VouchingQueue::open(data_dir).await?;
		let mut guard = self.vouching_queue.write().await;
		*guard = Some(Arc::new(queue));
		Ok(())
	}

	pub fn start_vouching_queue_task(handler: Arc<Self>) {
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
			loop {
				interval.tick().await;
				if let Err(e) = handler.process_vouching_queue().await {
					handler
						.log_error(&format!("Vouching queue error: {}", e))
						.await;
				}
			}
		});
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
		let pairing_code = PairingCode::generate()?;
		let session_id = pairing_code.session_id();
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
			confirmation_code: None,
			confirmation_expires_at: None,
			pending_challenge: None,
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
			confirmation_code: None,
			confirmation_expires_at: None,
			pending_challenge: None,
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

				// Handle confirmation timeout
				PairingState::AwaitingUserConfirmation { expires_at, .. } => {
					if chrono::Utc::now() > *expires_at {
						self.log_warn(&format!(
							"State Machine: Session {} confirmation timed out, marking as rejected",
							session.id
						))
						.await;

						// Emit timeout event
						let _ = self.event_sender.send(
							crate::service::network::core::NetworkEvent::PairingRejected {
								session_id: session.id,
								reason: "Confirmation timeout".to_string(),
							},
						);

						session.state = PairingState::Rejected {
							reason: "Confirmation timeout - user did not respond".to_string(),
						};

						// Clear confirmation fields
						session.confirmation_code = None;
						session.confirmation_expires_at = None;
						session.pending_challenge = None;
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

	fn build_vouch_payload(
		&self,
		session_id: Uuid,
		vouchee_device_info: &DeviceInfo,
		vouchee_public_key: &[u8],
		timestamp: chrono::DateTime<chrono::Utc>,
	) -> VouchPayload {
		VouchPayload {
			vouchee_device_id: vouchee_device_info.device_id,
			vouchee_public_key: vouchee_public_key.to_vec(),
			vouchee_device_info: vouchee_device_info.clone(),
			timestamp,
			session_id,
		}
	}

	fn sign_vouch_payload(&self, payload: &VouchPayload) -> Result<Vec<u8>> {
		let serialized =
			bincode::serialize(payload).map_err(|e| NetworkingError::Serialization(e))?;
		self.identity.sign(&serialized)
	}

	fn verify_vouch_signature(
		&self,
		payload: &VouchPayload,
		signature: &[u8],
		public_key_bytes: &[u8],
	) -> Result<bool> {
		PairingSecurity::validate_public_key(public_key_bytes)?;
		PairingSecurity::validate_signature(signature)?;
		let serialized =
			bincode::serialize(payload).map_err(|e| NetworkingError::Serialization(e))?;

		use ed25519_dalek::{Signature, Verifier, VerifyingKey};
		let verifying_key =
			VerifyingKey::from_bytes(public_key_bytes.try_into().map_err(|_| {
				NetworkingError::Protocol("Invalid voucher public key length".to_string())
			})?)
			.map_err(|e| NetworkingError::Protocol(format!("Invalid voucher public key: {}", e)))?;

		let sig = Signature::from_slice(signature)
			.map_err(|e| NetworkingError::Protocol(format!("Invalid signature: {}", e)))?;

		Ok(verifying_key.verify(&serialized, &sig).is_ok())
	}

	fn derive_proxy_shared_secret(
		&self,
		voucher_device_id: Uuid,
		target_device_id: Uuid,
		vouchee_device_id: Uuid,
		vouchee_public_key: &[u8],
		base_secret: &[u8],
	) -> Result<Vec<u8>> {
		use hkdf::Hkdf;
		use sha2::Sha256;

		let context = format!(
			"spacedrive-proxy-pairing-{}:{}:{}:{}",
			voucher_device_id,
			target_device_id,
			vouchee_device_id,
			hex::encode(vouchee_public_key)
		);

		let hkdf = Hkdf::<Sha256>::new(None, base_secret);
		let mut derived = [0u8; 32];
		hkdf.expand(context.as_bytes(), &mut derived).map_err(|e| {
			NetworkingError::Protocol(format!("Failed to derive proxy shared secret: {}", e))
		})?;

		Ok(derived.to_vec())
	}

	fn derive_proxy_session_keys(
		&self,
		voucher_device_id: Uuid,
		target_device_id: Uuid,
		vouchee_device_id: Uuid,
		vouchee_public_key: &[u8],
		base_secret: &[u8],
	) -> Result<(SessionKeys, SessionKeys)> {
		let shared_secret = self.derive_proxy_shared_secret(
			voucher_device_id,
			target_device_id,
			vouchee_device_id,
			vouchee_public_key,
			base_secret,
		)?;
		let receiver_keys = SessionKeys::from_shared_secret(shared_secret);
		let vouchee_keys = receiver_keys.clone().swap_keys();
		Ok((receiver_keys, vouchee_keys))
	}

	async fn emit_vouching_session(&self, session: &VouchingSession) -> Result<()> {
		let event_bus = { self.event_bus.read().await.clone() };
		let Some(event_bus) = event_bus else {
			return Ok(());
		};

		let resource =
			serde_json::to_value(session).map_err(|e| NetworkingError::Serialization(e))?;

		event_bus.emit(Event::ResourceChanged {
			resource_type: "vouching_session".to_string(),
			resource,
			metadata: Some(ResourceMetadata {
				no_merge_fields: vec!["vouches".to_string()],
				alternate_ids: vec![],
				affected_paths: vec![],
			}),
		});

		Ok(())
	}

	async fn update_vouch_status(
		&self,
		session_id: Uuid,
		device_id: Uuid,
		status: VouchStatus,
		reason: Option<String>,
	) -> Result<()> {
		if matches!(status, VouchStatus::Rejected | VouchStatus::Unreachable) {
			let mut keys = self.vouching_keys.write().await;
			keys.remove(&(session_id, device_id));
		}

		let mut should_finalize = false;
		let session_snapshot = {
			let mut sessions = self.vouching_sessions.write().await;
			let session = sessions.get_mut(&session_id).ok_or_else(|| {
				NetworkingError::Protocol(format!("Vouching session not found: {}", session_id))
			})?;

			if let Some(entry) = session
				.vouches
				.iter_mut()
				.find(|v| v.device_id == device_id)
			{
				entry.status = status;
				entry.reason = reason;
				entry.updated_at = chrono::Utc::now();
			} else {
				session.vouches.push(VouchState {
					device_id,
					device_name: "Unknown device".to_string(),
					status,
					updated_at: chrono::Utc::now(),
					reason,
				});
			}

			let all_terminal = session.vouches.iter().all(|v| {
				matches!(
					v.status,
					VouchStatus::Accepted | VouchStatus::Rejected | VouchStatus::Unreachable
				)
			});

			if all_terminal && !matches!(session.state, VouchingSessionState::Completed) {
				session.state = VouchingSessionState::Completed;
				should_finalize = true;
			}

			session.clone()
		};

		self.emit_vouching_session(&session_snapshot).await?;

		if should_finalize {
			self.finalize_vouching_session(session_id).await?;
		}

		Ok(())
	}

	async fn finalize_vouching_session(&self, session_id: Uuid) -> Result<()> {
		let session = {
			let sessions = self.vouching_sessions.read().await;
			sessions.get(&session_id).cloned().ok_or_else(|| {
				NetworkingError::Protocol(format!("Vouching session not found: {}", session_id))
			})?
		};

		let mut accepted = Vec::new();
		let mut rejected = Vec::new();

		for vouch in &session.vouches {
			match vouch.status {
				VouchStatus::Accepted => {
					let device_info = {
						let registry = self.device_registry.read().await;
						match registry.get_device_state(vouch.device_id) {
							Some(crate::service::network::device::DeviceState::Paired {
								info,
								..
							})
							| Some(crate::service::network::device::DeviceState::Connected {
								info,
								..
							})
							| Some(crate::service::network::device::DeviceState::Disconnected {
								info,
								..
							}) => Some(info.clone()),
							_ => None,
						}
					};

					let session_keys = {
						let keys = self.vouching_keys.read().await;
						keys.get(&(session_id, vouch.device_id)).cloned()
					};

					if let (Some(info), Some(keys)) = (device_info, session_keys) {
						accepted.push(super::proxy::AcceptedDevice {
							device_info: info,
							session_keys: keys,
						});
					} else {
						self.log_warn(&format!(
							"Missing device info or keys for accepted device {}",
							vouch.device_id
						))
						.await;
					}
				}
				VouchStatus::Rejected | VouchStatus::Unreachable => {
					let reason = vouch
						.reason
						.clone()
						.unwrap_or_else(|| "Vouch rejected".to_string());
					rejected.push(super::proxy::RejectedDevice {
						device_id: vouch.device_id,
						device_name: vouch.device_name.clone(),
						reason,
					});
				}
				_ => {}
			}
		}

		let vouchee_node_id = {
			let registry = self.device_registry.read().await;
			registry.get_node_id_for_device(session.vouchee_device_id)
		};

		if let Some(node_id) = vouchee_node_id {
			let message = PairingMessage::ProxyPairingComplete {
				session_id,
				voucher_device_id: session.voucher_device_id,
				accepted_by: accepted,
				rejected_by: rejected,
			};
			self.send_pairing_message_fire_and_forget(node_id, &message)
				.await?;
		} else {
			self.log_warn(&format!(
				"No node ID for vouchee device {}, cannot send completion",
				session.vouchee_device_id
			))
			.await;
		}

		{
			let mut keys = self.vouching_keys.write().await;
			keys.retain(|(sid, _), _| *sid != session_id);
		}

		self.schedule_vouching_cleanup(session_id).await;

		Ok(())
	}

	async fn schedule_vouching_cleanup(&self, session_id: Uuid) {
		let vouching_sessions = self.vouching_sessions.clone();
		let event_bus = self.event_bus.clone();
		let vouching_keys = self.vouching_keys.clone();
		tokio::spawn(async move {
			tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
			{
				let mut sessions = vouching_sessions.write().await;
				sessions.remove(&session_id);
			}
			{
				let mut keys = vouching_keys.write().await;
				keys.retain(|(sid, _), _| *sid != session_id);
			}

			let event_bus = { event_bus.read().await.clone() };
			if let Some(event_bus) = event_bus {
				event_bus.emit(Event::ResourceDeleted {
					resource_type: "vouching_session".to_string(),
					resource_id: session_id,
				});
			}
		});
	}

	pub async fn get_vouching_session(&self, session_id: Uuid) -> Option<VouchingSession> {
		let sessions = self.vouching_sessions.read().await;
		sessions.get(&session_id).cloned()
	}

	pub async fn create_vouching_session(
		&self,
		session_id: Uuid,
		vouchee_device_info: &DeviceInfo,
	) -> Result<()> {
		let voucher_device_id = self.get_device_info().await?.device_id;
		let session = VouchingSession {
			id: session_id,
			vouchee_device_id: vouchee_device_info.device_id,
			vouchee_device_name: vouchee_device_info.device_name.clone(),
			voucher_device_id,
			created_at: chrono::Utc::now(),
			state: VouchingSessionState::Pending,
			vouches: Vec::new(),
		};

		{
			let mut sessions = self.vouching_sessions.write().await;
			sessions.insert(session_id, session.clone());
		}

		self.emit_vouching_session(&session).await?;

		let event_bus = { self.event_bus.read().await.clone() };
		if let Some(event_bus) = event_bus {
			event_bus.emit(Event::ProxyPairingVouchingReady {
				session_id,
				vouchee_device_id: vouchee_device_info.device_id,
			});
		}

		let proxy_config = { self.proxy_config.read().await.clone() };
		if proxy_config.auto_vouch_to_all {
			let target_device_ids = {
				let registry = self.device_registry.read().await;
				registry
					.get_paired_devices()
					.into_iter()
					.map(|device| device.device_id)
					.filter(|device_id| {
						*device_id != voucher_device_id
							&& *device_id != vouchee_device_info.device_id
					})
					.collect::<Vec<_>>()
			};

			if !target_device_ids.is_empty() {
				if let Err(e) = self
					.start_proxy_vouching(session_id, target_device_ids)
					.await
				{
					self.log_warn(&format!(
						"Failed to auto vouch session {}: {}",
						session_id, e
					))
					.await;
				}
			}
		}

		Ok(())
	}

	pub async fn start_proxy_vouching(
		&self,
		session_id: Uuid,
		target_device_ids: Vec<Uuid>,
	) -> Result<VouchingSession> {
		let (vouchee_device_info, vouchee_public_key, shared_secret) = {
			let sessions = self.active_sessions.read().await;
			let session = sessions.get(&session_id).ok_or_else(|| {
				NetworkingError::Protocol(format!("Pairing session not found: {}", session_id))
			})?;

			if !matches!(session.state, PairingState::Completed) {
				return Err(NetworkingError::Protocol(
					"Pairing session is not completed".to_string(),
				));
			}

			let device_info = session.remote_device_info.clone().ok_or_else(|| {
				NetworkingError::Protocol("Missing vouchee device info".to_string())
			})?;
			let public_key = session.remote_public_key.clone().ok_or_else(|| {
				NetworkingError::Protocol("Missing vouchee public key".to_string())
			})?;
			let secret = session.shared_secret.clone();
			(device_info, public_key, secret)
		};

		let voucher_device_id = self.get_device_info().await?.device_id;
		let base_secret = match shared_secret {
			Some(secret) => secret,
			None => self.generate_shared_secret(session_id).await?,
		};

		let now = chrono::Utc::now();
		let initial_vouches = {
			let registry = self.device_registry.read().await;
			target_device_ids
				.iter()
				.map(|device_id| {
					let device_name = match registry.get_device_state(*device_id) {
						Some(crate::service::network::device::DeviceState::Paired {
							info, ..
						})
						| Some(crate::service::network::device::DeviceState::Connected {
							info,
							..
						})
						| Some(crate::service::network::device::DeviceState::Disconnected {
							info,
							..
						}) => info.device_name.clone(),
						_ => "Unknown device".to_string(),
					};
					VouchState {
						device_id: *device_id,
						device_name,
						status: VouchStatus::Selected,
						updated_at: now,
						reason: None,
					}
				})
				.collect::<Vec<_>>()
		};

		let mut session_snapshot = {
			let mut sessions = self.vouching_sessions.write().await;
			let entry = sessions
				.entry(session_id)
				.or_insert_with(|| VouchingSession {
					id: session_id,
					vouchee_device_id: vouchee_device_info.device_id,
					vouchee_device_name: vouchee_device_info.device_name.clone(),
					voucher_device_id,
					created_at: now,
					state: VouchingSessionState::Pending,
					vouches: Vec::new(),
				});

			entry.state = VouchingSessionState::InProgress;
			entry.vouches = initial_vouches;
			entry.clone()
		};

		self.emit_vouching_session(&session_snapshot).await?;

		if target_device_ids.is_empty() {
			{
				let mut sessions = self.vouching_sessions.write().await;
				if let Some(session) = sessions.get_mut(&session_id) {
					session.state = VouchingSessionState::Completed;
					session_snapshot = session.clone();
				}
			}
			self.emit_vouching_session(&session_snapshot).await?;
			self.finalize_vouching_session(session_id).await?;
			return Ok(session_snapshot);
		}

		for target_device_id in target_device_ids {
			if target_device_id == voucher_device_id
				|| target_device_id == vouchee_device_info.device_id
			{
				self.update_vouch_status(
					session_id,
					target_device_id,
					VouchStatus::Rejected,
					Some("Invalid vouch target".to_string()),
				)
				.await?;
				continue;
			}

			let target_device_info = {
				let registry = self.device_registry.read().await;
				match registry.get_device_state(target_device_id) {
					Some(crate::service::network::device::DeviceState::Paired { info, .. })
					| Some(crate::service::network::device::DeviceState::Connected {
						info, ..
					})
					| Some(crate::service::network::device::DeviceState::Disconnected {
						info,
						..
					}) => Some(info.clone()),
					_ => None,
				}
			};

			let Some(target_device_info) = target_device_info else {
				self.update_vouch_status(
					session_id,
					target_device_id,
					VouchStatus::Rejected,
					Some("Target device not paired".to_string()),
				)
				.await?;
				continue;
			};

			let timestamp = chrono::Utc::now();
			let payload = self.build_vouch_payload(
				session_id,
				&vouchee_device_info,
				&vouchee_public_key,
				timestamp,
			);
			let signature = self.sign_vouch_payload(&payload)?;
			let (receiver_keys, vouchee_keys) = self.derive_proxy_session_keys(
				voucher_device_id,
				target_device_id,
				vouchee_device_info.device_id,
				&vouchee_public_key,
				&base_secret,
			)?;

			{
				let mut keys = self.vouching_keys.write().await;
				keys.insert((session_id, target_device_id), vouchee_keys);
			}

			let queue_entry = VouchingQueueEntry {
				session_id,
				target_device_id,
				voucher_device_id,
				vouchee_device_id: vouchee_device_info.device_id,
				vouchee_device_info: vouchee_device_info.clone(),
				vouchee_public_key: vouchee_public_key.clone(),
				voucher_signature: signature.clone(),
				proxied_session_keys: receiver_keys.clone(),
				created_at: timestamp,
				expires_at: timestamp + chrono::Duration::days(7),
				status: VouchQueueStatus::Queued,
				retry_count: 0,
				last_attempt_at: None,
			};

			let queue = { self.vouching_queue.read().await.clone() };
			if let Some(queue) = queue {
				queue.upsert_entry(&queue_entry).await?;
			}

			let mut sent_now = false;
			if let Some(endpoint) = &self.endpoint {
				let registry = self.device_registry.read().await;
				if registry.is_node_connected(endpoint, target_device_id) {
					if let Some(node_id) = registry.get_node_id_for_device(target_device_id) {
						let request = PairingMessage::ProxyPairingRequest {
							session_id,
							vouchee_device_info: vouchee_device_info.clone(),
							vouchee_public_key: vouchee_public_key.clone(),
							voucher_device_id,
							voucher_signature: signature,
							timestamp,
							proxied_session_keys: receiver_keys,
						};
						match self
							.send_pairing_message_fire_and_forget(node_id, &request)
							.await
						{
							Ok(_) => {
								sent_now = true;
							}
							Err(e) => {
								self.log_warn(&format!(
									"Failed to send proxy pairing request to {}: {}",
									target_device_id, e
								))
								.await;
							}
						}
					}
				}
			}

			if sent_now {
				let queue = { self.vouching_queue.read().await.clone() };
				if let Some(queue) = queue {
					queue
						.update_status(
							session_id,
							target_device_id,
							VouchQueueStatus::Waiting,
							1,
							Some(chrono::Utc::now()),
						)
						.await?;
				}
				self.update_vouch_status(session_id, target_device_id, VouchStatus::Waiting, None)
					.await?;
			} else {
				self.update_vouch_status(session_id, target_device_id, VouchStatus::Queued, None)
					.await?;
			}
		}

		let session = self
			.get_vouching_session(session_id)
			.await
			.ok_or_else(|| NetworkingError::Protocol("Vouching session missing".to_string()))?;
		session_snapshot = session.clone();

		Ok(session_snapshot)
	}

	pub async fn confirm_proxy_pairing(&self, session_id: Uuid, accepted: bool) -> Result<()> {
		let pending = {
			let mut pending = self.pending_proxy_confirmations.write().await;
			pending.remove(&session_id)
		};

		let Some(pending) = pending else {
			return Err(NetworkingError::Protocol(
				"No pending proxy confirmation found".to_string(),
			));
		};

		let accepting_device_id = self.get_device_info().await?.device_id;
		let voucher_node_id = {
			let registry = self.device_registry.read().await;
			registry.get_node_id_for_device(pending.voucher_device_id)
		};

		if accepted {
			{
				let mut registry = self.device_registry.write().await;
				registry
					.complete_pairing(
						pending.vouchee_device_info.device_id,
						pending.vouchee_device_info.clone(),
						pending.proxied_session_keys.clone(),
						None,
						crate::service::network::device::PairingType::Proxied,
						Some(pending.voucher_device_id),
						Some(chrono::Utc::now()),
					)
					.await?;
			}

			if let Some(node_id) = voucher_node_id {
				let response = PairingMessage::ProxyPairingResponse {
					session_id,
					accepting_device_id,
					accepted: true,
					reason: None,
				};
				self.send_pairing_message_fire_and_forget(node_id, &response)
					.await?;
			}
		} else if let Some(node_id) = voucher_node_id {
			let response = PairingMessage::ProxyPairingResponse {
				session_id,
				accepting_device_id,
				accepted: false,
				reason: Some("User rejected proxy pairing".to_string()),
			};
			self.send_pairing_message_fire_and_forget(node_id, &response)
				.await?;
		}

		Ok(())
	}

	async fn handle_proxy_pairing_request(
		&self,
		session_id: Uuid,
		vouchee_device_info: DeviceInfo,
		vouchee_public_key: Vec<u8>,
		voucher_device_id: Uuid,
		voucher_signature: Vec<u8>,
		timestamp: chrono::DateTime<chrono::Utc>,
		proxied_session_keys: SessionKeys,
		remote_node_id: NodeId,
	) -> Result<()> {
		let proxy_config = { self.proxy_config.read().await.clone() };

		let (voucher_info, voucher_node_id) = {
			let registry = self.device_registry.read().await;
			let voucher_info = match registry.get_device_state(voucher_device_id) {
				Some(crate::service::network::device::DeviceState::Paired { info, .. })
				| Some(crate::service::network::device::DeviceState::Connected { info, .. })
				| Some(crate::service::network::device::DeviceState::Disconnected {
					info, ..
				}) => Some(info.clone()),
				_ => None,
			};
			(
				voucher_info,
				registry.get_node_id_for_device(voucher_device_id),
			)
		};

		if let Some(node_id) = voucher_node_id {
			if node_id != remote_node_id {
				self.send_proxy_pairing_rejection(
					remote_node_id,
					session_id,
					"Voucher node mismatch".to_string(),
				)
				.await?;
				return Ok(());
			}
		}

		let Some(voucher_info) = voucher_info else {
			self.send_proxy_pairing_rejection(
				remote_node_id,
				session_id,
				"Voucher not paired".to_string(),
			)
			.await?;
			return Ok(());
		};

		let payload = self.build_vouch_payload(
			session_id,
			&vouchee_device_info,
			&vouchee_public_key,
			timestamp,
		);

		PairingSecurity::validate_public_key(&vouchee_public_key)?;

		if !self.verify_vouch_signature(&payload, &voucher_signature, remote_node_id.as_bytes())? {
			self.send_proxy_pairing_rejection(
				remote_node_id,
				session_id,
				"Invalid voucher signature".to_string(),
			)
			.await?;
			return Ok(());
		}

		let max_age = chrono::Duration::seconds(proxy_config.vouch_signature_max_age as i64);
		if chrono::Utc::now().signed_duration_since(timestamp) > max_age {
			self.send_proxy_pairing_rejection(
				remote_node_id,
				session_id,
				"Vouch signature expired".to_string(),
			)
			.await?;
			return Ok(());
		}

		{
			let registry = self.device_registry.read().await;
			if registry
				.get_device_state(vouchee_device_info.device_id)
				.is_some()
			{
				self.send_proxy_pairing_rejection(
					remote_node_id,
					session_id,
					"Device already paired".to_string(),
				)
				.await?;
				return Ok(());
			}
		}

		let persistence = {
			let registry = self.device_registry.read().await;
			registry.persistence()
		};
		let persisted_voucher = persistence.get_paired_device(voucher_device_id).await?;

		let Some(persisted_voucher) = persisted_voucher else {
			self.send_proxy_pairing_rejection(
				remote_node_id,
				session_id,
				"Voucher not in persistence".to_string(),
			)
			.await?;
			return Ok(());
		};

		let voucher_is_trusted = matches!(
			persisted_voucher.trust_level,
			crate::service::network::device::TrustLevel::Trusted
		);
		let voucher_is_direct = matches!(
			persisted_voucher.pairing_type,
			crate::service::network::device::PairingType::Direct
		);

		if !voucher_is_trusted || !voucher_is_direct {
			self.send_proxy_pairing_rejection(
				remote_node_id,
				session_id,
				"Voucher not trusted for proxy pairing".to_string(),
			)
			.await?;
			return Ok(());
		}

		if proxied_session_keys.send_key == proxied_session_keys.receive_key {
			self.send_proxy_pairing_rejection(
				remote_node_id,
				session_id,
				"Invalid session keys".to_string(),
			)
			.await?;
			return Ok(());
		}

		if proxy_config.auto_accept_vouched && voucher_is_trusted {
			{
				let mut registry = self.device_registry.write().await;
				registry
					.complete_pairing(
						vouchee_device_info.device_id,
						vouchee_device_info.clone(),
						proxied_session_keys.clone(),
						None,
						crate::service::network::device::PairingType::Proxied,
						Some(voucher_device_id),
						Some(chrono::Utc::now()),
					)
					.await?;
			}

			let accepting_device_id = self.get_device_info().await?.device_id;
			let response = PairingMessage::ProxyPairingResponse {
				session_id,
				accepting_device_id,
				accepted: true,
				reason: None,
			};
			self.send_pairing_message_fire_and_forget(remote_node_id, &response)
				.await?;
			return Ok(());
		}

		let pending = PendingProxyConfirmation {
			session_id,
			voucher_device_id,
			voucher_device_name: voucher_info.device_name.clone(),
			vouchee_device_info: vouchee_device_info.clone(),
			vouchee_public_key: vouchee_public_key.clone(),
			proxied_session_keys,
			created_at: chrono::Utc::now(),
		};

		{
			let mut pending_map = self.pending_proxy_confirmations.write().await;
			pending_map.insert(session_id, pending);
		}

		let event_bus = { self.event_bus.read().await.clone() };
		if let Some(event_bus) = event_bus {
			let expires_at = (chrono::Utc::now()
				+ chrono::Duration::seconds(proxy_config.vouch_response_timeout as i64))
			.to_rfc3339();
			event_bus.emit(Event::ProxyPairingConfirmationRequired {
				session_id,
				vouchee_device_name: vouchee_device_info.device_name.clone(),
				vouchee_device_os: vouchee_device_info.os_version.clone(),
				voucher_device_name: voucher_info.device_name,
				voucher_device_id,
				expires_at,
			});
		}

		let pending_map = self.pending_proxy_confirmations.clone();
		let command_sender = self.command_sender.clone();
		let registry = self.device_registry.clone();
		let timeout = proxy_config.vouch_response_timeout;
		let accepting_device_id = self.get_device_info().await?.device_id;

		tokio::spawn(async move {
			tokio::time::sleep(tokio::time::Duration::from_secs(timeout)).await;
			let pending = {
				let mut guard = pending_map.write().await;
				guard.remove(&session_id)
			};

			if let Some(pending) = pending {
				let node_id = {
					let registry = registry.read().await;
					registry.get_node_id_for_device(pending.voucher_device_id)
				};
				if let Some(node_id) = node_id {
					if let Ok(data) = serde_json::to_vec(&PairingMessage::ProxyPairingResponse {
						session_id,
						accepting_device_id,
						accepted: false,
						reason: Some("Proxy confirmation timed out".to_string()),
					}) {
						let _ = command_sender.send(
							crate::service::network::core::event_loop::EventLoopCommand::SendMessageToNode {
								node_id,
								protocol: "pairing".to_string(),
								data,
							},
						);
					}
				}
			}
		});

		Ok(())
	}

	async fn send_proxy_pairing_rejection(
		&self,
		remote_node_id: NodeId,
		session_id: Uuid,
		reason: String,
	) -> Result<()> {
		let accepting_device_id = self.get_device_info().await?.device_id;
		let response = PairingMessage::ProxyPairingResponse {
			session_id,
			accepting_device_id,
			accepted: false,
			reason: Some(reason),
		};
		self.send_pairing_message_fire_and_forget(remote_node_id, &response)
			.await
	}

	async fn handle_proxy_pairing_response(
		&self,
		session_id: Uuid,
		accepting_device_id: Uuid,
		accepted: bool,
		reason: Option<String>,
	) -> Result<()> {
		if self.get_vouching_session(session_id).await.is_none() {
			self.log_warn(&format!(
				"Proxy pairing response for unknown session {}",
				session_id
			))
			.await;
			return Ok(());
		}

		let status = if accepted {
			VouchStatus::Accepted
		} else {
			VouchStatus::Rejected
		};

		if !accepted {
			let mut keys = self.vouching_keys.write().await;
			keys.remove(&(session_id, accepting_device_id));
		}

		let queue = { self.vouching_queue.read().await.clone() };
		if let Some(queue) = queue {
			queue.remove_entry(session_id, accepting_device_id).await?;
		}

		self.update_vouch_status(session_id, accepting_device_id, status, reason)
			.await?;

		Ok(())
	}

	async fn handle_proxy_pairing_complete(
		&self,
		session_id: Uuid,
		voucher_device_id: Uuid,
		accepted_by: Vec<super::proxy::AcceptedDevice>,
		rejected_by: Vec<super::proxy::RejectedDevice>,
	) -> Result<()> {
		for accepted in accepted_by {
			let device_id = accepted.device_info.device_id;
			let mut registry = self.device_registry.write().await;
			registry
				.complete_pairing(
					device_id,
					accepted.device_info.clone(),
					accepted.session_keys.clone(),
					None,
					crate::service::network::device::PairingType::Proxied,
					Some(voucher_device_id),
					Some(chrono::Utc::now()),
				)
				.await?;
		}

		if !rejected_by.is_empty() {
			self.log_info(&format!(
				"Proxy pairing completed with {} rejections",
				rejected_by.len()
			))
			.await;
		}

		self.log_info(&format!(
			"Proxy pairing completion handled for session {}",
			session_id
		))
		.await;

		Ok(())
	}

	async fn process_vouching_queue(&self) -> Result<()> {
		let queue = { self.vouching_queue.read().await.clone() };
		let Some(queue) = queue else {
			return Ok(());
		};

		let config = { self.proxy_config.read().await.clone() };
		let entries = queue.list_entries().await?;
		let now = chrono::Utc::now();

		for entry in entries {
			if self.get_vouching_session(entry.session_id).await.is_none() {
				queue
					.remove_entry(entry.session_id, entry.target_device_id)
					.await?;
				continue;
			}

			if entry.expires_at <= now {
				queue
					.remove_entry(entry.session_id, entry.target_device_id)
					.await?;
				self.update_vouch_status(
					entry.session_id,
					entry.target_device_id,
					VouchStatus::Unreachable,
					Some("Vouch expired".to_string()),
				)
				.await?;
				continue;
			}

			if entry.retry_count >= config.vouch_queue_retry_limit {
				queue
					.remove_entry(entry.session_id, entry.target_device_id)
					.await?;
				self.update_vouch_status(
					entry.session_id,
					entry.target_device_id,
					VouchStatus::Unreachable,
					Some("Vouch retry limit exceeded".to_string()),
				)
				.await?;
				continue;
			}

			if matches!(entry.status, VouchQueueStatus::Waiting) {
				if let Some(last_attempt_at) = entry.last_attempt_at {
					let timeout = chrono::Duration::seconds(config.vouch_response_timeout as i64);
					if now.signed_duration_since(last_attempt_at) > timeout {
						queue
							.remove_entry(entry.session_id, entry.target_device_id)
							.await?;
						self.update_vouch_status(
							entry.session_id,
							entry.target_device_id,
							VouchStatus::Unreachable,
							Some("Proxy response timeout".to_string()),
						)
						.await?;
					}
				}
				continue;
			}

			if !matches!(entry.status, VouchQueueStatus::Queued) {
				continue;
			}

			let endpoint = match &self.endpoint {
				Some(endpoint) => endpoint,
				None => continue,
			};

			let (is_connected, node_id) = {
				let registry = self.device_registry.read().await;
				(
					registry.is_node_connected(endpoint, entry.target_device_id),
					registry.get_node_id_for_device(entry.target_device_id),
				)
			};

			if !is_connected {
				continue;
			}

			let Some(node_id) = node_id else {
				continue;
			};

			let timestamp = chrono::Utc::now();
			let payload = self.build_vouch_payload(
				entry.session_id,
				&entry.vouchee_device_info,
				&entry.vouchee_public_key,
				timestamp,
			);
			let signature = self.sign_vouch_payload(&payload)?;

			let request = PairingMessage::ProxyPairingRequest {
				session_id: entry.session_id,
				vouchee_device_info: entry.vouchee_device_info.clone(),
				vouchee_public_key: entry.vouchee_public_key.clone(),
				voucher_device_id: entry.voucher_device_id,
				voucher_signature: signature,
				timestamp,
				proxied_session_keys: entry.proxied_session_keys.clone(),
			};

			if let Err(e) = self
				.send_pairing_message_fire_and_forget(node_id, &request)
				.await
			{
				self.log_warn(&format!(
					"Failed to send queued proxy pairing request to {}: {}",
					entry.target_device_id, e
				))
				.await;
				queue
					.update_status(
						entry.session_id,
						entry.target_device_id,
						VouchQueueStatus::Queued,
						entry.retry_count + 1,
						Some(now),
					)
					.await?;
				continue;
			}

			queue
				.update_status(
					entry.session_id,
					entry.target_device_id,
					VouchQueueStatus::Waiting,
					entry.retry_count + 1,
					Some(now),
				)
				.await?;

			self.update_vouch_status(
				entry.session_id,
				entry.target_device_id,
				VouchStatus::Waiting,
				None,
			)
			.await?;
		}

		Ok(())
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
			PairingMessage::Reject { session_id, reason } => {
				self.handle_rejection(session_id, reason).await?;
				Ok(None) // No response needed
			}
			PairingMessage::ProxyPairingRequest {
				session_id,
				vouchee_device_info,
				vouchee_public_key,
				voucher_device_id,
				voucher_signature,
				timestamp,
				proxied_session_keys,
			} => {
				self.handle_proxy_pairing_request(
					session_id,
					vouchee_device_info,
					vouchee_public_key,
					voucher_device_id,
					voucher_signature,
					timestamp,
					proxied_session_keys,
					remote_node_id,
				)
				.await?;
				Ok(None)
			}
			PairingMessage::ProxyPairingResponse {
				session_id,
				accepting_device_id,
				accepted,
				reason,
			} => {
				self.handle_proxy_pairing_response(
					session_id,
					accepting_device_id,
					accepted,
					reason,
				)
				.await?;
				Ok(None)
			}
			PairingMessage::ProxyPairingComplete {
				session_id,
				voucher_device_id,
				accepted_by,
				rejected_by,
			} => {
				self.handle_proxy_pairing_complete(
					session_id,
					voucher_device_id,
					accepted_by,
					rejected_by,
				)
				.await?;
				Ok(None)
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
	/// This implementation follows Iroh best practices:
	/// - Reuses persistent connections (cached in self.connections)
	/// - Creates a new stream for each message exchange
	/// - Keeps connections alive for future messages
	pub async fn send_pairing_message_to_node(
		&self,
		endpoint: &Endpoint,
		node_id: NodeId,
		message: &PairingMessage,
	) -> Result<Option<PairingMessage>> {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		let conn = utils::get_or_create_connection(
			self.connections.clone(),
			endpoint,
			node_id,
			crate::service::network::core::PAIRING_ALPN,
			&self.logger,
		)
		.await?;

		let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
			NetworkingError::ConnectionFailed(format!("Failed to open stream: {}", e))
		})?;

		let msg_data =
			serde_json::to_vec(message).map_err(|e| NetworkingError::Serialization(e))?;

		let len = msg_data.len() as u32;
		send.write_all(&len.to_be_bytes())
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to write length: {}", e)))?;

		send.write_all(&msg_data)
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to write message: {}", e)))?;

		send.finish()
			.map_err(|e| NetworkingError::Transport(format!("Failed to finish stream: {}", e)))?;

		let mut len_buf = [0u8; 4];
		match recv.read_exact(&mut len_buf).await {
			Ok(_) => {
				let resp_len = u32::from_be_bytes(len_buf) as usize;

				if resp_len > MAX_MESSAGE_SIZE {
					return Err(NetworkingError::Protocol(format!(
						"Message too large: {} bytes (max: {} bytes)",
						resp_len, MAX_MESSAGE_SIZE
					)));
				}

				let mut resp_buf = vec![0u8; resp_len];
				recv.read_exact(&mut resp_buf).await.map_err(|e| {
					NetworkingError::Transport(format!("Failed to read response: {}", e))
				})?;

				let response: PairingMessage = serde_json::from_slice(&resp_buf)
					.map_err(|e| NetworkingError::Serialization(e))?;

				Ok(Some(response))
			}
			Err(_) => Ok(None),
		}
	}

	pub async fn send_pairing_message_fire_and_forget(
		&self,
		node_id: NodeId,
		message: &PairingMessage,
	) -> Result<()> {
		let data = serde_json::to_vec(message).map_err(NetworkingError::Serialization)?;
		self.command_sender
			.send(
				crate::service::network::core::event_loop::EventLoopCommand::SendMessageToNode {
					node_id,
					protocol: "pairing".to_string(),
					data,
				},
			)
			.map_err(|_| NetworkingError::Protocol("Pairing command channel closed".to_string()))?;
		Ok(())
	}
}

#[async_trait]
impl ProtocolHandler for PairingProtocolHandler {
	fn protocol_name(&self) -> &str {
		"pairing"
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
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

			// Validate message size to prevent DoS attacks
			if msg_len > MAX_MESSAGE_SIZE {
				self.logger
					.error(&format!(
						"Rejecting oversized message: {} bytes (max: {} bytes)",
						msg_len, MAX_MESSAGE_SIZE
					))
					.await;
				break;
			}

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
						PairingMessage::Reject { .. } => "Reject",
						PairingMessage::ProxyPairingRequest { .. } => "ProxyPairingRequest",
						PairingMessage::ProxyPairingResponse { .. } => "ProxyPairingResponse",
						PairingMessage::ProxyPairingComplete { .. } => "ProxyPairingComplete",
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

			// Check if this was a completion or rejection message - if so, we can close the stream
			if matches!(
				message,
				PairingMessage::Complete { .. } | PairingMessage::Reject { .. }
			) {
				self.logger
					.info("Received Complete/Reject message, closing pairing stream")
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
			PairingMessage::ProxyPairingRequest { .. }
			| PairingMessage::ProxyPairingResponse { .. }
			| PairingMessage::ProxyPairingComplete { .. }
			| PairingMessage::Challenge { .. }
			| PairingMessage::Complete { .. } => {
				self.log_warn(
					"Received message in handle_request - this should be handled by stream",
				)
				.await;
				Ok(Vec::new())
			}
			// Reject messages are handled by handle_response, not here
			PairingMessage::Reject { session_id, reason } => {
				self.log_warn(&format!(
					"Received Reject in handle_request for session {}: {}",
					session_id, reason
				))
				.await;
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
					PairingMessage::Reject { session_id, .. } => Some(session_id),
					PairingMessage::ProxyPairingRequest { session_id, .. } => Some(session_id),
					PairingMessage::ProxyPairingResponse { session_id, .. } => Some(session_id),
					PairingMessage::ProxyPairingComplete { session_id, .. } => Some(session_id),
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

						// Send Response and wait for Complete message
						self.log_info(&format!(
							"Sending Response to node {} and waiting for Complete message",
							from_node
						))
						.await;

						// Get endpoint
						let endpoint = match &self.endpoint {
							Some(ep) => ep,
							None => {
								self.log_error("No endpoint available to send Response")
									.await;
								let mut sessions = self.active_sessions.write().await;
								if let Some(session) = sessions.get_mut(&session_id) {
									session.state = PairingState::Failed {
										reason: "No endpoint available".to_string(),
									};
								}
								return Ok(());
							}
						};

						// Deserialize the Response message
						let response_message: PairingMessage =
							match serde_json::from_slice(&response_data) {
								Ok(msg) => msg,
								Err(e) => {
									self.log_error(&format!(
										"Failed to deserialize Response message: {}",
										e
									))
									.await;
									let mut sessions = self.active_sessions.write().await;
									if let Some(session) = sessions.get_mut(&session_id) {
										session.state = PairingState::Failed {
											reason: "Failed to deserialize Response".to_string(),
										};
									}
									return Ok(());
								}
							};

						// Send Response and wait for Complete
						match self
							.send_pairing_message_to_node(endpoint, from_node, &response_message)
							.await
						{
							Ok(Some(PairingMessage::Complete {
								session_id: complete_session_id,
								success,
								reason,
							})) => {
								self.log_info(&format!(
									"Received Complete message for session {} - success: {}",
									complete_session_id, success
								))
								.await;

								// Process the Complete message
								if let Err(e) = self
									.handle_completion(
										complete_session_id,
										success,
										reason,
										from_device,
										from_node,
									)
									.await
								{
									self.log_error(&format!(
										"Failed to process Complete message: {}",
										e
									))
									.await;
								}
							}
							Ok(Some(_other_msg)) => {
								self.log_error(
									"Expected Complete message but received different message type",
								)
								.await;
								let mut sessions = self.active_sessions.write().await;
								if let Some(session) = sessions.get_mut(&session_id) {
									session.state = PairingState::Failed {
										reason: "Unexpected response type".to_string(),
									};
								}
							}
							Ok(None) => {
								self.log_error("No Complete message received from initiator")
									.await;
								let mut sessions = self.active_sessions.write().await;
								if let Some(session) = sessions.get_mut(&session_id) {
									session.state = PairingState::Failed {
										reason: "No Complete message received".to_string(),
									};
								}
							}
							Err(e) => {
								self.log_error(&format!(
									"Failed to send Response or receive Complete: {}",
									e
								))
								.await;
								let mut sessions = self.active_sessions.write().await;
								if let Some(session) = sessions.get_mut(&session_id) {
									session.state = PairingState::Failed {
										reason: format!("Send failed: {}", e),
									};
								}
							}
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
			PairingMessage::ProxyPairingRequest { .. }
			| PairingMessage::ProxyPairingResponse { .. }
			| PairingMessage::ProxyPairingComplete { .. }
			| PairingMessage::PairingRequest { .. }
			| PairingMessage::Response { .. } => {
				self.log_warn("Received message in handle_response - this should be handled by handle_request or stream").await;
			}
			// Handle rejection from initiator (joiner receives this)
			PairingMessage::Reject { session_id, reason } => {
				self.log_info(&format!(
					"Received Reject for session {} - reason: {}",
					session_id, reason
				))
				.await;
				self.handle_rejection(session_id, reason).await;
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
}
