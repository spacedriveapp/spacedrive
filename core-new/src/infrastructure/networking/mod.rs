//! Networking module for Spacedrive
//!
//! Provides production-ready networking with libp2p:
//! - Global DHT-based discovery via Kademlia
//! - Multi-transport support (TCP + QUIC)
//! - NAT traversal and hole punching
//! - Noise Protocol encryption
//! - Efficient device pairing and authentication
//! - Request-response messaging over libp2p
//! - Persistent device connections with auto-reconnection
//! - Protocol-agnostic message system for all device communication

pub mod identity;
pub mod manager;
pub mod pairing;

// LibP2P components
pub mod behavior;
pub mod codec;
pub mod discovery;

// Persistent connections system
pub mod persistent;

pub use identity::{NetworkIdentity, NetworkFingerprint, MasterKey, DeviceInfo, PublicKey, PrivateKey, Signature};
pub use pairing::{
    PairingCode, PairingState, PairingUserInterface, ConsolePairingUI, SessionKeys
};

// LibP2P exports
pub use behavior::SpacedriveBehaviour;
pub use codec::PairingCodec;
pub use discovery::LibP2PDiscovery;
pub use pairing::protocol::LibP2PPairingProtocol;

// Persistent connections exports
pub use persistent::{
    NetworkingService, PersistentConnectionManager, PersistentNetworkIdentity,
    DeviceMessage, ConnectionState, TrustLevel, ProtocolHandler,
    init_persistent_networking, handle_successful_pairing,
};

// LibP2P events and channels
use libp2p::{Multiaddr, PeerId};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum LibP2PEvent {
    DeviceDiscovered { peer_id: PeerId, addr: Multiaddr },
    PairingRequest { peer_id: PeerId, message: pairing::PairingMessage },
    PairingResponse { peer_id: PeerId, message: pairing::PairingMessage },
    ConnectionEstablished { peer_id: PeerId },
    ConnectionClosed { peer_id: PeerId },
    Error { peer_id: Option<PeerId>, error: String },
}

pub type EventSender = mpsc::UnboundedSender<LibP2PEvent>;
pub type EventReceiver = mpsc::UnboundedReceiver<LibP2PEvent>;

pub fn create_event_channel() -> (EventSender, EventReceiver) {
    mpsc::unbounded_channel()
}

use thiserror::Error;

#[derive(Error, Debug, Clone)]
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
    IoError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Connection timeout")]
    ConnectionTimeout,
    
    #[error("Not initialized: {0}")]
    NotInitialized(String),
    
    #[error("Pairing failed: {0}")]
    PairingFailed(String),
    
    #[error("Pairing cancelled")]
    PairingCancelled,
}

pub type Result<T> = std::result::Result<T, NetworkError>;