//! Joiner-specific pairing logic

use super::{
    messages::PairingMessage,
    types::{PairingSession, PairingState},
    PairingProtocolHandler,
};
use crate::infrastructure::networking::{
    device::{DeviceInfo, SessionKeys},
    NetworkingError, Result,
};
use libp2p::PeerId;
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
        )).await;

        // Sign the challenge
        self.log_debug("About to sign challenge...").await;
        let signature = match self.identity.sign(&challenge) {
            Ok(sig) => {
                self.log_debug(&format!(
                    "Successfully signed challenge, signature is {} bytes",
                    sig.len()
                )).await;
                sig
            }
            Err(e) => {
                self.log_error(&format!("FAILED to sign challenge: {}", e)).await;
                return Err(e);
            }
        };

        // Get local device info with proper network fingerprint
        self.log_debug("About to get local device info...").await;
        let device_info = match self.get_device_info().await {
            Ok(info) => {
                self.log_debug(&format!(
                    "Successfully got local device info for device {} with peer_id {}",
                    info.device_id, info.network_fingerprint.peer_id
                )).await;
                info
            }
            Err(e) => {
                self.log_error(&format!("FAILED to get local device info: {}", e)).await;
                return Err(e);
            }
        };

        // Update session state and store Initiator's device info
        self.log_debug("About to update session state to ResponseSent...").await;
        {
            let mut sessions = self.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                self.log_debug(&format!(
                    "Found session {}, updating state from {:?} to ResponseSent",
                    session_id, session.state
                )).await;
                session.state = PairingState::ResponseSent;
                session.remote_device_id = Some(initiator_device_info.device_id); // Store Initiator's device ID
                session.remote_device_info = Some(initiator_device_info.clone()); // Store full device info
                self.log_debug(&format!(
                    "Session {} state updated to ResponseSent with initiator device info for {}",
                    session_id, initiator_device_info.device_name
                )).await;
            } else {
                self.log_error(&format!(
                    "ERROR: Session {} not found when trying to update to ResponseSent",
                    session_id
                )).await;
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
        let serialized = serde_json::to_vec(&response).map_err(|e| {
            NetworkingError::Serialization(e)
        })?;

        self.log_info(&format!(
            "handle_pairing_challenge SUCCESS - returning {} bytes",
            serialized.len()
        )).await;
        Ok(serialized)
    }

    /// Handle completion message (Joiner receives this from Initiator)
    pub(crate) async fn handle_completion(
        &self,
        session_id: Uuid,
        success: bool,
        reason: Option<String>,
        from_device: Uuid,
        from_peer: PeerId,
    ) -> Result<()> {
        self.log_info(&format!(
            "Received completion message for session {} - success: {}",
            session_id, success
        )).await;

        if success {
            // Generate shared secret and complete pairing on joiner's side
            match self.generate_shared_secret(session_id).await {
                Ok(shared_secret) => {
                    self.log_debug(&format!(
                        "Generated shared secret of {} bytes",
                        shared_secret.len()
                    )).await;

                    // Create session keys
                    let session_keys =
                        SessionKeys::from_shared_secret(shared_secret.clone());

                    // Get Initiator's device info from the session state (received in Challenge message)
                    let initiator_device_info = {
                        let sessions = self.active_sessions.read().await;
                        if let Some(session) = sessions.get(&session_id) {
                            // Use the stored device info from the Challenge message
                            if let Some(device_info) = &session.remote_device_info {
                                device_info.clone()
                            } else {
                                // Fallback if no device info stored (shouldn't happen in normal flow)
                                self.log_warn("No remote device info stored in session, using fallback").await;
                                crate::infrastructure::networking::device::DeviceInfo {
                                    device_id: from_device,
                                    device_name: format!("Remote Device {}", &from_device.to_string()[..8]),
                                    device_type: crate::infrastructure::networking::device::DeviceType::Desktop,
                                    os_version: "Unknown".to_string(),
                                    app_version: "Unknown".to_string(),
                                    network_fingerprint: crate::infrastructure::networking::utils::identity::NetworkFingerprint {
                                        peer_id: from_peer.to_string(),
                                        public_key_hash: "unknown".to_string(),
                                    },
                                    last_seen: chrono::Utc::now(),
                                }
                            }
                        } else {
                            return Err(crate::infrastructure::networking::NetworkingError::Protocol(
                                "Session not found when completing pairing".to_string()
                            ));
                        }
                    };

                    // Complete pairing in device registry
                    // Use the actual device ID from device_info to ensure consistency
                    let actual_device_id = initiator_device_info.device_id;
                    let pairing_result = {
                        let mut registry = self.device_registry.write().await;
                        registry.complete_pairing(
                            actual_device_id,
                            initiator_device_info.clone(),
                            session_keys.clone(),
                        ).await
                    }; // Release write lock here

                    match pairing_result {
                        Ok(()) => {
                            // Update session state FIRST before any other operations that might fail
                            {
                                let mut sessions = self.active_sessions.write().await;
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.state = PairingState::Completed;
                                    session.shared_secret = Some(shared_secret.clone());
                                    session.remote_device_id = Some(actual_device_id);
                                }
                            }

                            self.log_info("Successfully completed pairing").await;

                            // Mark Initiator as connected (optional - pairing already completed)
                            let initiator_peer_id = Some(from_peer); // Use peer from completion message

                            if let Some(peer_id) = initiator_peer_id {
                                let (connection, _message_receiver) = crate::infrastructure::networking::device::DeviceConnection::new(
                                    peer_id,
                                    initiator_device_info.clone(),
                                    session_keys.clone(),
                                );

                                let _mark_result = {
                                    let mut registry = self.device_registry.write().await;
                                    registry.mark_connected(actual_device_id, connection).await
                                };
                            }
                        }
                        Err(e) => {
                            self.log_error(&format!("Failed to complete pairing in device registry: {}", e)).await;
                        }
                    }
                }
                Err(e) => {
                    self.log_error(&format!("Failed to generate shared secret: {}", e)).await;
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
                )).await;
            } else {
                self.log_error(&format!(
                    "ERROR: Session {} not found when processing completion",
                    session_id
                )).await;
            }
        }

        Ok(())
    }
}