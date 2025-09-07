//! Spacedrive Networking v2 - Unified Architecture
//!
//! This is a complete redesign of the networking system that addresses the fundamental
//! issues in the original implementation:
//! - Single LibP2P swarm instead of multiple competing swarms
//! - Proper Send/Sync design for background task execution
//! - Centralized event system and state management
//! - Modular protocol handling
//!
//! Key components:
//! - `core`: Central networking engine with unified LibP2P swarm
//! - `protocols`: Modular protocol handlers (pairing, messaging, file transfer)
//! - `device`: Device registry and connection management
//! - `utils`: Shared utilities (identity, codecs, logging)

pub mod core;
pub mod protocols;
pub mod device;
pub mod utils;

// Re-export main types for easy access
pub use core::{NetworkingService, NetworkEvent};

// Compatibility alias for legacy code
pub use NetworkingService as NetworkingCore;
pub use device::{DeviceInfo, DeviceRegistry, DeviceState};
pub use protocols::{ProtocolHandler, ProtocolRegistry};
pub use utils::{NetworkIdentity, NetworkLogger, SilentLogger};

// Re-export specific protocol types for CLI compatibility
pub use protocols::pairing::{PairingState, PairingSession};

/// Main error type for networking operations
#[derive(Debug, thiserror::Error)]
pub enum NetworkingError {
    #[error("LibP2P error: {0}")]
    LibP2P(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Device not found: {0}")]
    DeviceNotFound(uuid::Uuid),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Transport error: {0}")]
    Transport(String),
}

pub type Result<T> = std::result::Result<T, NetworkingError>;

/// Initialize the new networking system
pub async fn init_networking(
    device_manager: std::sync::Arc<crate::device::DeviceManager>,
    library_key_manager: std::sync::Arc<crate::keys::library_key_manager::LibraryKeyManager>,
    data_dir: impl AsRef<std::path::Path>,
) -> Result<NetworkingService> {
    let logger = std::sync::Arc::new(utils::logging::ConsoleLogger);
    NetworkingService::new(device_manager, library_key_manager, data_dir, logger).await
}