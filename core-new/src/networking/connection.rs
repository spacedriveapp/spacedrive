//! Network connection abstraction and management

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::networking::{
    identity::{NetworkFingerprint, NetworkIdentity, DeviceInfo},
    Result, NetworkError,
};
use uuid::Uuid;

/// Transport trait for connection manager
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(
        &self,
        device_id: Uuid,
        identity: &NetworkIdentity,
    ) -> Result<Box<dyn NetworkConnection>>;
    
    fn transport_type(&self) -> &'static str;
}

/// Abstract network connection interface
#[async_trait]
pub trait NetworkConnection: Send + Sync {
    /// Send data reliably
    async fn send(&mut self, data: &[u8]) -> Result<()>;
    
    /// Receive data
    async fn receive(&mut self) -> Result<Vec<u8>>;
    
    /// Stream a file efficiently
    async fn send_file(&mut self, path: &Path) -> Result<()>;
    
    /// Receive file stream
    async fn receive_file(&mut self, path: &Path) -> Result<()>;
    
    /// Get remote device info
    fn remote_device(&self) -> &DeviceInfo;
    
    /// Check if connection is alive
    fn is_connected(&self) -> bool;
    
    /// Close the connection
    async fn close(&mut self) -> Result<()>;
}

/// High-level device connection wrapper
pub struct DeviceConnection {
    inner: Box<dyn NetworkConnection>,
    device_info: DeviceInfo,
}

impl DeviceConnection {
    pub fn new(connection: Box<dyn NetworkConnection>, device_info: DeviceInfo) -> Self {
        Self {
            inner: connection,
            device_info,
        }
    }

    /// Get device information
    pub fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    /// Send data to the device
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        self.inner.send(data).await
    }

    /// Receive data from the device
    pub async fn receive(&mut self) -> Result<Vec<u8>> {
        self.inner.receive().await
    }

    /// Send a file to the device
    pub async fn send_file(&mut self, path: &Path) -> Result<()> {
        self.inner.send_file(path).await
    }

    /// Receive a file from the device
    pub async fn receive_file(&mut self, path: &Path) -> Result<()> {
        self.inner.receive_file(path).await
    }

    /// Check if the connection is active
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }

    /// Sync operations
    pub async fn sync_pull(
        &mut self,
        from_seq: u64,
        limit: Option<usize>,
    ) -> Result<PullResponse> {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        struct PullRequest {
            from_seq: u64,
            limit: Option<usize>,
        }

        let request = PullRequest { from_seq, limit };
        let request_data = crate::networking::serialization::serialize_with_context(&request, "Failed to serialize sync request")?;
        
        self.send(&request_data).await?;
        
        let response_data = self.receive().await?;
        let response: PullResponse = crate::networking::serialization::deserialize_with_context(&response_data, "Failed to deserialize sync response")?;
        
        Ok(response)
    }
}

#[async_trait]
impl NetworkConnection for DeviceConnection {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        self.inner.send(data).await
    }
    
    async fn receive(&mut self) -> Result<Vec<u8>> {
        self.inner.receive().await
    }
    
    async fn send_file(&mut self, path: &Path) -> Result<()> {
        self.inner.send_file(path).await
    }
    
    async fn receive_file(&mut self, path: &Path) -> Result<()> {
        self.inner.receive_file(path).await
    }
    
    fn remote_device(&self) -> &DeviceInfo {
        &self.device_info
    }
    
    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
    
    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}

/// Response from sync pull operation
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct PullResponse {
    pub changes: Vec<crate::networking::protocol::SyncLogEntry>,
    pub has_more: bool,
    pub next_seq: u64,
}

/// Connection manager handles all transports and connections
pub struct ConnectionManager {
    /// Our device identity
    identity: Arc<NetworkIdentity>,
    
    /// Active connections
    connections: Arc<RwLock<HashMap<Uuid, Arc<RwLock<DeviceConnection>>>>>,
    
    /// Available transports
    transports: Vec<Box<dyn Transport>>,
    
    /// Connection pool settings
    max_connections: usize,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(identity: Arc<NetworkIdentity>) -> Self {
        Self {
            identity,
            connections: Arc::new(RwLock::new(HashMap::new())),
            transports: Vec::new(),
            max_connections: 50,
        }
    }

    /// Add a transport to the manager
    pub fn add_transport(&mut self, transport: Box<dyn Transport>) {
        self.transports.push(transport);
    }

    /// Get or create a connection to a device
    pub async fn get_or_connect(&self, device_id: Uuid) -> Result<Arc<RwLock<DeviceConnection>>> {
        // Check if we already have an active connection
        {
            let connections = self.connections.read().await;
            if let Some(conn) = connections.get(&device_id) {
                let conn_guard = conn.read().await;
                if conn_guard.is_connected() {
                    return Ok(conn.clone());
                }
            }
        }

        // Need to create a new connection
        let connection = self.connect_new(device_id).await?;
        let connection = Arc::new(RwLock::new(connection));

        // Store in connection pool
        {
            let mut connections = self.connections.write().await;
            connections.insert(device_id, connection.clone());
            
            // Enforce connection limit
            if connections.len() > self.max_connections {
                // Remove oldest connection
                if let Some((oldest_id, _)) = connections.iter().next() {
                    let oldest_id = *oldest_id;
                    connections.remove(&oldest_id);
                }
            }
        }

        Ok(connection)
    }

    /// Create a new connection using available transports
    async fn connect_new(&self, device_id: Uuid) -> Result<DeviceConnection> {
        let mut last_error = None;

        // Try each transport in order
        for transport in &self.transports {
            match transport.connect(device_id, &self.identity).await {
                Ok(connection) => {
                    let device_info = DeviceInfo::new(
                        device_id,
                        "Remote Device".to_string(), // TODO: Get actual device name
                        connection.remote_device().public_key.clone(),
                    );
                    return Ok(DeviceConnection::new(connection, device_info));
                }
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            NetworkError::ConnectionFailed("No transports available".to_string())
        }))
    }

    /// Connect using local transport
    pub async fn connect_local(&self, device_id: Uuid) -> Result<DeviceConnection> {
        for transport in &self.transports {
            if transport.transport_type() == "local" {
                let connection = transport.connect(device_id, &self.identity).await?;
                let device_info = DeviceInfo::new(
                    device_id,
                    "Local Device".to_string(),
                    connection.remote_device().public_key.clone(),
                );
                return Ok(DeviceConnection::new(connection, device_info));
            }
        }
        
        Err(NetworkError::TransportError("No local transport available".to_string()))
    }

    /// Connect using relay transport
    pub async fn connect_relay(&self, device_id: Uuid) -> Result<DeviceConnection> {
        for transport in &self.transports {
            if transport.transport_type() == "relay" {
                let connection = transport.connect(device_id, &self.identity).await?;
                let device_info = DeviceInfo::new(
                    device_id,
                    "Relay Device".to_string(),
                    connection.remote_device().public_key.clone(),
                );
                return Ok(DeviceConnection::new(connection, device_info));
            }
        }
        
        Err(NetworkError::TransportError("No relay transport available".to_string()))
    }

    /// Get all active connections
    pub async fn active_connections(&self) -> Vec<Uuid> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    /// Close a specific connection
    pub async fn close_connection(&self, device_id: &Uuid) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.remove(device_id) {
            let mut conn_guard = connection.write().await;
            conn_guard.close().await?;
        }
        Ok(())
    }

    /// Close all connections
    pub async fn close_all(&self) -> Result<()> {
        let mut connections = self.connections.write().await;
        
        for (_, connection) in connections.drain() {
            let mut conn_guard = connection.write().await;
            if let Err(e) = conn_guard.close().await {
                tracing::warn!("Error closing connection: {}", e);
            }
        }
        
        Ok(())
    }

    /// Get connection statistics
    pub async fn connection_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let total = connections.len();
        let mut active = 0;

        for connection in connections.values() {
            let conn_guard = connection.read().await;
            if conn_guard.is_connected() {
                active += 1;
            }
        }

        ConnectionStats {
            total_connections: total,
            active_connections: active,
            max_connections: self.max_connections,
        }
    }
}

/// Connection statistics
#[derive(Debug)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub active_connections: usize,
    pub max_connections: usize,
}