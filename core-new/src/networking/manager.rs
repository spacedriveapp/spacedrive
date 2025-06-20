//! High-level networking API and manager

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::networking::{
    connection::{ConnectionManager, DeviceConnection, ConnectionStats, Transport},
    identity::{NetworkFingerprint, NetworkIdentity, DeviceInfo},
    pairing::{PairingCode, PairingManager, PairingDiscovery, PairingUserInterface, ConsolePairingUI, PairingProtocolHandler},
    // transport::{Transport, LocalTransport, RelayTransport}, // Disabled for now
    protocol::{FileTransfer, TransferProgress, SyncLogEntry},
    Result, NetworkError,
};
use uuid::Uuid;

/// Main networking interface
pub struct Network {
    /// Connection manager
    manager: Arc<ConnectionManager>,
    
    /// Our device identity
    identity: Arc<NetworkIdentity>,
    
    /// Known devices
    known_devices: Arc<RwLock<HashMap<Uuid, DeviceInfo>>>,
    
    /// Configuration
    config: NetworkConfig,
    
    /// Pairing manager
    pairing_manager: Arc<RwLock<PairingManager>>,
}

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Enable local P2P transport
    pub enable_local: bool,
    
    /// Enable relay transport
    pub enable_relay: bool,
    
    /// Relay server URL
    pub relay_url: Option<String>,
    
    /// Authentication token for relay
    pub auth_token: Option<String>,
    
    /// Maximum concurrent connections
    pub max_connections: usize,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enable_local: true,
            enable_relay: true,
            relay_url: Some("wss://relay.spacedrive.com".to_string()),
            auth_token: None,
            max_connections: 50,
            connection_timeout: 30,
        }
    }
}

impl Network {
    /// Create a new network instance
    pub async fn new(identity: NetworkIdentity, config: NetworkConfig) -> Result<Self> {
        let identity = Arc::new(identity);
        let manager = Arc::new(ConnectionManager::new(identity.clone()));
        
        let network = Self {
            manager,
            identity,
            known_devices: Arc::new(RwLock::new(HashMap::new())),
            config,
            pairing_manager: Arc::new(RwLock::new(PairingManager::new())),
        };
        
        network.initialize_transports().await?;
        
        Ok(network)
    }
    
    /// Initialize transport layers
    async fn initialize_transports(&self) -> Result<()> {
        // TODO: Implement transport initialization when dependencies are re-enabled
        // For now, just return OK since we don't have any transports
        tracing::warn!("Transport initialization skipped - transports disabled");
        Ok(())
    }
    
    /// Connect to a device (auto-selects transport)
    pub async fn connect(&self, device_id: Uuid) -> Result<Arc<RwLock<DeviceConnection>>> {
        // Try local first, then fall back to relay
        if let Ok(conn) = self.manager.connect_local(device_id).await {
            let device_info = conn.device_info().clone();
            self.update_known_device(device_info).await;
            return Ok(Arc::new(RwLock::new(conn)));
        }
        
        // Fall back to relay
        let conn = self.manager.connect_relay(device_id).await?;
        let device_info = conn.device_info().clone();
        self.update_known_device(device_info).await;
        Ok(Arc::new(RwLock::new(conn)))
    }
    
    /// Share file with device
    pub async fn share_file<F>(
        &self,
        device_id: Uuid,
        file_path: &Path,
        progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(TransferProgress),
    {
        let connection_arc = self.connect(device_id).await?;
        let mut connection = connection_arc.write().await;
        
        FileTransfer::send_file(&mut *connection, file_path, progress_callback).await
    }
    
    /// Receive file from device
    pub async fn receive_file<F>(
        &self,
        device_id: Uuid,
        output_path: &Path,
        progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(TransferProgress),
    {
        let connection_arc = self.connect(device_id).await?;
        let mut connection = connection_arc.write().await;
        
        FileTransfer::receive_file(&mut *connection, output_path, progress_callback).await?;
        Ok(())
    }
    
    /// Sync with device
    pub async fn sync_with(
        &self,
        device_id: Uuid,
        from_seq: u64,
    ) -> Result<Vec<SyncLogEntry>> {
        let connection_arc = self.connect(device_id).await?;
        let mut connection = connection_arc.write().await;
        
        let response = connection.sync_pull(from_seq, Some(1000)).await?;
        Ok(response.changes)
    }
    
    /// Start device pairing process as initiator
    pub async fn initiate_pairing(&self) -> Result<PairingCode> {
        self.initiate_pairing_with_ui(&ConsolePairingUI).await
    }
    
    /// Start device pairing process with custom UI
    pub async fn initiate_pairing_with_ui<UI: PairingUserInterface>(&self, ui: &UI) -> Result<PairingCode> {
        let code = PairingCode::generate()?;
        
        // Show the code to the user
        ui.show_pairing_code(&code.as_string(), code.time_remaining().unwrap_or_default().num_seconds() as u32).await;
        
        // Create device info for local device
        let device_info = self.identity.to_device_info();
        
        // Start discovery service
        let mut discovery = PairingDiscovery::new(device_info);
        
        // Bind to a random port for pairing
        let addr = std::net::SocketAddr::new(
            local_ip_address::local_ip().unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            0 // Let OS choose port
        );
        
        // Start broadcasting
        discovery.start_broadcast(&code, addr.port()).await?;
        
        tracing::info!("Pairing code generated: {} - listening for connections", code.as_string());
        
        Ok(code)
    }
    
    /// Complete device pairing as joiner
    pub async fn complete_pairing(&self, words: [String; 6]) -> Result<DeviceInfo> {
        self.complete_pairing_with_ui(words, &ConsolePairingUI).await
    }
    
    /// Complete device pairing with custom UI
    pub async fn complete_pairing_with_ui<UI: PairingUserInterface>(
        &self, 
        words: [String; 6],
        ui: &UI
    ) -> Result<DeviceInfo> {
        let code = PairingCode::from_words(&words)?;
        
        if code.is_expired() {
            return Err(NetworkError::AuthenticationFailed("Pairing code expired".to_string()));
        }
        
        // Create device info for local device
        let device_info = self.identity.to_device_info();
        
        // Create discovery service
        let discovery = PairingDiscovery::new(device_info.clone());
        
        // Scan for the pairing device
        ui.show_pairing_progress(crate::networking::PairingState::Scanning).await;
        let target = discovery.scan_for_pairing_device(&code, std::time::Duration::from_secs(30)).await?;
        
        // Connect to the target
        ui.show_pairing_progress(crate::networking::PairingState::Connecting).await;
        let mut connection = crate::networking::PairingConnection::connect_to_target(target, device_info).await?;
        
        // Unlock our private key for authentication
        let private_key = self.identity.unlock_private_key("password")
            .map_err(|e| NetworkError::AuthenticationFailed(format!("Failed to unlock private key: {}", e)))?;
        
        // Authenticate using challenge-response
        ui.show_pairing_progress(crate::networking::PairingState::Authenticating).await;
        PairingProtocolHandler::authenticate_as_joiner(&mut connection, &code).await?;
        
        // Exchange device information (joiner receives first)
        ui.show_pairing_progress(crate::networking::PairingState::ExchangingKeys).await;
        let remote_device = PairingProtocolHandler::exchange_device_information_as_joiner(&mut connection, &private_key).await?;
        
        // Ask user for confirmation
        ui.show_pairing_progress(crate::networking::PairingState::AwaitingConfirmation).await;
        let user_confirmed = ui.confirm_pairing(&remote_device).await?;
        
        if !user_confirmed {
            return Err(NetworkError::AuthenticationFailed("User rejected pairing".to_string()));
        }
        
        // Establish session keys (joiner receives first)
        ui.show_pairing_progress(crate::networking::PairingState::EstablishingSession).await;
        let _session_keys = PairingProtocolHandler::establish_session_keys_as_joiner(&mut connection).await?;
        
        // Add device to known devices
        self.add_known_device(remote_device.clone()).await;
        
        ui.show_pairing_progress(crate::networking::PairingState::Completed).await;
        
        tracing::info!("Pairing completed successfully with device: {}", remote_device.device_name);
        
        Ok(remote_device)
    }
    
    /// Discover devices on local network
    pub async fn discover_local_devices(&self) -> Result<Vec<DeviceInfo>> {
        // TODO: Use local transport to discover devices
        Ok(Vec::new())
    }
    
    /// Get all known devices
    pub async fn known_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.known_devices.read().await;
        devices.values().cloned().collect()
    }
    
    /// Add a known device
    pub async fn add_known_device(&self, device_info: DeviceInfo) {
        self.update_known_device(device_info).await;
    }
    
    /// Update known device information
    async fn update_known_device(&self, device_info: DeviceInfo) {
        let mut devices = self.known_devices.write().await;
        devices.insert(device_info.device_id, device_info);
    }
    
    /// Remove a known device
    pub async fn remove_known_device(&self, device_id: &Uuid) -> bool {
        let mut devices = self.known_devices.write().await;
        devices.remove(device_id).is_some()
    }
    
    /// Get our device identity
    pub fn identity(&self) -> &NetworkIdentity {
        &self.identity
    }
    
    /// Get connection statistics
    pub async fn connection_stats(&self) -> ConnectionStats {
        self.manager.connection_stats().await
    }
    
    /// Get active connections
    pub async fn active_connections(&self) -> Vec<Uuid> {
        self.manager.active_connections().await
    }
    
    /// Close connection to specific device
    pub async fn close_connection(&self, device_id: &Uuid) -> Result<()> {
        self.manager.close_connection(device_id).await
    }
    
    /// Close all connections
    pub async fn close_all_connections(&self) -> Result<()> {
        self.manager.close_all().await
    }
    
    /// Check if we can reach a device
    pub async fn ping_device(&self, device_id: Uuid) -> Result<std::time::Duration> {
        let start = std::time::Instant::now();
        let connection_arc = self.connect(device_id).await?;
        let mut connection = connection_arc.write().await;
        
        // Send ping message
        use crate::networking::protocol::{ProtocolMessage, ProtocolHandler};
        let ping_msg = ProtocolMessage::Ping {
            timestamp: chrono::Utc::now(),
        };
        
        ProtocolHandler::send_message(&mut *connection, ping_msg).await?;
        
        // Wait for pong
        let response = ProtocolHandler::receive_message(&mut *connection).await?;
        
        match response {
            ProtocolMessage::Pong { .. } => {
                Ok(start.elapsed())
            }
            _ => Err(NetworkError::ProtocolError("Expected pong response".to_string())),
        }
    }
    
    /// Start network services (listening, discovery, etc.)
    pub async fn start_services(&self) -> Result<()> {
        // TODO: Start listening on all transports
        // TODO: Start device discovery
        // TODO: Start background maintenance tasks
        
        tracing::info!("Network services started");
        Ok(())
    }
    
    /// Stop network services
    pub async fn stop_services(&self) -> Result<()> {
        self.close_all_connections().await?;
        
        // TODO: Stop listening on all transports
        // TODO: Stop discovery
        // TODO: Stop background tasks
        
        tracing::info!("Network services stopped");
        Ok(())
    }
    
    /// Update network configuration
    pub async fn update_config(&mut self, config: NetworkConfig) -> Result<()> {
        self.config = config;
        
        // Reinitialize transports with new config
        self.initialize_transports().await?;
        
        Ok(())
    }
    
    /// Get current network configuration
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

/// Network events for subscribers
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Device connected
    DeviceConnected {
        device_id: Uuid,
        device_info: DeviceInfo,
        transport_type: String,
    },
    
    /// Device disconnected
    DeviceDisconnected {
        device_id: Uuid,
        reason: String,
    },
    
    /// File transfer started
    FileTransferStarted {
        device_id: Uuid,
        transfer_id: uuid::Uuid,
        file_name: String,
        file_size: u64,
        is_outgoing: bool,
    },
    
    /// File transfer progress
    FileTransferProgress {
        transfer_id: uuid::Uuid,
        progress: TransferProgress,
    },
    
    /// File transfer completed
    FileTransferCompleted {
        transfer_id: uuid::Uuid,
        success: bool,
        error: Option<String>,
    },
    
    /// Device discovered
    DeviceDiscovered {
        device_info: DeviceInfo,
        transport_type: String,
    },
    
    /// Pairing request received
    PairingRequested {
        device_id: Uuid,
        device_name: String,
    },
    
    /// Network error
    NetworkError {
        error: String,
    },
}

/// Network event handler trait
pub trait NetworkEventHandler: Send + Sync {
    fn handle_event(&self, event: NetworkEvent);
}

/// Simple event bus for network events
pub struct NetworkEventBus {
    handlers: Arc<RwLock<Vec<Arc<dyn NetworkEventHandler>>>>,
}

impl NetworkEventBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub async fn add_handler(&self, handler: Arc<dyn NetworkEventHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }
    
    pub async fn emit(&self, event: NetworkEvent) {
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            handler.handle_event(event.clone());
        }
    }
}