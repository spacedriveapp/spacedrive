//! Transport layer implementations for networking

use async_trait::async_trait;
use std::sync::Arc;

use crate::networking::{
    connection::NetworkConnection,
    identity::{NetworkFingerprint, NetworkIdentity},
    Result,
};
use uuid::Uuid;

// pub mod local;  // Disabled for now due to missing dependencies
// pub mod relay;  // Disabled for now due to missing dependencies

// pub use local::LocalTransport;
// pub use relay::RelayTransport;

/// Abstract transport interface
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to a device using this transport
    async fn connect(
        &self,
        device_id: Uuid,
        identity: &NetworkIdentity,
    ) -> Result<Box<dyn NetworkConnection>>;

    /// Start listening for incoming connections
    async fn listen(&self, identity: &NetworkIdentity) -> Result<()>;

    /// Stop listening for connections
    async fn stop_listening(&self) -> Result<()>;

    /// Get transport type identifier
    fn transport_type(&self) -> &'static str;

    /// Check if transport is available
    async fn is_available(&self) -> bool;
}