//! Device connection management for persistent connections
//!
//! Manages individual connections to paired devices, handling encryption, message routing,
//! keep-alive, and connection lifecycle for each trusted device.

use chrono::{DateTime, Utc, Duration};
use libp2p::{Multiaddr, PeerId, Swarm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::networking::{
    NetworkError, Result, DeviceInfo, SpacedriveBehaviour,
};
use super::{
    messages::DeviceMessage,
    identity::{PairedDeviceRecord, SessionKeys, SessionState, ActiveSession, ConnectionRecord, ConnectionResult, TransportType},
};

/// Request ID for tracking message responses
pub type RequestId = u64;

/// Represents an active connection to a paired device
pub struct DeviceConnection {
    /// Remote device information
    device_info: DeviceInfo,
    
    /// LibP2P peer ID
    peer_id: PeerId,
    
    /// Session keys for this connection
    session_keys: SessionKeys,
    
    /// Connection state
    state: ConnectionState,
    
    /// Last activity timestamp
    last_activity: DateTime<Utc>,
    
    /// Connection established timestamp
    connected_at: DateTime<Utc>,
    
    /// Keep-alive scheduler
    keepalive: KeepaliveScheduler,
    
    /// Request/response handlers
    request_handlers: HashMap<RequestId, PendingRequest>,
    
    /// Message queue for outbound messages
    outbound_queue: Vec<QueuedMessage>,
    
    /// Connection metrics
    metrics: ConnectionMetrics,
    
    /// Last known remote addresses
    remote_addresses: Vec<Multiaddr>,
    
    /// Message channel for sending to connection manager
    event_sender: Option<mpsc::UnboundedSender<ConnectionEvent>>,
}

/// Connection state tracking
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Attempting to establish connection
    Connecting,
    /// Performing authentication handshake
    Authenticating,
    /// Fully established and authenticated
    Connected,
    /// Attempting to reconnect after failure
    Reconnecting,
    /// Connection lost, not attempting to reconnect
    Disconnected,
    /// Connection failed with error
    Failed(String),
    /// Gracefully closed
    Closed,
}

/// Keep-alive scheduler for connection health
pub struct KeepaliveScheduler {
    /// Interval between keep-alive messages
    interval: Duration,
    /// Last keep-alive sent
    last_sent: DateTime<Utc>,
    /// Last keep-alive response received
    last_received: Option<DateTime<Utc>>,
    /// Number of missed keep-alives
    missed_count: u32,
    /// Maximum missed before considering connection dead
    max_missed: u32,
}

/// Pending request awaiting response
#[derive(Debug)]
pub struct PendingRequest {
    /// Original message sent
    message: DeviceMessage,
    /// When request was sent
    sent_at: DateTime<Utc>,
    /// Request timeout
    timeout: DateTime<Utc>,
    /// Response channel
    response_sender: Option<mpsc::UnboundedSender<DeviceMessage>>,
}

/// Queued outbound message
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    /// Message to send
    message: DeviceMessage,
    /// When message was queued
    queued_at: DateTime<Utc>,
    /// Priority level
    priority: MessagePriority,
    /// Request ID for tracking responses
    request_id: Option<RequestId>,
}

/// Message priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    /// Critical system messages (keep-alive, session management)
    Critical = 0,
    /// High priority (real-time updates, user interactions)
    High = 1,
    /// Normal priority (sync operations, file transfers)
    Normal = 2,
    /// Low priority (background tasks, maintenance)
    Low = 3,
}

/// Connection metrics and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetrics {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Number of failed message sends
    pub send_failures: u64,
    /// Average round-trip time in milliseconds
    pub avg_rtt_ms: f64,
    /// Current round-trip time measurements
    rtt_samples: Vec<u64>,
    /// Connection uptime
    pub uptime_secs: u64,
    /// Last ping time
    last_ping: Option<DateTime<Utc>>,
}

/// Events emitted by device connections
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// Connection state changed
    StateChanged {
        device_id: Uuid,
        old_state: ConnectionState,
        new_state: ConnectionState,
    },
    /// Message received from device
    MessageReceived {
        device_id: Uuid,
        message: DeviceMessage,
    },
    /// Message send failed
    SendFailed {
        device_id: Uuid,
        message: DeviceMessage,
        error: String,
    },
    /// Keep-alive timeout
    KeepaliveTimeout {
        device_id: Uuid,
        missed_count: u32,
    },
    /// Connection metrics updated
    MetricsUpdated {
        device_id: Uuid,
        metrics: ConnectionMetrics,
    },
}

impl KeepaliveScheduler {
    /// Create new keep-alive scheduler
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_sent: Utc::now(),
            last_received: None,
            missed_count: 0,
            max_missed: 3,
        }
    }
    
    /// Check if keep-alive should be sent
    pub fn should_send_keepalive(&self) -> bool {
        Utc::now().signed_duration_since(self.last_sent) >= self.interval
    }
    
    /// Mark keep-alive as sent
    pub fn mark_sent(&mut self) {
        self.last_sent = Utc::now();
    }
    
    /// Mark keep-alive response received
    pub fn mark_received(&mut self) {
        self.last_received = Some(Utc::now());
        self.missed_count = 0;
    }
    
    /// Check if connection is considered dead
    pub fn is_connection_dead(&mut self) -> bool {
        if self.should_send_keepalive() {
            self.missed_count += 1;
        }
        self.missed_count >= self.max_missed
    }
}

impl ConnectionMetrics {
    /// Create new connection metrics
    pub fn new() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
            send_failures: 0,
            avg_rtt_ms: 0.0,
            rtt_samples: Vec::new(),
            uptime_secs: 0,
            last_ping: None,
        }
    }
    
    /// Record message sent
    pub fn record_send(&mut self, message_size: usize) {
        self.bytes_sent += message_size as u64;
        self.messages_sent += 1;
    }
    
    /// Record message received
    pub fn record_receive(&mut self, message_size: usize) {
        self.bytes_received += message_size as u64;
        self.messages_received += 1;
    }
    
    /// Record send failure
    pub fn record_send_failure(&mut self) {
        self.send_failures += 1;
    }
    
    /// Record RTT measurement
    pub fn record_rtt(&mut self, rtt_ms: u64) {
        self.rtt_samples.push(rtt_ms);
        
        // Keep only recent samples
        const MAX_SAMPLES: usize = 100;
        if self.rtt_samples.len() > MAX_SAMPLES {
            self.rtt_samples.drain(0..self.rtt_samples.len() - MAX_SAMPLES);
        }
        
        // Update average
        self.avg_rtt_ms = self.rtt_samples.iter().map(|&x| x as f64).sum::<f64>() 
            / self.rtt_samples.len() as f64;
    }
    
    /// Update uptime
    pub fn update_uptime(&mut self, connected_at: DateTime<Utc>) {
        self.uptime_secs = Utc::now()
            .signed_duration_since(connected_at)
            .num_seconds()
            .max(0) as u64;
    }
}

impl DeviceConnection {
    /// Create new device connection
    pub fn new(
        device_info: DeviceInfo,
        session_keys: SessionKeys,
        event_sender: Option<mpsc::UnboundedSender<ConnectionEvent>>,
    ) -> Result<Self> {
        // Convert device fingerprint to peer ID
        let peer_id = Self::device_to_peer_id(&device_info)?;
        
        Ok(Self {
            device_info,
            peer_id,
            session_keys,
            state: ConnectionState::Connecting,
            last_activity: Utc::now(),
            connected_at: Utc::now(),
            keepalive: KeepaliveScheduler::new(Duration::seconds(30)),
            request_handlers: HashMap::new(),
            outbound_queue: Vec::new(),
            metrics: ConnectionMetrics::new(),
            remote_addresses: Vec::new(),
            event_sender,
        })
    }
    
    /// Establish connection to a paired device
    pub async fn establish(
        swarm: &mut Swarm<SpacedriveBehaviour>,
        device_record: &PairedDeviceRecord,
        session_keys: Option<SessionKeys>,
        event_sender: Option<mpsc::UnboundedSender<ConnectionEvent>>,
    ) -> Result<Self> {
        let device_info = device_record.device_info.clone();
        let keys = session_keys.unwrap_or_else(|| SessionKeys::new());
        
        // Convert device fingerprint to peer ID
        let peer_id = Self::device_to_peer_id(&device_info)?;
        
        // Try known addresses first
        for addr_str in &device_record.connection_config.known_addresses {
            if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                if let Err(e) = swarm.dial(addr.clone()) {
                    tracing::debug!("Failed to dial {}: {}", addr, e);
                } else {
                    tracing::debug!("Dialing known address: {}", addr);
                }
            }
        }
        
        // Start DHT discovery for this peer
        let _query_id = swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
        
        let mut connection = Self {
            device_info,
            peer_id,
            session_keys: keys,
            state: ConnectionState::Connecting,
            last_activity: Utc::now(),
            connected_at: Utc::now(),
            keepalive: KeepaliveScheduler::new(
                Duration::seconds(device_record.connection_config.keepalive_interval_secs as i64)
            ),
            request_handlers: HashMap::new(),
            outbound_queue: Vec::new(),
            metrics: ConnectionMetrics::new(),
            remote_addresses: Vec::new(),
            event_sender,
        };
        
        // Send connection establishment message
        let establish_msg = DeviceMessage::ConnectionEstablish {
            device_info: connection.device_info.clone(),
            protocol_version: 1,
            capabilities: vec!["sync".to_string(), "file-transfer".to_string(), "spacedrop".to_string()],
        };
        
        connection.queue_message(establish_msg, MessagePriority::Critical);
        
        Ok(connection)
    }
    
    /// Convert device info to LibP2P peer ID
    fn device_to_peer_id(device_info: &DeviceInfo) -> Result<PeerId> {
        // Use deterministic peer ID generation from device fingerprint
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(b"spacedrive-peer-id-v1");
        hasher.update(device_info.network_fingerprint.as_bytes());
        let hash = hasher.finalize();
        
        // Use first 32 bytes as Ed25519 seed for peer ID
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&hash.as_bytes()[..32]);
        
        let keypair = libp2p::identity::Keypair::ed25519_from_bytes(seed)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create peer ID: {}", e)))?;
        
        Ok(keypair.public().to_peer_id())
    }
    
    /// Send a message to this device
    pub async fn send_message(
        &mut self,
        swarm: &mut Swarm<SpacedriveBehaviour>,
        message: DeviceMessage,
    ) -> Result<()> {
        if !matches!(self.state, ConnectionState::Connected) {
            return Err(NetworkError::ConnectionFailed(
                format!("Connection not established (state: {:?})", self.state)
            ));
        }
        
        // Encrypt message with session keys
        let encrypted = self.encrypt_message(&message)?;
        
        // Send via libp2p request-response
        let request_id = swarm.behaviour_mut()
            .request_response
            .send_request(&self.peer_id, encrypted);
        
        // Track pending request
        let request_id_u64 = format!("{:?}", request_id).parse::<u64>().unwrap_or(0);
        self.request_handlers.insert(request_id_u64, PendingRequest::new(message.clone()));
        
        // Update metrics
        let message_size = message.estimated_size();
        self.metrics.record_send(message_size);
        self.last_activity = Utc::now();
        
        // Handle ping messages for RTT measurement
        if let DeviceMessage::Ping { timestamp } = &message {
            self.metrics.last_ping = Some(*timestamp);
        }
        
        tracing::debug!("Sent {} message to device {}", message.message_type(), self.device_info.device_id);
        Ok(())
    }
    
    /// Queue message for sending
    pub fn queue_message(&mut self, message: DeviceMessage, priority: MessagePriority) {
        let request_id = if message.requires_auth() {
            Some(self.generate_request_id())
        } else {
            None
        };
        
        let queued = QueuedMessage {
            message,
            queued_at: Utc::now(),
            priority,
            request_id,
        };
        
        self.outbound_queue.push(queued);
        
        // Sort by priority (critical messages first)
        self.outbound_queue.sort_by(|a, b| a.priority.cmp(&b.priority));
    }
    
    /// Process outbound message queue
    pub async fn process_outbound_queue(
        &mut self,
        swarm: &mut Swarm<SpacedriveBehaviour>,
    ) -> Result<usize> {
        if !matches!(self.state, ConnectionState::Connected) {
            return Ok(0);
        }
        
        let mut sent_count = 0;
        let messages_to_send: Vec<_> = self.outbound_queue.drain(..).collect();
        
        for queued in messages_to_send {
            let message_clone = queued.message.clone();
            match self.send_message(swarm, message_clone.clone()).await {
                Ok(()) => {
                    sent_count += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to send queued message: {}", e);
                    self.metrics.record_send_failure();
                    
                    // Re-queue if it's a critical message
                    if queued.priority == MessagePriority::Critical {
                        self.outbound_queue.push(queued);
                    }
                    
                    if let Some(sender) = &self.event_sender {
                        let _ = sender.send(ConnectionEvent::SendFailed {
                            device_id: self.device_info.device_id,
                            message: message_clone,
                            error: e.to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(sent_count)
    }
    
    /// Handle incoming message from this device
    pub async fn handle_message(
        &mut self,
        encrypted_message: Vec<u8>,
    ) -> Result<Option<DeviceMessage>> {
        // Decrypt with session keys
        let message = self.decrypt_message(&encrypted_message)?;
        
        self.last_activity = Utc::now();
        self.metrics.record_receive(encrypted_message.len());
        
        // Handle system messages
        match &message {
            DeviceMessage::Keepalive => {
                self.keepalive.mark_received();
                self.send_keepalive_response().await?;
                return Ok(None);
            }
            DeviceMessage::KeepaliveResponse => {
                self.keepalive.mark_received();
                return Ok(None);
            }
            DeviceMessage::Pong { original_timestamp, response_timestamp } => {
                if let Some(ping_time) = self.metrics.last_ping {
                    if ping_time == *original_timestamp {
                        let rtt = response_timestamp
                            .signed_duration_since(ping_time)
                            .num_milliseconds() as u64;
                        self.metrics.record_rtt(rtt);
                    }
                }
                return Ok(None);
            }
            DeviceMessage::Ping { timestamp } => {
                let pong = DeviceMessage::Pong {
                    original_timestamp: *timestamp,
                    response_timestamp: Utc::now(),
                };
                self.queue_message(pong, MessagePriority::Critical);
                return Ok(None);
            }
            DeviceMessage::ConnectionClose { reason } => {
                tracing::info!("Device {} requested connection close: {}", self.device_info.device_id, reason);
                self.set_state(ConnectionState::Closed);
                return Ok(None);
            }
            _ => {}
        }
        
        // Emit message received event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(ConnectionEvent::MessageReceived {
                device_id: self.device_info.device_id,
                message: message.clone(),
            });
        }
        
        Ok(Some(message))
    }
    
    /// Send keep-alive response
    async fn send_keepalive_response(&mut self) -> Result<()> {
        self.queue_message(DeviceMessage::KeepaliveResponse, MessagePriority::Critical);
        Ok(())
    }
    
    /// Check if connection needs refresh or maintenance
    pub fn needs_maintenance(&mut self) -> Vec<MaintenanceAction> {
        let mut actions = Vec::new();
        
        // Check keep-alive timeout
        if self.keepalive.is_connection_dead() {
            actions.push(MaintenanceAction::KeepaliveTimeout);
        } else if self.keepalive.should_send_keepalive() {
            actions.push(MaintenanceAction::SendKeepalive);
        }
        
        // Check session key rotation
        if self.session_keys.needs_rotation(Duration::hours(24)) {
            actions.push(MaintenanceAction::RotateKeys);
        }
        
        // Check for stale requests
        let now = Utc::now();
        let expired_requests: Vec<_> = self.request_handlers
            .iter()
            .filter(|(_, req)| now > req.timeout)
            .map(|(&id, _)| id)
            .collect();
        
        if !expired_requests.is_empty() {
            actions.push(MaintenanceAction::CleanupRequests(expired_requests));
        }
        
        actions
    }
    
    /// Perform maintenance action
    pub async fn perform_maintenance(
        &mut self,
        action: MaintenanceAction,
        swarm: &mut Swarm<SpacedriveBehaviour>,
    ) -> Result<()> {
        match action {
            MaintenanceAction::SendKeepalive => {
                self.queue_message(DeviceMessage::Keepalive, MessagePriority::Critical);
                self.keepalive.mark_sent();
            }
            MaintenanceAction::KeepaliveTimeout => {
                tracing::warn!("Keep-alive timeout for device {}", self.device_info.device_id);
                self.set_state(ConnectionState::Disconnected);
                
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(ConnectionEvent::KeepaliveTimeout {
                        device_id: self.device_info.device_id,
                        missed_count: self.keepalive.missed_count,
                    });
                }
            }
            MaintenanceAction::RotateKeys => {
                tracing::info!("Rotating session keys for device {}", self.device_info.device_id);
                // Key rotation would be handled by the connection manager
            }
            MaintenanceAction::CleanupRequests(expired_ids) => {
                for id in expired_ids {
                    self.request_handlers.remove(&id);
                }
            }
        }
        
        Ok(())
    }
    
    /// Update connection metrics
    pub fn update_metrics(&mut self) {
        self.metrics.update_uptime(self.connected_at);
        
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(ConnectionEvent::MetricsUpdated {
                device_id: self.device_info.device_id,
                metrics: self.metrics.clone(),
            });
        }
    }
    
    /// Set connection state and emit event
    fn set_state(&mut self, new_state: ConnectionState) {
        let old_state = std::mem::replace(&mut self.state, new_state.clone());
        
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(ConnectionEvent::StateChanged {
                device_id: self.device_info.device_id,
                old_state,
                new_state,
            });
        }
    }
    
    /// Close connection gracefully
    pub async fn close(&mut self) -> Result<()> {
        self.queue_message(
            DeviceMessage::ConnectionClose {
                reason: "Graceful shutdown".to_string(),
            },
            MessagePriority::Critical,
        );
        
        self.set_state(ConnectionState::Closed);
        Ok(())
    }
    
    /// Generate unique request ID
    fn generate_request_id(&self) -> RequestId {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        self.device_info.device_id.hash(&mut hasher);
        Utc::now().timestamp_nanos().hash(&mut hasher);
        hasher.finish()
    }
    
    /// Encrypt message with session keys
    fn encrypt_message(&self, message: &DeviceMessage) -> Result<Vec<u8>> {
        use ring::aead;
        
        // Serialize message
        let json_data = serde_json::to_vec(message)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to serialize message: {}", e)))?;
        
        // Generate nonce
        let mut nonce_bytes = [0u8; 12];
        use ring::rand::{SystemRandom, SecureRandom};
        let rng = SystemRandom::new();
        rng.fill(&mut nonce_bytes)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate nonce: {:?}", e)))?;
        
        // Encrypt with send key
        let unbound_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &self.session_keys.send_key)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create encryption key: {:?}", e)))?;
        let sealing_key = aead::LessSafeKey::new(unbound_key);
        
        let mut ciphertext = json_data;
        sealing_key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce_bytes),
                aead::Aad::empty(),
                &mut ciphertext,
            )
            .map_err(|e| NetworkError::EncryptionError(format!("Encryption failed: {:?}", e)))?;
        
        // Prepend nonce to ciphertext
        let mut encrypted = Vec::new();
        encrypted.extend_from_slice(&nonce_bytes);
        encrypted.extend_from_slice(&ciphertext);
        
        Ok(encrypted)
    }
    
    /// Decrypt message with session keys
    fn decrypt_message(&self, encrypted_data: &[u8]) -> Result<DeviceMessage> {
        use ring::aead;
        
        if encrypted_data.len() < 12 {
            return Err(NetworkError::EncryptionError("Invalid encrypted data length".to_string()));
        }
        
        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(nonce_bytes);
        
        // Decrypt with receive key
        let unbound_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &self.session_keys.receive_key)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create decryption key: {:?}", e)))?;
        let opening_key = aead::LessSafeKey::new(unbound_key);
        
        let mut ciphertext = ciphertext.to_vec();
        let plaintext = opening_key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::empty(),
                &mut ciphertext,
            )
            .map_err(|e| NetworkError::EncryptionError(format!("Decryption failed: {:?}", e)))?;
        
        // Deserialize message
        let message: DeviceMessage = serde_json::from_slice(plaintext)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to deserialize message: {}", e)))?;
        
        Ok(message)
    }
    
    /// Get connection state
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }
    
    /// Get device info
    pub fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
    
    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }
    
    /// Get connection metrics
    pub fn metrics(&self) -> &ConnectionMetrics {
        &self.metrics
    }
}

/// Maintenance actions for connections
#[derive(Debug, Clone)]
pub enum MaintenanceAction {
    SendKeepalive,
    KeepaliveTimeout,
    RotateKeys,
    CleanupRequests(Vec<RequestId>),
}

impl PendingRequest {
    /// Create new pending request
    pub fn new(message: DeviceMessage) -> Self {
        Self {
            message,
            sent_at: Utc::now(),
            timeout: Utc::now() + Duration::seconds(30),
            response_sender: None,
        }
    }
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self::new()
    }
}