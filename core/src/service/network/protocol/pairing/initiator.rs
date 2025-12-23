//! Initiator-specific pairing logic

use super::{
	messages::PairingMessage,
	security::PairingSecurity,
	types::{PairingSession, PairingState},
	PairingProtocolHandler,
};
use crate::service::network::{
	core::NetworkEvent,
	device::{DeviceInfo, SessionKeys},
	NetworkingError, Result,
};
use chrono::{Duration, Utc};
use iroh::{NodeId, Watcher};
use uuid::Uuid;

/// Timeout for user confirmation in seconds
const CONFIRMATION_TIMEOUT_SECS: i64 = 60;

impl PairingProtocolHandler {
	/// Handle an incoming pairing request (Initiator receives this from Joiner)
	///
	/// Instead of immediately sending a challenge, this now transitions to
	/// AwaitingUserConfirmation state and emits an event for the UI to display
	/// a confirmation dialog.
	pub(crate) async fn handle_pairing_request(
		&self,
		from_device: Uuid,
		session_id: Uuid,
		device_info: DeviceInfo,
		public_key: Vec<u8>,
	) -> Result<Vec<u8>> {
		// Validate the public key format first
		super::security::PairingSecurity::validate_public_key(&public_key)?;
		self.log_info(&format!(
			"Received pairing request from device '{}' for session {}",
			device_info.device_name, session_id
		))
		.await;

		// Generate challenge (we'll store it for later use after confirmation)
		let challenge = self.generate_challenge()?;
		self.log_debug(&format!(
			"Generated challenge of {} bytes for session {}",
			challenge.len(),
			session_id
		))
		.await;

		// Generate 2-digit confirmation code
		let confirmation_code = PairingSecurity::generate_confirmation_code();
		let expires_at = Utc::now() + Duration::seconds(CONFIRMATION_TIMEOUT_SECS);

		self.log_info(&format!(
			"Generated confirmation code {} for device '{}' (expires in {}s)",
			confirmation_code, device_info.device_name, CONFIRMATION_TIMEOUT_SECS
		))
		.await;

		// Hold the write lock for the entire duration to prevent any scoping issues
		let mut sessions = self.active_sessions.write().await;
		self.log_debug(&format!(
			"INITIATOR_HANDLER_DEBUG: Looking for session {} in {} total sessions",
			session_id,
			sessions.len()
		))
		.await;

		if let Some(existing_session) = sessions.get_mut(&session_id) {
			self.log_debug(&format!(
				"INITIATOR_HANDLER_DEBUG: Found existing session {} in state {:?}",
				session_id, existing_session.state
			))
			.await;
			self.log_debug(&format!(
				"Transitioning existing session {} to AwaitingUserConfirmation",
				session_id
			))
			.await;

			// Update the existing session to await user confirmation
			existing_session.state = PairingState::AwaitingUserConfirmation {
				confirmation_code: confirmation_code.clone(),
				expires_at,
			};
			existing_session.remote_device_id = Some(from_device);
			existing_session.remote_device_info = Some(device_info.clone());
			existing_session.remote_public_key = Some(public_key.clone());
			existing_session.confirmation_code = Some(confirmation_code.clone());
			existing_session.confirmation_expires_at = Some(expires_at);
			existing_session.pending_challenge = Some(challenge.clone());
		} else {
			self.log_debug(&format!(
				"INITIATOR_HANDLER_DEBUG: No existing session found for {}, creating new session",
				session_id
			))
			.await;
			self.log_debug(&format!(
				"Creating new session {} for pairing request",
				session_id
			))
			.await;

			// Create new session only if none exists
			let session = PairingSession {
				id: session_id,
				state: PairingState::AwaitingUserConfirmation {
					confirmation_code: confirmation_code.clone(),
					expires_at,
				},
				remote_device_id: Some(from_device),
				remote_device_info: Some(device_info.clone()),
				remote_public_key: Some(public_key.clone()),
				shared_secret: None,
				created_at: chrono::Utc::now(),
				confirmation_code: Some(confirmation_code.clone()),
				confirmation_expires_at: Some(expires_at),
				pending_challenge: Some(challenge.clone()),
			};

			sessions.insert(session_id, session);
		}
		// Write lock is automatically released here

		// Emit event for UI to show confirmation dialog
		self.emit_pairing_confirmation_required(
			session_id,
			device_info.clone(),
			confirmation_code.clone(),
			expires_at,
		)
		.await;

		// Return empty response - we don't send the challenge until user confirms
		// The joiner will wait for either a Challenge or Reject message
		self.log_info(&format!(
			"Pairing request from '{}' awaiting user confirmation (code: {})",
			device_info.device_name, confirmation_code
		))
		.await;

		// We need to return something to the joiner to indicate we received their request
		// but are waiting for user confirmation. We'll send an acknowledgment response.
		// Note: The actual Challenge will be sent when user confirms via confirm_pairing_request
		let ack_response = PairingMessage::Complete {
			session_id,
			success: false,
			reason: Some("Awaiting user confirmation".to_string()),
		};

		// Actually, we should NOT send anything yet - the joiner should wait.
		// Let's return an empty vec to indicate no immediate response.
		// The UI will call confirm_pairing_request which will send the actual challenge.
		Ok(Vec::new())
	}

	/// Emit event for UI to show confirmation dialog
	async fn emit_pairing_confirmation_required(
		&self,
		session_id: Uuid,
		device_info: DeviceInfo,
		confirmation_code: String,
		expires_at: chrono::DateTime<Utc>,
	) {
		// Send event to NetworkingService event bus
		let event = NetworkEvent::PairingConfirmationRequired {
			session_id,
			device_name: device_info.device_name.clone(),
			device_os: device_info.os_version.clone(),
			confirmation_code,
			expires_at,
		};

		// The event will be picked up by NetworkEventBridge and translated to a core Event
		if let Err(_) = self.event_sender.send(event) {
			self.log_warn("Failed to emit PairingConfirmationRequired event (no listeners)")
				.await;
		}
	}

	/// Handle user confirmation of a pairing request
	///
	/// Called when the user accepts or rejects a pairing request from the UI.
	pub async fn handle_user_confirmation(
		&self,
		session_id: Uuid,
		accepted: bool,
	) -> Result<Option<Vec<u8>>> {
		self.log_info(&format!(
			"User {} pairing request for session {}",
			if accepted { "accepted" } else { "rejected" },
			session_id
		))
		.await;

		let mut sessions = self.active_sessions.write().await;
		let session = sessions.get_mut(&session_id).ok_or_else(|| {
			NetworkingError::Protocol(format!("Session {} not found", session_id))
		})?;

		// Verify session is in AwaitingUserConfirmation state
		let (confirmation_code, expires_at) = match &session.state {
			PairingState::AwaitingUserConfirmation {
				confirmation_code,
				expires_at,
			} => (confirmation_code.clone(), *expires_at),
			other => {
				return Err(NetworkingError::Protocol(format!(
					"Session {} is not awaiting user confirmation, state: {:?}",
					session_id, other
				)));
			}
		};

		// Check if confirmation has expired
		if Utc::now() > expires_at {
			session.state = PairingState::Failed {
				reason: "Confirmation timeout".to_string(),
			};
			return Err(NetworkingError::Protocol(
				"Confirmation timeout - please try again".to_string(),
			));
		}

		if !accepted {
			// User rejected - mark session as rejected and prepare reject message
			let device_name = session
				.remote_device_info
				.as_ref()
				.map(|i| i.device_name.clone())
				.unwrap_or_else(|| "Unknown device".to_string());

			session.state = PairingState::Rejected {
				reason: "User rejected pairing request".to_string(),
			};

			self.log_info(&format!(
				"Pairing request from '{}' rejected by user",
				device_name
			))
			.await;

			// Prepare reject message to send to joiner
			let reject_msg = PairingMessage::Reject {
				session_id,
				reason: "Pairing request rejected by user".to_string(),
			};

			return Ok(Some(
				serde_json::to_vec(&reject_msg).map_err(|e| NetworkingError::Serialization(e))?,
			));
		}

		// User accepted - proceed with challenge
		let challenge = session.pending_challenge.clone().ok_or_else(|| {
			NetworkingError::Protocol("No pending challenge found for session".to_string())
		})?;

		let device_info = session
			.remote_device_info
			.as_ref()
			.ok_or_else(|| {
				NetworkingError::Protocol("No remote device info found for session".to_string())
			})?
			.clone();

		// Transition to ChallengeReceived state
		session.state = PairingState::ChallengeReceived {
			challenge: challenge.clone(),
		};

		// Clear confirmation fields
		session.confirmation_code = None;
		session.confirmation_expires_at = None;
		session.pending_challenge = None;

		drop(sessions); // Release lock before async operations

		// Get local device info for the challenge message
		let local_device_info = self.get_device_info().await.map_err(|e| {
			NetworkingError::Protocol(format!("Failed to get initiator device info: {}", e))
		})?;

		// Prepare challenge message
		let challenge_msg = PairingMessage::Challenge {
			session_id,
			challenge: challenge.clone(),
			device_info: local_device_info,
		};

		self.log_info(&format!(
			"User confirmed pairing with '{}', sending Challenge",
			device_info.device_name
		))
		.await;

		Ok(Some(
			serde_json::to_vec(&challenge_msg).map_err(|e| NetworkingError::Serialization(e))?,
		))
	}

	/// Handle a pairing response (Initiator receives this from Joiner)
	/// Initiator verifies joiner's signature and sends Complete message
	pub(crate) async fn handle_pairing_response(
		&self,
		from_device: Uuid,
		session_id: Uuid,
		response: Vec<u8>,
		device_info: DeviceInfo,
	) -> Result<Vec<u8>> {
		// Get session and validate state
		let session = self
			.active_sessions
			.read()
			.await
			.get(&session_id)
			.cloned()
			.ok_or_else(|| NetworkingError::Protocol("Session not found".to_string()))?;

		let challenge = match &session.state {
			PairingState::ChallengeReceived { challenge } => challenge.clone(),
			_ => {
				return Err(NetworkingError::Protocol(format!(
					"Invalid session state: expected ChallengeReceived, got {:?}",
					session.state
				)))
			}
		};

		// Get joiner's public key (stored during pairing request)
		let device_public_key = session
			.remote_public_key
			.as_ref()
			.ok_or_else(|| {
				NetworkingError::Protocol(
					"Device public key not found in session for signature verification".to_string(),
				)
			})?
			.clone();

		// Validate inputs
		PairingSecurity::validate_challenge(&challenge)?;
		PairingSecurity::validate_signature(&response)?;
		PairingSecurity::validate_public_key(&device_public_key)?;

		// Verify joiner's signature on the challenge
		let signature_valid =
			PairingSecurity::verify_challenge_response(&device_public_key, &challenge, &response)?;

		if !signature_valid {
			self.log_error(&format!(
				"Invalid signature for session {} from device {}",
				session_id, from_device
			))
			.await;

			// Mark session as failed
			{
				let mut sessions = self.active_sessions.write().await;
				if let Some(session) = sessions.get_mut(&session_id) {
					session.state = PairingState::Failed {
						reason: "Invalid challenge signature".to_string(),
					};
				}
			}

			// Send failure Complete message to joiner
			let failure_response = PairingMessage::Complete {
				session_id,
				success: false,
				reason: Some("Challenge signature verification failed".to_string()),
			};

			return serde_json::to_vec(&failure_response)
				.map_err(|e| NetworkingError::Serialization(e));
		}

		self.log_info(&format!(
			"Signature verified successfully for session {} from device {}",
			session_id, from_device
		))
		.await;

		// Signature is valid - complete pairing on Initiator's side
		let shared_secret = self.generate_shared_secret(session_id).await?;
		let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

		let actual_device_id = device_info.device_id;
		let node_id = match device_info.network_fingerprint.node_id.parse::<NodeId>() {
			Ok(id) => id,
			Err(_) => {
				self.log_warn("Failed to parse node ID from device info, using fallback")
					.await;
				NodeId::from_bytes(&[0u8; 32]).unwrap()
			}
		};

		// Register joiner's device in Pairing state
		{
			let mut registry = self.device_registry.write().await;
			let node_addr = iroh::NodeAddr::new(node_id);

			registry
				.start_pairing(actual_device_id, node_id, session_id, node_addr)
				.map_err(|e| {
					self.log_warn(&format!(
						"Warning: Could not register device in Pairing state: {}",
						e
					));
					e
				})
				.ok();
		}

		// Get relay URL from endpoint for caching (enables reconnection via relay)
		let relay_url = self
			.endpoint
			.as_ref()
			.and_then(|ep| ep.home_relay().get().into_iter().next())
			.map(|r| r.to_string());

		// Complete pairing in device registry
		{
			let mut registry = self.device_registry.write().await;
			registry
				.complete_pairing(
					actual_device_id,
					device_info.clone(),
					session_keys,
					relay_url,
				)
				.await?;
		}

		// Mark joiner as connected
		{
			let simple_connection = crate::service::network::device::ConnectionInfo {
				latency_ms: None,
				rx_bytes: 0,
				tx_bytes: 0,
			};

			let mut registry = self.device_registry.write().await;
			if let Err(e) = registry
				.mark_connected(actual_device_id, simple_connection)
				.await
			{
				self.log_warn(&format!(
					"Warning: Failed to mark device as connected: {}",
					e
				))
				.await;
			} else {
				self.log_info(&format!(
					"Successfully marked device {} as connected",
					actual_device_id
				))
				.await;
			}
		}

		// Update session to Completed
		{
			let mut sessions = self.active_sessions.write().await;
			if let Some(session) = sessions.get_mut(&session_id) {
				session.state = PairingState::Completed;
				session.shared_secret = Some(shared_secret);
				session.remote_device_id = Some(actual_device_id);
				self.log_info(&format!(
					"Session {} completed on Initiator's side for device {}",
					session_id, actual_device_id
				))
				.await;
			}
		}

		// Send success Complete message to joiner
		// If this fails to serialize, the error propagates and joiner never receives confirmation
		let success_response = PairingMessage::Complete {
			session_id,
			success: true,
			reason: None,
		};

		self.log_info(&format!(
			"Sending success Complete message for session {}",
			session_id
		))
		.await;

		serde_json::to_vec(&success_response).map_err(|e| {
			self.log_error(&format!("Failed to serialize Complete message: {}", e));
			NetworkingError::Serialization(e)
		})
	}
}
