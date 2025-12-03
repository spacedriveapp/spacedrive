//! Initiator-specific pairing logic

use super::{
	messages::PairingMessage,
	security::PairingSecurity,
	types::{PairingSession, PairingState},
	PairingProtocolHandler,
};
use crate::service::network::{
	device::{DeviceInfo, SessionKeys},
	NetworkingError, Result,
};
use iroh::{NodeId, Watcher};
use uuid::Uuid;

impl PairingProtocolHandler {
	/// Handle an incoming pairing request (Initiator receives this from Joiner)
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
			"Received pairing request from device {} for session {}",
			from_device, session_id
		))
		.await;

		// Generate challenge
		let challenge = self.generate_challenge()?;
		self.log_debug(&format!(
			"Generated challenge of {} bytes for session {}",
			challenge.len(),
			session_id
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
				"Transitioning existing session {} to ChallengeReceived",
				session_id
			))
			.await;

			// Update the existing session in place
			existing_session.state = PairingState::ChallengeReceived {
				challenge: challenge.clone(),
			};
			existing_session.remote_device_id = Some(from_device);
			existing_session.remote_device_info = Some(device_info.clone());
			existing_session.remote_public_key = Some(public_key.clone());
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
				state: PairingState::ChallengeReceived {
					challenge: challenge.clone(),
				},
				remote_device_id: Some(from_device),
				remote_device_info: Some(device_info.clone()),
				remote_public_key: Some(public_key.clone()),
				shared_secret: None,
				created_at: chrono::Utc::now(),
			};

			sessions.insert(session_id, session);
		}
		// Write lock is automatically released here

		// Send challenge response with proper network fingerprint
		let local_device_info = self.get_device_info().await.map_err(|e| {
			NetworkingError::Protocol(format!("Failed to get initiator device info: {}", e))
		})?;

		let response = PairingMessage::Challenge {
			session_id,
			challenge: challenge.clone(),
			device_info: local_device_info,
		};

		self.log_info(&format!(
			"Sending Challenge response for session {} with {} byte challenge",
			session_id,
			challenge.len()
		))
		.await;
		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
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
