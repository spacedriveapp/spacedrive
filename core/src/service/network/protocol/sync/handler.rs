//! Sync Protocol Handler (DEPRECATED - BEING REPLACED)
//!
//! This handler implemented the old leader-based sync protocol.
//! It is being replaced with the new leaderless hybrid protocol.
//!
//! Status: Stubbed out during migration to leaderless architecture

use super::messages::{StateRecord, SyncMessage};
use crate::service::network::{NetworkingError, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

/// Sync protocol handler (DEPRECATED)
///
/// This is a stub implementation during the migration to leaderless sync.
/// The new implementation will be in PeerSync service.
pub struct SyncProtocolHandler {
	library_id: Uuid,
}

impl SyncProtocolHandler {
	/// Create a new sync protocol handler (stub)
	pub fn new(library_id: Uuid) -> Self {
		warn!(
			library_id = %library_id,
			"Creating stubbed SyncProtocolHandler - leaderless protocol not yet implemented"
		);
		Self { library_id }
	}

	/// Get library ID
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}
}

#[async_trait]
impl crate::service::network::protocol::ProtocolHandler for SyncProtocolHandler {
	fn protocol_name(&self) -> &'static str {
		"sync"
	}

	async fn handle_stream(
		&self,
		_send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		_recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		_remote_node_id: iroh::NodeId,
	) {
		warn!("SyncProtocolHandler::handle_stream called but protocol not yet implemented");
	}

	async fn handle_request(&self, _from_device: Uuid, _request: Vec<u8>) -> Result<Vec<u8>> {
		warn!("SyncProtocolHandler::handle_request called but protocol not yet implemented");
		Err(NetworkingError::Protocol(
			"Sync protocol not yet implemented (leaderless migration in progress)".to_string(),
		))
	}

	async fn handle_response(
		&self,
		_from_device: Uuid,
		_from_node: iroh::NodeId,
		_response: Vec<u8>,
	) -> Result<()> {
		warn!("SyncProtocolHandler::handle_response called but protocol not yet implemented");
		Ok(())
	}

	async fn handle_event(
		&self,
		_event: crate::service::network::protocol::ProtocolEvent,
	) -> std::result::Result<(), crate::service::network::NetworkingError> {
		// No-op for now
		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_handler_creation() {
		let handler = SyncProtocolHandler::new(Uuid::new_v4());
		assert_eq!(handler.protocol_name(), "sync");
	}
}
