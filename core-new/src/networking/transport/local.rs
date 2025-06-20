//! Local P2P transport using mDNS discovery and QUIC

use async_trait::async_trait;
use quinn::{Endpoint, ServerConfig, ClientConfig, Connection};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::networking::{
    connection::NetworkConnection,
    identity::{DeviceId, DeviceIdentity, DeviceInfo},
    security::NoiseSession,
    transport::Transport,
    Result, NetworkError,
};

/// Local network transport using mDNS + QUIC
pub struct LocalTransport {
    /// QUIC endpoint
    endpoint: Option<Endpoint>,
    
    /// mDNS service discovery
    mdns: Arc<RwLock<Option<mdns::Service>>>,
    
    /// Local address
    local_addr: Option<SocketAddr>,
}

impl LocalTransport {
    /// Create a new local transport
    pub fn new() -> Self {
        Self {
            endpoint: None,
            mdns: Arc::new(RwLock::new(None)),
            local_addr: None,
        }
    }

    /// Initialize the transport with device identity
    pub async fn initialize(&mut self, identity: &DeviceIdentity) -> Result<()> {
        // Setup QUIC endpoint
        let server_config = self.create_server_config(identity)?;
        
        let endpoint = Endpoint::server(server_config, "0.0.0.0:0".parse().unwrap())
            .map_err(|e| NetworkError::TransportError(format!("QUIC endpoint creation failed: {}", e)))?;
        
        self.local_addr = Some(endpoint.local_addr().unwrap());
        self.endpoint = Some(endpoint);

        // Setup mDNS service
        self.setup_mdns_service(identity).await?;
        
        Ok(())
    }

    /// Create QUIC server configuration
    fn create_server_config(&self, identity: &DeviceIdentity) -> Result<ServerConfig> {
        // For now, we'll use a dummy TLS config
        // In production, this would be replaced with Noise Protocol integration
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| NetworkError::TransportError(format!("Certificate generation failed: {}", e)))?;
        
        let cert_der = cert.serialize_der()
            .map_err(|e| NetworkError::TransportError(format!("Certificate serialization failed: {}", e)))?;
        let priv_key = cert.serialize_private_key_der();
        
        let cert_chain = vec![rustls::Certificate(cert_der)];
        let key = rustls::PrivateKey(priv_key);
        
        let server_config = ServerConfig::with_single_cert(cert_chain, key)
            .map_err(|e| NetworkError::TransportError(format!("TLS config creation failed: {}", e)))?;
        
        Ok(server_config)
    }

    /// Setup mDNS service advertisement
    async fn setup_mdns_service(&self, identity: &DeviceIdentity) -> Result<()> {
        let local_addr = self.local_addr.ok_or_else(|| {
            NetworkError::TransportError("No local address available".to_string())
        })?;

        // Create mDNS service
        let service = mdns::Service::new(
            format!("spacedrive-{}", identity.device_id),
            "_spacedrive._tcp".to_string(),
            local_addr.port(),
        );

        let mut mdns_lock = self.mdns.write().await;
        *mdns_lock = Some(service);
        
        Ok(())
    }

    /// Discover devices on the local network
    pub async fn discover_devices(&self) -> Result<Vec<DiscoveredDevice>> {
        let mdns_lock = self.mdns.read().await;
        let _service = mdns_lock.as_ref().ok_or_else(|| {
            NetworkError::TransportError("mDNS service not initialized".to_string())
        })?;

        // TODO: Implement actual mDNS discovery
        // For now, return empty list
        Ok(Vec::new())
    }
}

#[async_trait]
impl Transport for LocalTransport {
    async fn connect(
        &self,
        device_id: DeviceId,
        identity: &DeviceIdentity,
    ) -> Result<Box<dyn NetworkConnection>> {
        // Discover the target device
        let devices = self.discover_devices().await?;
        let target_device = devices.iter()
            .find(|d| d.device_id == device_id)
            .ok_or_else(|| NetworkError::DeviceNotFound(device_id))?;

        // Connect via QUIC
        let endpoint = self.endpoint.as_ref().ok_or_else(|| {
            NetworkError::TransportError("QUIC endpoint not initialized".to_string())
        })?;

        let client_config = self.create_client_config()?;
        let connection = endpoint
            .connect_with(client_config, target_device.addr, "localhost")
            .map_err(|e| NetworkError::ConnectionFailed(format!("QUIC connection failed: {}", e)))?
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("QUIC handshake failed: {}", e)))?;

        // Create secure connection wrapper
        let device_info = DeviceInfo::new(
            device_id,
            target_device.name.clone(),
            identity.public_key.clone(), // TODO: Get actual remote public key
        );

        Ok(Box::new(LocalConnection::new(connection, device_info)))
    }

    async fn listen(&self, identity: &DeviceIdentity) -> Result<()> {
        self.setup_mdns_service(identity).await?;
        Ok(())
    }

    async fn stop_listening(&self) -> Result<()> {
        let mut mdns_lock = self.mdns.write().await;
        *mdns_lock = None;
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "local"
    }

    async fn is_available(&self) -> bool {
        // Check if we can bind to a local address
        std::net::TcpListener::bind("0.0.0.0:0").is_ok()
    }
}

/// Create QUIC client configuration
impl LocalTransport {
    fn create_client_config(&self) -> Result<ClientConfig> {
        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(SkipVerification))
            .with_no_client_auth();

        let client_config = ClientConfig::new(Arc::new(crypto));
        Ok(client_config)
    }
}

/// Skip certificate verification for local connections
struct SkipVerification;

impl rustls::client::ServerCertVerifier for SkipVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

/// Discovered device on local network
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub device_id: DeviceId,
    pub name: String,
    pub addr: SocketAddr,
}

/// Local QUIC connection wrapper
pub struct LocalConnection {
    connection: Connection,
    device_info: DeviceInfo,
    is_connected: bool,
}

impl LocalConnection {
    pub fn new(connection: Connection, device_info: DeviceInfo) -> Self {
        Self {
            connection,
            device_info,
            is_connected: true,
        }
    }
}

#[async_trait]
impl NetworkConnection for LocalConnection {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        if !self.is_connected {
            return Err(NetworkError::ConnectionFailed("Connection closed".to_string()));
        }

        let mut send_stream = self.connection
            .open_uni()
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to open stream: {}", e)))?;

        use tokio::io::AsyncWriteExt;
        send_stream.write_all(data).await
            .map_err(|e| NetworkError::IoError(e))?;
        
        send_stream.finish().await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to finish stream: {}", e)))?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<Vec<u8>> {
        if !self.is_connected {
            return Err(NetworkError::ConnectionFailed("Connection closed".to_string()));
        }

        let mut recv_stream = self.connection
            .accept_uni()
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to accept stream: {}", e)))?;

        use tokio::io::AsyncReadExt;
        let mut buffer = Vec::new();
        recv_stream.read_to_end(&mut buffer).await
            .map_err(|e| NetworkError::IoError(e))?;

        Ok(buffer)
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
        self.is_connected && !self.connection.close_reason().is_some()
    }

    async fn close(&mut self) -> Result<()> {
        self.is_connected = false;
        self.connection.close(0u32.into(), b"closing");
        Ok(())
    }
}