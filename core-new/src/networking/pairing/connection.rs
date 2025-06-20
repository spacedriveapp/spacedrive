//! Secure pairing connection with TLS and ephemeral certificates

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use rustls::{ClientConfig, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rcgen::{Certificate as RcgenCertificate, CertificateParams};

use crate::networking::{DeviceInfo, NetworkError, Result};
use super::PairingTarget; // PairingCode used in future certificate validation

/// Secure pairing connection using TLS with ephemeral certificates
pub struct PairingConnection {
    /// TLS stream
    stream: Box<dyn PairingStream>,
    /// Connection state
    state: PairingConnectionState,
    /// Local device information
    local_device: DeviceInfo,
    /// Remote device information (filled during pairing)
    remote_device: Option<DeviceInfo>,
    /// Whether we are the initiator
    is_initiator: bool,
}

/// Pairing connection state
#[derive(Debug, Clone, PartialEq)]
pub enum PairingConnectionState {
    Connecting,
    Authenticating,
    ExchangingKeys,
    AwaitingConfirmation,
    Completed,
    Failed(String),
}

/// Trait for TLS stream abstraction
pub trait PairingStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    fn peer_addr(&self) -> Result<SocketAddr>;
}

/// TLS client stream wrapper
pub struct TlsClientStream {
    stream: tokio_rustls::client::TlsStream<TcpStream>,
}

impl PairingStream for TlsClientStream {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream.get_ref().0.peer_addr()
            .map_err(|e| NetworkError::TransportError(format!("Failed to get peer addr: {}", e)))
    }
}

impl AsyncRead for TlsClientStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for TlsClientStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/// TLS server stream wrapper
pub struct TlsServerStream {
    stream: tokio_rustls::server::TlsStream<TcpStream>,
}

impl PairingStream for TlsServerStream {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream.get_ref().0.peer_addr()
            .map_err(|e| NetworkError::TransportError(format!("Failed to get peer addr: {}", e)))
    }
}

impl AsyncRead for TlsServerStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for TlsServerStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl PairingConnection {
    /// Connect to a pairing target as client
    pub async fn connect_to_target(
        target: PairingTarget,
        local_device: DeviceInfo,
    ) -> Result<Self> {
        // Create ephemeral TLS configuration
        let tls_config = Self::create_ephemeral_client_config()?;
        let connector = tokio_rustls::TlsConnector::from(Arc::new(tls_config));
        
        // Connect to target
        let addr = SocketAddr::new(target.address, target.port);
        let tcp_stream = TcpStream::connect(addr).await
            .map_err(|e| NetworkError::ConnectionFailed(format!("TCP connection failed: {}", e)))?;
        
        // Establish TLS connection
        let domain = ServerName::try_from("spacedrive-pairing")
            .map_err(|e| NetworkError::TransportError(format!("Invalid server name: {}", e)))?;
        
        let tls_stream = connector.connect(domain, tcp_stream).await
            .map_err(|e| NetworkError::TransportError(format!("TLS connection failed: {}", e)))?;
        
        let stream = Box::new(TlsClientStream { stream: tls_stream }) as Box<dyn PairingStream>;
        
        Ok(Self {
            stream,
            state: PairingConnectionState::Connecting,
            local_device,
            remote_device: None,
            is_initiator: false, // Client is joiner
        })
    }
    
    /// Accept a pairing connection as server
    pub async fn accept_connection(
        tcp_stream: TcpStream,
        local_device: DeviceInfo,
    ) -> Result<Self> {
        // Create ephemeral TLS configuration
        let tls_config = Self::create_ephemeral_server_config()?;
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));
        
        // Establish TLS connection
        let tls_stream = acceptor.accept(tcp_stream).await
            .map_err(|e| NetworkError::TransportError(format!("TLS accept failed: {}", e)))?;
        
        let stream = Box::new(TlsServerStream { stream: tls_stream }) as Box<dyn PairingStream>;
        
        Ok(Self {
            stream,
            state: PairingConnectionState::Connecting,
            local_device,
            remote_device: None,
            is_initiator: true, // Server is initiator
        })
    }
    
    /// Create ephemeral TLS client configuration
    fn create_ephemeral_client_config() -> Result<ClientConfig> {
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PairingCertVerifier))
            .with_no_client_auth();
        
        Ok(config)
    }
    
    /// Create ephemeral TLS server configuration
    fn create_ephemeral_server_config() -> Result<ServerConfig> {
        // Create self-signed certificate with default key generation
        let cert_params = CertificateParams::new(vec!["spacedrive-pairing".to_string()]);
        
        let cert = RcgenCertificate::from_params(cert_params)
            .map_err(|e| NetworkError::EncryptionError(format!("Certificate generation failed: {:?}", e)))?;
        
        // Convert to rustls format
        let cert_der = cert.serialize_der()
            .map_err(|e| NetworkError::EncryptionError(format!("Certificate serialization failed: {:?}", e)))?;
        let private_key_der = cert.serialize_private_key_der();
        
        let certs = vec![CertificateDer::from(cert_der)];
        let private_key = rustls::pki_types::PrivatePkcs8KeyDer::from(private_key_der).into();
        
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, private_key)
            .map_err(|e| NetworkError::EncryptionError(format!("TLS config creation failed: {:?}", e)))?;
        
        Ok(config)
    }
    
    /// Get current connection state
    pub fn state(&self) -> &PairingConnectionState {
        &self.state
    }
    
    /// Get local device info
    pub fn local_device(&self) -> &DeviceInfo {
        &self.local_device
    }
    
    /// Get remote device info (if available)
    pub fn remote_device(&self) -> Option<&DeviceInfo> {
        self.remote_device.as_ref()
    }
    
    /// Check if this is the initiator
    pub fn is_initiator(&self) -> bool {
        self.is_initiator
    }
    
    /// Send message over secure connection
    pub async fn send_message(&mut self, message: &[u8]) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        
        // Send length prefix
        let len = message.len() as u32;
        self.stream.write_all(&len.to_be_bytes()).await
            .map_err(|e| NetworkError::TransportError(format!("Failed to send length: {}", e)))?;
        
        // Send message
        self.stream.write_all(message).await
            .map_err(|e| NetworkError::TransportError(format!("Failed to send message: {}", e)))?;
        
        self.stream.flush().await
            .map_err(|e| NetworkError::TransportError(format!("Failed to flush: {}", e)))?;
        
        Ok(())
    }
    
    /// Receive message from secure connection
    pub async fn receive_message(&mut self) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;
        
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes).await
            .map_err(|e| NetworkError::TransportError(format!("Failed to read length: {}", e)))?;
        
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        // Sanity check message length
        if len > 1024 * 1024 {
            return Err(NetworkError::ProtocolError("Message too large".to_string()));
        }
        
        // Read message
        let mut buffer = vec![0u8; len];
        self.stream.read_exact(&mut buffer).await
            .map_err(|e| NetworkError::TransportError(format!("Failed to read message: {}", e)))?;
        
        Ok(buffer)
    }
    
    /// Get peer address
    pub fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream.peer_addr()
    }
    
    /// Update connection state
    pub fn set_state(&mut self, state: PairingConnectionState) {
        self.state = state;
    }
    
    /// Set remote device info
    pub fn set_remote_device(&mut self, device_info: DeviceInfo) {
        self.remote_device = Some(device_info);
    }
}

/// Certificate verifier for pairing (allows self-signed certificates)
#[derive(Debug)]
struct PairingCertVerifier;

impl rustls::client::danger::ServerCertVerifier for PairingCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // For pairing, we accept any certificate since we'll verify via challenge-response
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA1,
            rustls::SignatureScheme::ECDSA_SHA1_Legacy,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}

/// Pairing server for accepting incoming connections
pub struct PairingServer {
    listener: TcpListener,
    local_device: DeviceInfo,
}

impl PairingServer {
    /// Create a new pairing server
    pub async fn bind(addr: SocketAddr, local_device: DeviceInfo) -> Result<Self> {
        let listener = TcpListener::bind(addr).await
            .map_err(|e| NetworkError::TransportError(format!("Failed to bind listener: {}", e)))?;
        
        Ok(Self {
            listener,
            local_device,
        })
    }
    
    /// Accept a pairing connection
    pub async fn accept(&self) -> Result<PairingConnection> {
        let (tcp_stream, _addr) = self.listener.accept().await
            .map_err(|e| NetworkError::TransportError(format!("Failed to accept connection: {}", e)))?;
        
        PairingConnection::accept_connection(tcp_stream, self.local_device.clone()).await
    }
    
    /// Get the local address
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr()
            .map_err(|e| NetworkError::TransportError(format!("Failed to get local addr: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::identity::PublicKey;
    use std::net::{IpAddr, Ipv4Addr};
    use uuid::Uuid;

    fn create_test_device_info() -> DeviceInfo {
        DeviceInfo::new(
            Uuid::new_v4(),
            "Test Device".to_string(),
            PublicKey::from_bytes(vec![0u8; 32]).unwrap(),
        )
    }

    #[tokio::test]
    async fn test_tls_config_creation() {
        let client_config = PairingConnection::create_ephemeral_client_config();
        assert!(client_config.is_ok());
        
        let server_config = PairingConnection::create_ephemeral_server_config();
        assert!(server_config.is_ok());
    }

    #[tokio::test]
    async fn test_pairing_server_creation() {
        let device_info = create_test_device_info();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        
        let server = PairingServer::bind(addr, device_info).await;
        assert!(server.is_ok());
        
        let server = server.unwrap();
        let local_addr = server.local_addr().unwrap();
        assert!(local_addr.port() > 0);
    }
}