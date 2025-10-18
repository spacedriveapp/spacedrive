//! Joiner-specific pairing logic

use super::{
	messages::PairingMessage,
	types::{PairingSession, PairingState},
	PairingProtocolHandler,
};
use crate::service::network::{
	device::{DeviceInfo, SessionKeys},
	NetworkingError, Result,
};
use iroh::NodeId;
use uuid::Uuid;

impl PairingProtocolHandler {
	/// Handle a pairing challenge (Joiner receives this from Initiator)
	pub(crate) async fn handle_pairing_challenge(
		&self,
		session_id: Uuid,
		challenge: Vec<u8>,
		initiator_device_info: DeviceInfo,
	) -> Result<Vec<u8>> {
		self.log_info(&format!(
			"handle_pairing_challenge ENTRY - session {} with {} bytes",
			session_id,
			challenge.len()
		))
		.await;

		// Sign the challenge
		self.log_debug("About to sign challenge...").await;
		let signature = match self.identity.sign(&challenge) {
			Ok(sig) => {
				self.log_debug(&format!(
					"Successfully signed challenge, signature is {} bytes",
					sig.len()
				))
				.await;
				sig
			}
			Err(e) => {
				self.log_error(&format!("FAILED to sign challenge: {}", e))
					.await;
				return Err(e);
			}
		};

		// Get local device info with proper network fingerprint
		self.log_debug("About to get local device info...").await;
		let device_info = match self.get_device_info().await {
			Ok(info) => {
				self.log_debug(&format!(
					"Successfully got local device info for device {} with node_id {}",
					info.device_id, info.network_fingerprint.node_id
				))
				.await;
				info
			}
			Err(e) => {
				self.log_error(&format!("FAILED to get local device info: {}", e))
					.await;
				return Err(e);
			}
		};

		// Store initiator info for later (when we receive Complete message)
		// DO NOT complete pairing yet - wait for initiator to confirm she verified our signature
		{
			let mut sessions = self.active_sessions.write().await;
			if let Some(session) = sessions.get_mut(&session_id) {
				self.log_debug(&format!(
					"Storing initiator info and transitioning session {} to ResponseSent",
					session_id
				))
				.await;
				session.remote_device_id = Some(initiator_device_info.device_id);
				session.remote_device_info = Some(initiator_device_info.clone());
				session.state = PairingState::ResponseSent; // NOT Completed!
			} else {
				self.log_error(&format!(
					"ERROR: Session {} not found when trying to store initiator info",
					session_id
				))
				.await;
				return Err(NetworkingError::Protocol(format!(
					"Session {} not found",
					session_id
				)));
			}
		}

		// Send response - pairing will complete when we receive Complete message from initiator
		self.log_debug("About to create response message...").await;
		let response = PairingMessage::Response {
			session_id,
			response: signature,
			device_info,
		};

		self.log_debug("About to serialize response...").await;
		let serialized =
			serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))?;

		self.log_info(&format!(
			"handle_pairing_challenge SUCCESS - returning {} bytes, waiting for Complete message",
			serialized.len()
		))
		.await;
		Ok(serialized)
	}

	/// Handle completion message (Joiner receives this from Initiator)
	/// This is the ONLY place joiner completes pairing - ensures cryptographic certainty
	pub(crate) async fn handle_completion(
		&self,
		session_id: Uuid,
		success: bool,
		reason: Option<String>,
		from_device: Uuid,
		from_node: NodeId,
	) -> Result<()> {
		self.log_info(&format!(
			"Received completion message for session {} - success: {}",
			session_id, success
		))
		.await;

		if success {
			// initiator has verified our signature and confirmed pairing success
			// NOW we can complete pairing on our side

			// Get initiator device info that we stored in handle_pairing_challenge
			let initiator_device_info = {
				let sessions = self.active_sessions.read().await;
				sessions
					.get(&session_id)
					.and_then(|s| s.remote_device_info.clone())
					.ok_or_else(|| {
						NetworkingError::Protocol(
							"No device info stored - handle_pairing_challenge must run first"
								.to_string(),
						)
					})?
			};

			// Generate shared secret and session keys
			let shared_secret = self.generate_shared_secret(session_id).await?;
			let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

			let device_id = initiator_device_info.device_id;
			let node_id = match initiator_device_info
				.network_fingerprint
				.node_id
				.parse::<NodeId>()
			{
				Ok(id) => id,
				Err(_) => {
					self.log_warn(
						"Failed to parse node ID from initiator device info, using from_node fallback",
					)
					.await;
					from_node
				}
			};

			// Register the initiator device in Pairing state
			{
				let mut registry = self.device_registry.write().await;
				let mut node_addr = iroh::NodeAddr::new(node_id);

				// Add direct addresses from device_info
				for addr_str in &initiator_device_info.direct_addresses {
					if let Ok(socket_addr) = addr_str.parse() {
						node_addr = node_addr.with_direct_addresses([socket_addr]);
					}
				}

				if !initiator_device_info.direct_addresses.is_empty() {
					self.log_info(&format!(
						"Extracted {} direct addresses from initiator device info",
						initiator_device_info.direct_addresses.len()
					))
					.await;
				}

				registry
					.start_pairing(device_id, node_id, session_id, node_addr)
					.map_err(|e| {
						self.log_warn(&format!(
							"Warning: Could not register initiator device in Pairing state: {}",
							e
						));
						e
					})
					.ok(); // Ignore errors - device might already be in pairing state
			}

			// Complete pairing in device registry
			{
				let mut registry = self.device_registry.write().await;
				registry
					.complete_pairing(device_id, initiator_device_info.clone(), session_keys)
					.await?;
			}

			// Mark initiator as connected
			{
				let simple_connection = crate::service::network::device::ConnectionInfo {
					addresses: vec![], // Will be filled in later
					latency_ms: None,
					rx_bytes: 0,
					tx_bytes: 0,
				};

				let mut registry = self.device_registry.write().await;
				if let Err(e) = registry.mark_connected(device_id, simple_connection).await {
					self.log_warn(&format!(
						"Warning: Failed to mark initiator device {} as connected: {}",
						device_id, e
					))
					.await;
				} else {
					self.log_info(&format!(
						"Successfully marked initiator device {} as connected",
						device_id
					))
					.await;
				}
			}

			// Update session state to completed
			{
				let mut sessions = self.active_sessions.write().await;
				if let Some(session) = sessions.get_mut(&session_id) {
					session.state = PairingState::Completed;
					session.shared_secret = Some(shared_secret);
					session.remote_device_id = Some(device_id);
					self.log_info(&format!(
						"Session {} completed successfully for {}",
						session_id, initiator_device_info.device_name
					))
					.await;
				} else {
					return Err(NetworkingError::Protocol(format!(
						"Session {} not found when updating to Completed",
						session_id
					)));
				}
			}

			self.log_info("Pairing completed successfully with cryptographic confirmation")
				.await;
		} else {
			// Pairing failed - initiator rejected our signature or other error
			let failure_reason = reason.unwrap_or_else(|| "Pairing failed".to_string());
			let mut sessions = self.active_sessions.write().await;
			if let Some(session) = sessions.get_mut(&session_id) {
				session.state = PairingState::Failed {
					reason: failure_reason.clone(),
				};
				self.log_error(&format!(
					"Session {} marked as failed: {}",
					session_id, failure_reason
				))
				.await;
			} else {
				self.log_warn(&format!(
					"Session {} not found when processing failure",
					session_id
				))
				.await;
			}
		}

		Ok(())
	}
}
