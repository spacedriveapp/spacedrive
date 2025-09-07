//! Initiator-specific pairing logic

use super::{
    messages::PairingMessage,
    security::PairingSecurity,
    types::{PairingSession, PairingState},
    PairingProtocolHandler,
};
use crate::service::networking::{
    device::{DeviceInfo, SessionKeys},
    NetworkingError, Result,
};
use iroh::net::key::NodeId;
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
        )).await;

        // Generate challenge
        let challenge = self.generate_challenge()?;
        self.log_debug(&format!(
            "Generated challenge of {} bytes for session {}",
            challenge.len(),
            session_id
        )).await;

        // Hold the write lock for the entire duration to prevent any scoping issues
        let mut sessions = self.active_sessions.write().await;
        self.log_debug(&format!("🔍 INITIATOR_HANDLER_DEBUG: Looking for session {} in {} total sessions", session_id, sessions.len())).await;

        if let Some(existing_session) = sessions.get_mut(&session_id) {
            self.log_debug(&format!("🔍 INITIATOR_HANDLER_DEBUG: Found existing session {} in state {:?}", session_id, existing_session.state)).await;
            self.log_debug(&format!("Transitioning existing session {} to ChallengeReceived", session_id)).await;

            // Update the existing session in place
            existing_session.state = PairingState::ChallengeReceived {
                challenge: challenge.clone(),
            };
            existing_session.remote_device_id = Some(from_device);
            existing_session.remote_device_info = Some(device_info.clone());
            existing_session.remote_public_key = Some(public_key.clone());
        } else {
            self.log_debug(&format!("🔍 INITIATOR_HANDLER_DEBUG: No existing session found for {}, creating new session", session_id)).await;
            self.log_debug(&format!("Creating new session {} for pairing request", session_id)).await;

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
        let local_device_info = self
            .get_device_info()
            .await
            .map_err(|e| {
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
        )).await;
        serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
    }

    /// Handle a pairing response (Initiator receives this from Joiner)
    pub(crate) async fn handle_pairing_response(
        &self,
        from_device: Uuid,
        session_id: Uuid,
        response: Vec<u8>,
        device_info: DeviceInfo,
    ) -> Result<Vec<u8>> {
        // Verify the response signature
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
                return Err(NetworkingError::Protocol(
                    "Invalid session state".to_string(),
                ))
            }
        };

        // Get the public key from the session (stored during pairing request)
        let device_public_key = session.remote_public_key
            .as_ref()
            .ok_or_else(|| NetworkingError::Protocol(
                "Device public key not found in session for signature verification".to_string()
            ))?
            .clone();

        // Validate inputs
        PairingSecurity::validate_challenge(&challenge)?;
        PairingSecurity::validate_signature(&response)?;
        PairingSecurity::validate_public_key(&device_public_key)?;

        // Verify the signature
        let signature_valid = PairingSecurity::verify_challenge_response(
            &device_public_key,
            &challenge,
            &response,
        )?;

        if !signature_valid {
            self.log_error(&format!(
                "Invalid signature for session {} from device {}",
                session_id, from_device
            )).await;

            // Mark session as failed
            if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
                session.state = PairingState::Failed {
                    reason: "Invalid challenge signature".to_string(),
                };
            }

            return Err(NetworkingError::Protocol(
                "Challenge signature verification failed".to_string(),
            ));
        }

        self.log_info(&format!(
            "Signature verified successfully for session {} from device {}",
            session_id, from_device
        )).await;

        // Generate session keys using pairing code secret
        let shared_secret = self.generate_shared_secret(session_id).await?;
        let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

        // Complete pairing in device registry with proper lock scoping
        // Use the actual device ID from device_info to ensure consistency
        let actual_device_id = device_info.device_id;
        {
            let mut registry = self.device_registry.write().await;
            registry.complete_pairing(
                actual_device_id,
                device_info.clone(),
                session_keys.clone(),
            ).await?;
        } // Release write lock here

        // Get node ID from the device info's network fingerprint
        let node_id = match device_info.network_fingerprint.node_id.parse::<NodeId>() {
            Ok(id) => id,
            Err(_) => {
                self.log_warn("Failed to parse node ID from device info, using fallback").await;
                NodeId::from_bytes(&[0u8; 32]).unwrap()
            }
        };

        // Mark device as connected since pairing is successful
        let simple_connection = crate::service::networking::device::DeviceConnection {
            addresses: vec![], // Will be filled in later
            latency_ms: None,
            rx_bytes: 0,
            tx_bytes: 0,
        };

        if let Err(e) = {
            let mut registry = self.device_registry.write().await;
            registry.mark_connected(actual_device_id, simple_connection).await
        }
        {
            self.log_warn(&format!(
                "Warning - failed to mark device as connected: {}",
                e
            )).await;
        } else {
            self.log_info(&format!(
                "Successfully marked device {} as connected",
                actual_device_id
            )).await;
        }

        // Update session
        if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
            session.state = PairingState::Completed;
            session.shared_secret = Some(shared_secret);
            session.remote_device_id = Some(actual_device_id);
            self.log_info(&format!(
                "Session {} updated with shared secret and remote device ID {}",
                session_id, actual_device_id
            )).await;
        }

        // IMPORTANT: Establish a persistent messaging connection after pairing
        // The initiator needs to establish a connection to the joiner as well
        self.log_info(&format!(
            "Establishing persistent messaging connection to paired device {} (node: {})",
            actual_device_id, node_id
        )).await;

        // Send a command to establish a new persistent connection
        let command = crate::service::networking::core::event_loop::EventLoopCommand::EstablishPersistentConnection {
            device_id: actual_device_id,
            node_id,
        };

        if let Err(e) = self.command_sender.send(command) {
            self.log_error(&format!(
                "Failed to send establish connection command: {:?}",
                e
            )).await;
        } else {
            self.log_info("Sent command to establish persistent connection").await;
        }

        // Send completion message
        let response = PairingMessage::Complete {
            session_id,
            success: true,
            reason: None,
        };

        serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
    }
}