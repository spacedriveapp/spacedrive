//! Internet relay transport for NAT traversal

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, WebSocketStream, MaybeTlsStream, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

use crate::networking::{
    connection::NetworkConnection,
    identity::{DeviceId, DeviceIdentity, DeviceInfo},
    transport::Transport,
    Result, NetworkError,
};

/// Internet transport via relay service
pub struct RelayTransport {
    /// Relay server URL
    relay_url: String,
    
    /// WebSocket connection to relay
    relay_client: Arc<RwLock<Option<RelayClient>>>,
    
    /// Authentication token
    auth_token: Option<String>,
}

impl RelayTransport {
    /// Create a new relay transport
    pub fn new(relay_url: String) -> Self {
        Self {
            relay_url,
            relay_client: Arc::new(RwLock::new(None)),
            auth_token: None,
        }
    }

    /// Set authentication token
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Initialize connection to relay server
    pub async fn initialize(&self, identity: &DeviceIdentity) -> Result<()> {
        let client = RelayClient::connect(&self.relay_url).await?;
        
        // Authenticate with relay
        let auth_token = self.auth_token.as_ref().ok_or_else(|| {
            NetworkError::AuthenticationFailed("No auth token provided".to_string())
        })?;
        
        client.authenticate(identity, auth_token).await?;
        
        let mut relay_client_lock = self.relay_client.write().await;
        *relay_client_lock = Some(client);
        
        Ok(())
    }
}

#[async_trait]
impl Transport for RelayTransport {
    async fn connect(
        &self,
        device_id: DeviceId,
        identity: &DeviceIdentity,
    ) -> Result<Box<dyn NetworkConnection>> {
        let relay_client_lock = self.relay_client.read().await;
        let relay_client = relay_client_lock.as_ref().ok_or_else(|| {
            NetworkError::TransportError("Relay client not initialized".to_string())
        })?;

        // Request connection to target device
        let session = relay_client.connect_to(device_id).await?;
        
        let device_info = DeviceInfo::new(
            device_id,
            "Relay Device".to_string(),
            identity.public_key.clone(), // TODO: Get actual remote public key
        );

        Ok(Box::new(RelayConnection::new(
            relay_client.clone_client().await?,
            session,
            device_info,
        )))
    }

    async fn listen(&self, identity: &DeviceIdentity) -> Result<()> {
        self.initialize(identity).await
    }

    async fn stop_listening(&self) -> Result<()> {
        let mut relay_client_lock = self.relay_client.write().await;
        if let Some(client) = relay_client_lock.take() {
            client.disconnect().await?;
        }
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "relay"
    }

    async fn is_available(&self) -> bool {
        // Try to connect to relay server
        tokio_tungstenite::connect_async(&self.relay_url).await.is_ok()
    }
}

/// Relay client for WebSocket communication
pub struct RelayClient {
    websocket: Arc<RwLock<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>>,
    device_id: Option<DeviceId>,
}

impl RelayClient {
    /// Connect to relay server
    pub async fn connect(url: &str) -> Result<Self> {
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| NetworkError::ConnectionFailed(format!("WebSocket connection failed: {}", e)))?;

        Ok(Self {
            websocket: Arc::new(RwLock::new(ws_stream)),
            device_id: None,
        })
    }

    /// Authenticate with relay server
    pub async fn authenticate(&self, identity: &DeviceIdentity, auth_token: &str) -> Result<()> {
        let register_msg = RelayMessage::Register {
            device_id: identity.device_id,
            public_key: identity.public_key.clone(),
            auth_token: auth_token.to_string(),
        };

        self.send_message(register_msg).await?;
        
        // Wait for acknowledgment
        let response = self.receive_message().await?;
        match response {
            RelayMessage::RegisterAck => Ok(()),
            RelayMessage::Error { message } => {
                Err(NetworkError::AuthenticationFailed(message))
            }
            _ => Err(NetworkError::ProtocolError("Unexpected response".to_string())),
        }
    }

    /// Request connection to another device
    pub async fn connect_to(&self, device_id: DeviceId) -> Result<SessionId> {
        let connect_msg = RelayMessage::Connect {
            target_device_id: device_id,
            offer: SessionOffer::new(),
        };

        self.send_message(connect_msg).await?;
        
        // Wait for session establishment
        let response = self.receive_message().await?;
        match response {
            RelayMessage::ConnectAck { session_id } => Ok(session_id),
            RelayMessage::Error { message } => {
                Err(NetworkError::ConnectionFailed(message))
            }
            _ => Err(NetworkError::ProtocolError("Unexpected response".to_string())),
        }
    }

    /// Send message to relay
    async fn send_message(&self, message: RelayMessage) -> Result<()> {
        let data = bincode::encode_to_vec(&message, bincode::config::standard())?;
        let ws_message = Message::Binary(data);
        
        let mut websocket = self.websocket.write().await;
        websocket.send(ws_message).await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to send message: {}", e)))?;
        
        Ok(())
    }

    /// Receive message from relay
    async fn receive_message(&self) -> Result<RelayMessage> {
        let mut websocket = self.websocket.write().await;
        
        let message = websocket.next().await
            .ok_or_else(|| NetworkError::ConnectionFailed("Connection closed".to_string()))?
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to receive message: {}", e)))?;

        match message {
            Message::Binary(data) => {
                let (relay_message, _): (RelayMessage, usize) = bincode::decode_from_slice(&data, bincode::config::standard())?;
                Ok(relay_message)
            }
            Message::Close(_) => {
                Err(NetworkError::ConnectionFailed("Connection closed by relay".to_string()))
            }
            _ => Err(NetworkError::ProtocolError("Unexpected message type".to_string())),
        }
    }

    /// Send data through relay
    pub async fn send_data(&self, session_id: SessionId, data: Vec<u8>) -> Result<()> {
        let data_msg = RelayMessage::Data {
            session_id,
            encrypted_payload: data, // TODO: Encrypt with session key
        };
        
        self.send_message(data_msg).await
    }

    /// Receive data through relay
    pub async fn receive_data(&self) -> Result<(SessionId, Vec<u8>)> {
        let message = self.receive_message().await?;
        match message {
            RelayMessage::Data { session_id, encrypted_payload } => {
                // TODO: Decrypt with session key
                Ok((session_id, encrypted_payload))
            }
            _ => Err(NetworkError::ProtocolError("Expected data message".to_string())),
        }
    }

    /// Clone the client for use in connections
    pub async fn clone_client(&self) -> Result<RelayClient> {
        // For simplicity, we'll return a new client
        // In production, this would share the WebSocket connection
        Err(NetworkError::TransportError("Client cloning not implemented".to_string()))
    }

    /// Disconnect from relay
    pub async fn disconnect(&self) -> Result<()> {
        let mut websocket = self.websocket.write().await;
        websocket.close(None).await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to close connection: {}", e)))?;
        Ok(())
    }
}

/// Relay protocol messages
#[derive(Serialize, Deserialize, Debug)]
pub enum RelayMessage {
    /// Register device with relay
    Register {
        device_id: DeviceId,
        public_key: crate::networking::identity::PublicKey,
        auth_token: String,
    },
    
    /// Registration acknowledgment
    RegisterAck,
    
    /// Request connection to another device
    Connect {
        target_device_id: DeviceId,
        offer: SessionOffer,
    },
    
    /// Connection acknowledgment
    ConnectAck {
        session_id: SessionId,
    },
    
    /// Relay data between devices
    Data {
        session_id: SessionId,
        encrypted_payload: Vec<u8>, 
    },
    
    /// Error message
    Error {
        message: String,
    },
}

/// Session identifier for relay connections
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SessionId(u64);

impl SessionId {
    pub fn new() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        SessionId(rng.gen())
    }
}

/// Session offer for connection establishment
#[derive(Serialize, Deserialize, Debug)]
pub struct SessionOffer {
    pub session_id: SessionId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SessionOffer {
    pub fn new() -> Self {
        Self {
            session_id: SessionId::new(),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Relay connection wrapper
pub struct RelayConnection {
    relay_client: RelayClient,
    session_id: SessionId,
    device_info: DeviceInfo,
    is_connected: bool,
}

impl RelayConnection {
    pub fn new(
        relay_client: RelayClient,
        session_id: SessionId,
        device_info: DeviceInfo,
    ) -> Self {
        Self {
            relay_client,
            session_id,
            device_info,
            is_connected: true,
        }
    }
}

#[async_trait]
impl NetworkConnection for RelayConnection {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        if !self.is_connected {
            return Err(NetworkError::ConnectionFailed("Connection closed".to_string()));
        }

        self.relay_client.send_data(self.session_id, data.to_vec()).await
    }

    async fn receive(&mut self) -> Result<Vec<u8>> {
        if !self.is_connected {
            return Err(NetworkError::ConnectionFailed("Connection closed".to_string()));
        }

        let (_session_id, data) = self.relay_client.receive_data().await?;
        Ok(data)
    }

    async fn send_file(&mut self, path: &std::path::Path) -> Result<()> {
        use tokio::fs::File;
        use tokio::io::AsyncReadExt;

        let mut file = File::open(path).await
            .map_err(|e| NetworkError::IoError(e))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await
            .map_err(|e| NetworkError::IoError(e))?;

        self.send(&buffer).await
    }

    async fn receive_file(&mut self, path: &std::path::Path) -> Result<()> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let data = self.receive().await?;
        
        let mut file = File::create(path).await
            .map_err(|e| NetworkError::IoError(e))?;
        
        file.write_all(&data).await
            .map_err(|e| NetworkError::IoError(e))?;

        Ok(())
    }

    fn remote_device(&self) -> &DeviceInfo {
        &self.device_info
    }

    fn is_connected(&self) -> bool {
        self.is_connected
    }

    async fn close(&mut self) -> Result<()> {
        self.is_connected = false;
        // TODO: Send close message to relay
        Ok(())
    }
}