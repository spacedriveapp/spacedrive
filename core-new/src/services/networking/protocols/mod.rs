//! Protocol handling system for different message types

pub mod file_transfer;
pub mod messaging;
pub mod pairing;
pub mod registry;

use crate::services::networking::{NetworkingError, Result};
use async_trait::async_trait;
use iroh::net::key::NodeId;
use std::collections::HashMap;
use uuid::Uuid;

pub use file_transfer::{FileTransferMessage, FileTransferProtocolHandler, FileMetadata, TransferMode, TransferSession};
pub use messaging::MessagingProtocolHandler;
pub use pairing::{PairingMessage, PairingProtocolHandler, PairingSession, PairingState};
pub use registry::ProtocolRegistry;

/// Trait for handling specific protocols over Iroh streams
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
	/// Get the protocol name
	fn protocol_name(&self) -> &str;

	/// Handle an incoming stream (bidirectional or unidirectional)
	async fn handle_stream(
		&self,
		send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: NodeId,
	);

	/// Handle an incoming request (legacy compatibility)
	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>>;

	/// Handle an incoming response (legacy compatibility)
	async fn handle_response(&self, from_device: Uuid, from_node: NodeId, response_data: Vec<u8>) -> Result<()>;

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