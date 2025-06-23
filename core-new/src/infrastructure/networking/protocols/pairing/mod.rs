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
use crate::infrastructure::networking::{
    device::{DeviceInfo, DeviceRegistry, SessionKeys},
    utils::{identity::NetworkFingerprint, logging::NetworkLogger, NetworkIdentity},
    NetworkingError, Result,
};
use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId};
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
    
    /// Logger for structured logging
    logger: Arc<dyn NetworkLogger>,
    
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
    ) -> Self {
        Self {
            identity,
            device_registry,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            logger,
            role: None,
            persistence: None,
        }
    }

    /// Create a new pairing protocol handler with persistence
    pub fn new_with_persistence(
        identity: NetworkIdentity, 
        device_registry: Arc<RwLock<DeviceRegistry>>,
        logger: Arc<dyn NetworkLogger>,
        data_dir: PathBuf,
    ) -> Self {
        let persistence = Arc::new(PairingPersistence::new(data_dir));
        Self {
            identity,
            device_registry,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            logger,
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
                self.log_info(&format!("Loaded {} persisted pairing sessions", count)).await;
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
        self.logger.info(&format!("{} {}", role_prefix, message)).await;
    }

    /// Log debug message with role prefix
    async fn log_debug(&self, message: &str) {
        let role_prefix = match &self.role {
            Some(PairingRole::Initiator) => "[INITIATOR]",
            Some(PairingRole::Joiner) => "[JOINER]",
            None => "[PAIRING]",
        };
        self.logger.debug(&format!("{} {}", role_prefix, message)).await;
    }

    /// Log warning message with role prefix
    async fn log_warn(&self, message: &str) {
        let role_prefix = match &self.role {
            Some(PairingRole::Initiator) => "[INITIATOR]",
            Some(PairingRole::Joiner) => "[JOINER]",
            None => "[PAIRING]",
        };
        self.logger.warn(&format!("{} {}", role_prefix, message)).await;
    }

    /// Log error message with role prefix
    async fn log_error(&self, message: &str) {
        let role_prefix = match &self.role {
            Some(PairingRole::Initiator) => "[INITIATOR]",
            Some(PairingRole::Joiner) => "[JOINER]",
            None => "[PAIRING]",
        };
        self.logger.error(&format!("{} {}", role_prefix, message)).await;
    }

    /// Start a new pairing session as initiator
    /// Returns the session ID which should be advertised via DHT by the caller
    pub async fn start_pairing_session(&self) -> Result<Uuid> {
        let session_id = Uuid::new_v4();
        self.start_pairing_session_with_id(session_id).await?;
        Ok(session_id)
    }

    /// Start a new pairing session with a specific session ID
    pub async fn start_pairing_session_with_id(&self, session_id: Uuid) -> Result<()> {
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

        // Save to persistence
        self.save_sessions_to_persistence().await?;

        self.log_info(&format!("Started pairing session: {}", session_id)).await;
        Ok(())
    }

    /// Join an existing pairing session with a specific session ID
    /// This allows a joiner to participate in an initiator's session
    pub async fn join_pairing_session(&self, session_id: Uuid) -> Result<()> {
        
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

        // Create new scanning session for Bob (the joiner)
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

        // Save to persistence
        self.save_sessions_to_persistence().await?;

        self.log_info(&format!(
            "Joined pairing session: {} (state: Scanning)",
            session_id
        )).await;

        // Verify session was created correctly
        let sessions = self.active_sessions.read().await;
        if let Some(created_session) = sessions.get(&session_id) {
            if matches!(created_session.state, PairingState::Scanning) {
                self.log_debug(&format!(
                    "Pairing session verified in Scanning state: {}",
                    session_id
                )).await;
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
        let initial_count = sessions.len();

        // Remove sessions older than timeout duration
        let handler = self;
        sessions.retain(|_, session| {
            let age = now.signed_duration_since(session.created_at);
            if age > timeout_duration {
                // Note: Can't use async logging inside retain closure
                false
            } else {
                true
            }
        });

        let cleaned_count = initial_count - sessions.len();
        if cleaned_count > 0 {
            self.log_info(&format!("Cleaned up {} expired pairing sessions", cleaned_count)).await;
        }

        Ok(cleaned_count)
    }

    /// Start a background task to periodically clean up expired sessions
    pub fn start_cleanup_task(handler: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                if let Err(e) = handler.cleanup_expired_sessions().await {
                    eprintln!("Error during session cleanup: {}", e);
                }
            }
        });
    }

    
    fn generate_challenge(&self) -> Result<Vec<u8>> {
        use rand::RngCore;
        let mut challenge = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut challenge);
        Ok(challenge)
    }

    fn generate_shared_secret(&self) -> Result<Vec<u8>> {
        use rand::RngCore;
        let mut secret = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        Ok(secret)
    }
}

#[async_trait]
impl ProtocolHandler for PairingProtocolHandler {
    fn protocol_name(&self) -> &str {
        "pairing"
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
                        self.log_error(&format!("Marked pairing session {} as failed: {}", session_id, error)).await;
                    }
                }
            }
        }

        result
    }

    async fn handle_response(
        &self,
        from_device: Uuid,
        from_peer: PeerId,
        response_data: Vec<u8>,
    ) -> Result<()> {
        self.log_debug(&format!(
            "handle_response called with {} bytes from device {}",
            response_data.len(),
            from_device
        )).await;

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
                )).await;

                // Check session state before processing
                {
                    let sessions = self.active_sessions.read().await;
                    if let Some(session) = sessions.get(&session_id) {
                        self.log_debug(&format!(
                            "Session {} state before challenge processing: {}",
                            session_id, session.state
                        )).await;
                    } else {
                        self.log_debug(&format!("No session found for {}", session_id)).await;
                    }
                }

                self.log_debug("About to call handle_pairing_challenge...").await;

                // Call the existing handle_pairing_challenge method
                match self
                    .handle_pairing_challenge(session_id, challenge.clone(), device_info)
                    .await
                {
                    Ok(response_data) => {
                        self.log_debug(&format!("handle_pairing_challenge succeeded, generated {} byte response", response_data.len())).await;

                        // Check session state after handle_pairing_challenge
                        {
                            let sessions = self.active_sessions.read().await;
                            if let Some(session) = sessions.get(&session_id) {
                                self.log_debug(&format!(
                                    "Session {} state after handle_pairing_challenge: {}",
                                    session_id, session.state
                                )).await;
                            }
                        }

                        // Use the peer ID directly from the method parameter (this is Initiator's peer ID)
                        let remote_peer_id = Some(from_peer);
                        self.log_debug(&format!(
                            "Using peer ID from method parameter: {:?}",
                            from_peer
                        )).await;

                        // Update the session state to ResponsePending so the unified pairing flow can send it
                        {
                            let mut sessions = self.active_sessions.write().await;
                            if let Some(session) = sessions.get_mut(&session_id) {
                                session.state = PairingState::ResponsePending {
                                    challenge: challenge.clone(),
                                    response_data: response_data.clone(),
                                    remote_peer_id,
                                };
                                self.log_debug(&format!(
                                    "Session {} updated to ResponsePending state",
                                    session_id
                                )).await;
                            } else {
                                self.log_error(&format!("ERROR: Session {} not found when trying to update to ResponsePending", session_id)).await;
                            }
                        }

                        // Verify state change
                        {
                            let sessions = self.active_sessions.read().await;
                            if let Some(session) = sessions.get(&session_id) {
                                self.log_debug(&format!(
                                    "Session {} final state: {}",
                                    session_id, session.state
                                )).await;
                            }
                        }

                        self.log_info(&format!(
                            "Challenge response ready to send for session {}",
                            session_id
                        )).await;
                    }
                    Err(e) => {
                        self.log_error(&format!(
                            "handle_pairing_challenge FAILED for session {}: {}",
                            session_id, e
                        )).await;
                    }
                }
            }
            PairingMessage::Complete {
                session_id,
                success,
                reason,
            } => {
                self.handle_completion(session_id, success, reason, from_device, from_peer).await?;
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