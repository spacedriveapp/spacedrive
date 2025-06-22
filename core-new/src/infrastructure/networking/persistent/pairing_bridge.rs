//! Bridge between ephemeral pairing and persistent device management
//!
//! This module provides the critical integration between LibP2PPairingProtocol
//! (which handles the pairing process) and PersistentConnectionManager (which
//! handles long-term device relationships and connections).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{timeout, sleep};
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::networking::{DeviceInfo, Result, NetworkError};
use crate::networking::pairing::{
    protocol::LibP2PPairingProtocol, 
    PairingUserInterface, 
    PairingState, 
    SessionKeys,
    PairingCode
};
use crate::networking::identity::NetworkIdentity;
use super::{TrustLevel};
use super::service::NetworkingServiceRef;

/// Session information for active pairing attempts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingSession {
    pub id: Uuid,
    pub code: String,
    pub expires_at: DateTime<Utc>,
    pub role: PairingRole,
    pub status: PairingStatus,
    pub auto_accept: bool,
}

impl PairingSession {
    pub fn expires_in_seconds(&self) -> u32 {
        let now = Utc::now();
        if self.expires_at > now {
            (self.expires_at - now).num_seconds() as u32
        } else {
            0
        }
    }
}

/// Role in the pairing process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PairingRole {
    Initiator,
    Joiner,
}

/// Current status of a pairing session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PairingStatus {
    GeneratingCode,
    Broadcasting,
    WaitingForConnection,
    Connected,
    Authenticating,
    Completed,
    Failed(String),
    Cancelled,
}

/// Bridge between pairing protocol and persistent networking
#[derive(Clone)]
pub struct PairingBridge {
    /// Networking service for persistence integration
    networking_service: Arc<NetworkingServiceRef>,
    
    /// Active pairing sessions
    active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    
    /// Task handles for active pairing operations
    pairing_tasks: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
    
    /// Network identity for creating pairing protocols
    network_identity: NetworkIdentity,
    
    /// Password for pairing operations
    password: String,
}

impl PairingBridge {
    /// Create a new pairing bridge
    pub fn new(
        networking_service: Arc<NetworkingServiceRef>,
        network_identity: NetworkIdentity, 
        password: String,
    ) -> Self {
        Self {
            networking_service,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            pairing_tasks: Arc::new(RwLock::new(HashMap::new())),
            network_identity,
            password,
        }
    }

    /// Start pairing as initiator with automatic device registration on success
    /// Returns immediately with pairing code, while pairing continues in background
    pub async fn start_pairing_as_initiator(&self, auto_accept: bool) -> Result<PairingSession> {
        let session_id = Uuid::new_v4();
        let expires_at = Utc::now() + chrono::Duration::seconds(300); // 5 minutes
        
        info!("Starting pairing as initiator with session ID: {}", session_id);
        
        // Create initial session record
        let mut session = PairingSession {
            id: session_id,
            code: String::new(), // Will be filled when protocol generates it
            expires_at,
            role: PairingRole::Initiator,
            status: PairingStatus::GeneratingCode,
            auto_accept,
        };
        
        // Store initial session
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(session_id, session.clone());
        }
        
        // Clone necessary data for the background task
        let network_identity = self.network_identity.clone();
        let password = self.password.clone();
        let networking_service = self.networking_service.clone();
        let active_sessions = self.active_sessions.clone();
        
        // Generate pairing code immediately (non-blocking)
        let result = self.generate_pairing_code_immediately(
            session_id,
            network_identity.clone(),
            password.clone(),
        ).await;
        
        // Update session with pairing code and get final session
        let final_session = {
            let mut sessions = self.active_sessions.write().await;
            if let Some(stored_session) = sessions.get_mut(&session_id) {
                match result {
                    Ok(code) => {
                        stored_session.code = code.clone();
                        stored_session.status = PairingStatus::Broadcasting;
                        info!("Generated pairing code: {} (expires in {} seconds)", 
                              code.split_whitespace().take(3).collect::<Vec<_>>().join(" ") + "...",
                              stored_session.expires_in_seconds());
                    }
                    Err(e) => {
                        stored_session.status = PairingStatus::Failed(e.to_string());
                        error!("Failed to generate pairing code for session {}: {}", session_id, e);
                        return Err(e);
                    }
                }
            }
            
            sessions.get(&session_id).cloned().unwrap_or(session)
        }; // Write lock released here
        
        // Start LibP2P protocol in background to handle incoming connections
        // This creates the networking setup but returns the code immediately
        info!("Preparing LibP2P protocol for session {}", session_id);
        
        // Mark session as waiting for connections
        {
            let mut sessions = self.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = PairingStatus::WaitingForConnection;
            }
        }
        
        // Start LibP2P listeners for Alice to become discoverable to Bob
        info!("Starting LibP2P protocol for session {} to make Alice discoverable", session_id);
        
        // We need to start the LibP2P protocol in a way that doesn't block returning the pairing code
        // The key insight is that the pairing code is already generated above, so now we just need
        // to start the LibP2P listeners to make Alice discoverable to Bob
        
        // Clone the data we need for the background protocol
        let session_id_clone = session_id;
        let auto_accept_clone = auto_accept;
        let network_identity_clone = self.network_identity.clone();
        let password_clone = self.password.clone();
        let networking_service_clone = self.networking_service.clone();
        let active_sessions_clone = self.active_sessions.clone();
        
        // Use tokio::spawn_local within a LocalSet to avoid Send constraints
        let local_set = tokio::task::LocalSet::new();
        local_set.spawn_local(async move {
            info!("Background LibP2P protocol starting for session {}", session_id_clone);
            
            let result = Self::run_initiator_protocol_task(
                session_id_clone,
                auto_accept_clone,
                network_identity_clone,
                password_clone,
                networking_service_clone,
                active_sessions_clone.clone(),
            ).await;
            
            // Update session status based on protocol result
            let mut sessions = active_sessions_clone.write().await;
            if let Some(session) = sessions.get_mut(&session_id_clone) {
                match result {
                    Ok(_) => {
                        session.status = PairingStatus::Completed;
                        info!("Background LibP2P protocol completed successfully for session {}", session_id_clone);
                    }
                    Err(e) => {
                        session.status = PairingStatus::Failed(e.to_string());
                        error!("Background LibP2P protocol failed for session {}: {}", session_id_clone, e);
                    }
                }
            }
        });
        
        // Run the LocalSet briefly to start the background task, then continue
        let _ = tokio::time::timeout(std::time::Duration::from_millis(50), local_set.run_until(async {
            // This will start the spawn_local task but not wait for it to complete
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        })).await;
        
        info!("LibP2P protocol started in background for session {}", session_id);
        
        Ok(final_session)
    }
    
    /// Join pairing session with automatic device registration on success
    pub async fn join_pairing_session(&self, code: String) -> Result<()> {
        let session_id = Uuid::new_v4();
        
        info!("Joining pairing session with code: {} (session {})", 
              code.split_whitespace().take(3).collect::<Vec<_>>().join(" ") + "...", 
              session_id);
        
        // Create session record
        let session = PairingSession {
            id: session_id,
            code: code.clone(),
            expires_at: Utc::now() + chrono::Duration::seconds(300),
            role: PairingRole::Joiner,
            status: PairingStatus::WaitingForConnection,
            auto_accept: true, // Joiners implicitly accept by joining
        };
        
        // Store session
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(session_id, session);
        }
        
        // Clone necessary data for the LocalSet execution
        let network_identity = self.network_identity.clone();
        let password = self.password.clone();
        let networking_service = self.networking_service.clone();
        let active_sessions = self.active_sessions.clone();
        
        // Execute pairing protocol on LocalSet to avoid Send requirements
        let local_set = tokio::task::LocalSet::new();
        let result = local_set.run_until(async {
            Self::run_joiner_protocol_task(
                session_id,
                code,
                network_identity,
                password,
                networking_service,
                active_sessions.clone(),
            ).await
        }).await;
        
        // Update session with result
        let mut sessions = self.active_sessions.write().await;
        if let Some(stored_session) = sessions.get_mut(&session_id) {
            match result {
                Ok(()) => {
                    stored_session.status = PairingStatus::Completed;
                    info!("Pairing completed successfully for session {}", session_id);
                }
                Err(e) => {
                    stored_session.status = PairingStatus::Failed(e.to_string());
                    error!("Joiner pairing failed for session {}: {}", session_id, e);
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get status of all active pairing sessions
    pub async fn get_pairing_status(&self) -> Vec<PairingSession> {
        let sessions = self.active_sessions.read().await;
        sessions.values().cloned().collect()
    }
    
    /// Cancel an active pairing session
    pub async fn cancel_pairing(&self, session_id: Uuid) -> Result<()> {
        info!("Cancelling pairing session: {}", session_id);
        
        // Update session status and remove it
        {
            let mut sessions = self.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = PairingStatus::Cancelled;
            }
            // Remove immediately since we're not using background tasks
            sessions.remove(&session_id);
        }
        
        // Clean up any remaining task handles
        {
            let mut tasks = self.pairing_tasks.write().await;
            tasks.remove(&session_id);
        }
        
        Ok(())
    }

    /// Static task method for initiator protocol (Send-safe)
    async fn run_initiator_protocol_task(
        session_id: Uuid,
        auto_accept: bool,
        network_identity: NetworkIdentity,
        password: String,
        networking_service: Arc<NetworkingServiceRef>,
        active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    ) -> Result<String> {
        // Create LibP2PPairingProtocol
        let device_info = network_identity.to_device_info();
        let private_key = network_identity.unlock_private_key(&password)?;
        let mut protocol = LibP2PPairingProtocol::new(
            &network_identity,
            device_info,
            private_key,
            &password,
        ).await?;
        
        // Start listening
        let _listening_addrs = protocol.start_listening().await?;
        
        // Create UI interface for pairing
        let ui = BridgePairingUI::new(session_id, active_sessions);
        
        // Start pairing as initiator
        let (remote_device, session_keys) = protocol.start_as_initiator(&ui).await?;
        
        // Register device with persistent networking
        Self::handle_pairing_complete(remote_device, session_keys, networking_service).await?;
        
        // Get the generated pairing code from the session
        let sessions = ui.sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            Ok(session.code.clone())
        } else {
            Err(NetworkError::NotInitialized("Session not found".to_string()))
        }
    }
    
    /// Static task method for joiner protocol (Send-safe)
    async fn run_joiner_protocol_task(
        session_id: Uuid,
        code: String,
        network_identity: NetworkIdentity,
        password: String,
        networking_service: Arc<NetworkingServiceRef>,
        active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    ) -> Result<()> {
        // Create LibP2PPairingProtocol
        let device_info = network_identity.to_device_info();
        let private_key = network_identity.unlock_private_key(&password)?;
        let mut protocol = LibP2PPairingProtocol::new(
            &network_identity,
            device_info,
            private_key,
            &password,
        ).await?;
        
        // Start listening
        let _listening_addrs = protocol.start_listening().await?;
        
        // Create UI interface for pairing
        let ui = BridgePairingUI::new(session_id, active_sessions);
        
        // Parse pairing code from string
        let pairing_code = PairingCode::from_string(&code)?;
        
        // Start pairing as joiner
        let (remote_device, session_keys) = protocol.start_as_joiner(&ui, pairing_code).await?;
        
        // Register device with persistent networking
        Self::handle_pairing_complete(remote_device, session_keys, networking_service).await?;
        
        Ok(())
    }
    
    
    /// Static method to handle pairing completion (Send-safe)
    async fn handle_pairing_complete(
        remote_device: DeviceInfo,
        session_keys: SessionKeys,
        networking_service: Arc<NetworkingServiceRef>,
    ) -> Result<()> {
        info!("Pairing completed successfully with device: {} ({})", 
              remote_device.device_name, remote_device.device_id);
        
        // Convert pairing SessionKeys to persistent SessionKeys
        let persistent_keys = crate::networking::persistent::SessionKeys::from(session_keys);
        
        // Add device to persistent networking service
        networking_service
            .add_paired_device(remote_device, persistent_keys)
            .await?;
        
        Ok(())
    }
    
    /// Called when LibP2PPairingProtocol completes successfully (legacy method for compatibility)
    async fn on_pairing_complete(
        &self,
        remote_device: DeviceInfo,
        session_keys: SessionKeys,
        session_id: Uuid,
    ) -> Result<()> {
        // Use the static method
        Self::handle_pairing_complete(remote_device, session_keys, self.networking_service.clone()).await?;
        
        // Update session status
        {
            let mut sessions = self.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = PairingStatus::Completed;
            }
        }
        
        // Clean up session after success (synchronous cleanup)
        tokio::time::sleep(Duration::from_secs(30)).await; // Keep session for status queries
        
        let mut sessions = self.active_sessions.write().await;
        sessions.remove(&session_id);
        
        let mut tasks = self.pairing_tasks.write().await;
        tasks.remove(&session_id);
        
        Ok(())
    }
    
    /// Generate pairing code immediately without waiting for peer connection
    async fn generate_pairing_code_immediately(
        &self,
        session_id: Uuid,
        network_identity: NetworkIdentity,
        password: String,
    ) -> Result<String> {
        debug!("Generating pairing code for session {}", session_id);
        
        // Generate pairing code directly using the PairingCode struct
        // This is immediate and doesn't require LibP2P setup
        let pairing_code = crate::networking::pairing::PairingCode::generate()?;
        let code_string = pairing_code.as_string();
        
        debug!("Generated pairing code: {}... for session {}", 
               code_string.split_whitespace().take(3).collect::<Vec<_>>().join(" "),
               session_id);
        
        Ok(code_string)
    }
    
    /// Start background task to listen for pairing connections
    async fn start_background_pairing_listener(
        &self,
        session_id: Uuid,
        auto_accept: bool,
        network_identity: NetworkIdentity,
        password: String,
        networking_service: Arc<NetworkingServiceRef>,
        active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    ) -> Result<()> {
        debug!("Starting background pairing listener for session {}", session_id);
        
        // Update session status to indicate we're ready for connections
        {
            let mut sessions = active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = PairingStatus::WaitingForConnection;
            }
        }
        
        // For now, we'll just mark as waiting for connection
        // The real pairing will be handled by the LibP2P protocol running
        // in the subprocess's main event loop, not in background threads
        info!("Background pairing listener ready for session {} (subprocess handles LibP2P directly)", session_id);
        Ok(())
    }
    
    
    /// Mark session as failed
    async fn mark_session_failed(&self, session_id: Uuid, reason: String) {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.status = PairingStatus::Failed(reason);
        }
    }
    
}


/// UI interface for pairing that updates session status
struct BridgePairingUI {
    session_id: Uuid,
    sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
}

impl BridgePairingUI {
    fn new(
        session_id: Uuid,
        sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    ) -> Self {
        Self { session_id, sessions }
    }
    
    async fn update_session_status(&self, status: PairingStatus) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&self.session_id) {
            session.status = status;
        }
    }
    
    async fn update_session_code(&self, code: String) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&self.session_id) {
            session.code = code;
        }
    }
}

#[async_trait]
impl PairingUserInterface for BridgePairingUI {
    async fn confirm_pairing(&self, _device_info: &DeviceInfo) -> crate::networking::Result<bool> {
        // For now, always auto-approve in persistent pairing
        // In the future, this could check the session's auto_accept flag
        // and potentially prompt the user through the daemon/CLI interface
        Ok(true)
    }
    
    async fn show_pairing_progress(&self, state: PairingState) {
        let status = match &state {
            PairingState::GeneratingCode => PairingStatus::WaitingForConnection,
            PairingState::Broadcasting => PairingStatus::WaitingForConnection,
            PairingState::Scanning => PairingStatus::WaitingForConnection,
            PairingState::Connecting => PairingStatus::Connected,
            PairingState::Authenticating => PairingStatus::Authenticating,
            PairingState::ExchangingKeys => PairingStatus::Authenticating,
            PairingState::EstablishingSession => PairingStatus::Authenticating,
            PairingState::Completed => PairingStatus::Completed,
            PairingState::Failed(reason) => PairingStatus::Failed(reason.clone()),
            _ => return, // Don't update for other states
        };
        
        debug!("Pairing progress for session {}: {:?}", self.session_id, state);
        self.update_session_status(status).await;
    }
    
    async fn show_pairing_error(&self, error: &crate::networking::NetworkError) {
        error!("Pairing error for session {}: {}", self.session_id, error);
        self.update_session_status(PairingStatus::Failed(error.to_string())).await;
    }
    
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
        info!("Generated pairing code: {} (expires in {} seconds)", code, expires_in_seconds);
        self.update_session_code(code.to_string()).await;
    }
    
    async fn prompt_pairing_code(&self) -> crate::networking::Result<[String; 12]> {
        // This shouldn't be called in the bridge implementation
        // since we receive the pairing code from the user via CLI
        Err(crate::networking::NetworkError::NotInitialized(
            "prompt_pairing_code not supported in bridge implementation".to_string()
        ))
    }
}