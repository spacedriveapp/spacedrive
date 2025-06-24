//! Protocol handling system for different message types

pub mod file_transfer;
pub mod messaging;
pub mod pairing;
pub mod registry;

use crate::infrastructure::networking::{NetworkingError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

pub use file_transfer::{FileTransferMessage, FileTransferProtocolHandler, FileMetadata, TransferMode, TransferSession};
pub use messaging::MessagingProtocolHandler;
pub use pairing::{PairingMessage, PairingProtocolHandler, PairingSession, PairingState};
pub use registry::ProtocolRegistry;

/// Trait for handling specific protocols
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
	/// Get the protocol name
	fn protocol_name(&self) -> &str;

	/// Handle an incoming request
	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>>;

	/// Handle an incoming response (for request-response protocols)
	async fn handle_response(&self, from_device: Uuid, from_peer: libp2p::PeerId, response_data: Vec<u8>) -> Result<()>;

	/// Handle protocol-specific events
	async fn handle_event(&self, event: ProtocolEvent) -> Result<()>;

	/// Enable downcasting to concrete types
	fn as_any(&self) -> &dyn std::any::Any;
}

/// Events that protocols can receive
#[derive(Debug, Clone)]
pub enum ProtocolEvent {
	/// Device connected
	DeviceConnected { device_id: Uuid },

	/// Device disconnected
	DeviceDisconnected { device_id: Uuid },

	/// Connection failed
	ConnectionFailed { device_id: Uuid, reason: String },

	/// Custom protocol event
	Custom {
		protocol: String,
		data: HashMap<String, serde_json::Value>,
	},
}
