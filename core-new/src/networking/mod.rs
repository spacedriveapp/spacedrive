//! Networking module for Spacedrive
//!
//! Provides secure, transport-agnostic networking with support for:
//! - Local P2P connections via mDNS + QUIC
//! - Internet connectivity through relay servers
//! - End-to-end encryption using Noise Protocol
//! - Efficient file transfer
//! - Device pairing and authentication

pub mod identity;
pub mod connection;
pub mod transport;
pub mod security;
pub mod protocol;
pub mod manager;
pub mod pairing;

pub use manager::Network;
pub use identity::{NetworkIdentity, NetworkFingerprint, MasterKey, DeviceInfo, PublicKey, PrivateKey, Signature};
pub use connection::{NetworkConnection, DeviceConnection, Transport};
pub use pairing::{
    PairingCode, PairingManager, PairingSession, PairingState,
    PairingDiscovery, PairingConnection, PairingProtocolHandler,
    PairingUserInterface, ConsolePairingUI, SessionKeys
};
// pub use transport::{Transport, LocalTransport, RelayTransport}; // Disabled for now
pub use security::NoiseSession;
pub use protocol::{FileHeader, FileTransfer};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Device not found: {0}")]
    DeviceNotFound(uuid::Uuid),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Transport error: {0}")]
    TransportError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Connection timeout")]
    ConnectionTimeout,
}

pub type Result<T> = std::result::Result<T, NetworkError>;