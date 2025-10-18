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

		// Complete pairing immediately after successful challenge response since crypto exchange is done
		self.log_debug("About to complete pairing after challenge response...")
			.await;

		// Generate shared secret and complete pairing
		let shared_secret = self.generate_shared_secret(session_id).await?;
		let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

		// Get node ID from the initiator's device info
		let device_id = initiator_device_info.device_id;
		let node_id = match initiator_device_info
			.network_fingerprint
			.node_id
			.parse::<NodeId>()
		{
			Ok(id) => id,
			Err(_) => {
				self.log_warn("Failed to parse node ID from initiator device info, using fallback")
					.await;
				NodeId::from_bytes(&[0u8; 32]).unwrap()
			}
		};

		// Register the initiator device in Pairing state before completing pairing
		// This stores Alice's addresses so Bob can reconnect if needed
		{
			let mut registry = self.device_registry.write().await;
			// Create NodeAddr with addresses extracted from initiator_device_info
			let mut node_addr = iroh::NodeAddr::new(node_id);

			// Add direct addresses from the device_info
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
		} // Release write lock here

		// Complete pairing in device registry
		{
			let mut registry = self.device_registry.write().await;
			if let Err(e) = registry
				.complete_pairing(
					device_id,
					initiator_device_info.clone(),
					session_keys.clone(),
				)
				.await
			{
				self.log_error(&format!("Failed to complete pairing in registry: {}", e))
					.await;
				return Err(e);
			}
		}

		// Mark the initiator device as connected immediately after pairing completes
		// This ensures Bob sees Alice as connected even if the completion message fails
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
					"Warning - failed to mark initiator device {} as connected: {}",
					device_id, e
				))
				.await;
			} else {
				self.log_info(&format!(
					"Successfully marked initiator device {} as connected after pairing",
					device_id
				))
				.await;
			}
		}

		// Update session state to completed
		{
			let mut sessions = self.active_sessions.write().await;
			if let Some(session) = sessions.get_mut(&session_id) {
				self.log_debug(&format!(
					"Found session {}, updating state from {:?} to Completed",
					session_id, session.state
				))
				.await;
				session.state = PairingState::Completed;
				session.remote_device_id = Some(initiator_device_info.device_id);
				session.remote_device_info = Some(initiator_device_info.clone());
				session.shared_secret = Some(shared_secret.clone());
				self.log_info(&format!(
					"Session {} completed with shared secret for {}",
					session_id, initiator_device_info.device_name
				))
				.await;
			} else {
				self.log_error(&format!(
					"ERROR: Session {} not found when trying to complete",
					session_id
				))
				.await;
			}
		}

		// Send response
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
			"handle_pairing_challenge SUCCESS - returning {} bytes",
			serialized.len()
		))
		.await;
		Ok(serialized)
	}

	/// Handle completion message (Joiner receives this from Initiator)
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
			// Generate shared secret and complete pairing on joiner's side
			match self.generate_shared_secret(session_id).await {
				Ok(shared_secret) => {
					self.log_debug(&format!(
						"Generated shared secret of {} bytes",
						shared_secret.len()
					))
					.await;

					// Create session keys
					let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

					// Get Initiator's device info from the session state (received in Challenge message)
					let initiator_device_info = {
						let sessions = self.active_sessions.read().await;
						if let Some(session) = sessions.get(&session_id) {
							// Use the stored device info from the Challenge message
							if let Some(device_info) = &session.remote_device_info {
								device_info.clone()
							} else {
								// Fallback if no device info stored (shouldn't happen in normal flow)
								self.log_warn(
									"No remote device info stored in session, using fallback",
								)
								.await;
								crate::service::network::device::DeviceInfo {
                                    device_id: from_device,
                                    device_name: format!("Remote Device {}", &from_device.to_string()[..8]),
                                    device_type: crate::service::network::device::DeviceType::Desktop,
                                    os_version: "Unknown".to_string(),
                                    app_version: "Unknown".to_string(),
                                    network_fingerprint: crate::service::network::utils::identity::NetworkFingerprint {
                                        node_id: from_node.to_string(),
                                        public_key_hash: "unknown".to_string(),
                                    },
                                    last_seen: chrono::Utc::now(),
                                    direct_addresses: vec![],
                                }
							}
						} else {
							return Err(crate::service::network::NetworkingError::Protocol(
								"Session not found when completing pairing".to_string(),
							));
						}
					};

					// Use the actual device ID from device_info to ensure consistency
					let device_id = initiator_device_info.device_id;
					let node_id = match initiator_device_info
						.network_fingerprint
						.node_id
						.parse::<NodeId>()
					{
						Ok(id) => id,
						Err(_) => {
							self.log_warn("Failed to parse node ID from initiator device info, using from_node fallback")
								.await;
							from_node
						}
					};

					// Register the initiator device in Pairing state before completing pairing
					// This stores Alice's addresses so Bob can reconnect if needed
					{
						let mut registry = self.device_registry.write().await;
						// Create NodeAddr with addresses extracted from initiator_device_info
						let mut node_addr = iroh::NodeAddr::new(node_id);

						// Add direct addresses from the device_info
						for addr_str in &initiator_device_info.direct_addresses {
							if let Ok(socket_addr) = addr_str.parse() {
								node_addr = node_addr.with_direct_addresses([socket_addr]);
							}
						}

						if !initiator_device_info.direct_addresses.is_empty() {
							self.log_info(&format!(
								"Extracted {} direct addresses from initiator device info in completion handler",
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
					} // Release write lock here

					// Complete pairing in device registry
					let pairing_result = {
						let mut registry = self.device_registry.write().await;
						registry
							.complete_pairing(
								device_id,
								initiator_device_info.clone(),
								session_keys.clone(),
							)
							.await
					}; // Release write lock here

					match pairing_result {
						Ok(()) => {
							// Update session state FIRST before any other operations that might fail
							{
								let mut sessions = self.active_sessions.write().await;
								if let Some(session) = sessions.get_mut(&session_id) {
									session.state = PairingState::Completed;
									session.shared_secret = Some(shared_secret.clone());
									session.remote_device_id = Some(device_id);
								}
							}

							self.log_info("Successfully completed pairing").await;

							// Mark Initiator as connected (optional - pairing already completed)
							let initiator_node_id = Some(from_node); // Use node from completion message

							if let Some(node_id) = initiator_node_id {
								let simple_connection =
									crate::service::network::device::ConnectionInfo {
										addresses: vec![], // Will be filled in later
										latency_ms: None,
										rx_bytes: 0,
										tx_bytes: 0,
									};

								let _mark_result = {
									let mut registry = self.device_registry.write().await;
									registry.mark_connected(device_id, simple_connection).await
								};
							}
						}
						Err(e) => {
							self.log_error(&format!(
								"Failed to complete pairing in device registry: {}",
								e
							))
							.await;
						}
					}
				}
				Err(e) => {
					self.log_error(&format!("Failed to generate shared secret: {}", e))
						.await;
					let mut sessions = self.active_sessions.write().await;
					if let Some(session) = sessions.get_mut(&session_id) {
						session.state = PairingState::Failed {
							reason: format!("Failed to generate shared secret: {}", e),
						};
					}
				}
			}
		} else {
			// Pairing failed
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
				self.log_error(&format!(
					"ERROR: Session {} not found when processing completion",
					session_id
				))
				.await;
			}
		}

		Ok(())
	}
}
