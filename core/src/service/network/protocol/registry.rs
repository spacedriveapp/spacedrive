//! Protocol registry for managing protocol handlers

use super::{ProtocolEvent, ProtocolHandler};
use iroh::NodeId;
use crate::service::network::{NetworkingError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Registry for protocol handlers
pub struct ProtocolRegistry {
	handlers: HashMap<String, Arc<dyn ProtocolHandler>>,
}

impl ProtocolRegistry {
	/// Create a new protocol registry
	pub fn new() -> Self {
		Self {
			handlers: HashMap::new(),
		}
	}

	/// Register a protocol handler
	pub fn register_handler(&mut self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
		let protocol_name = handler.protocol_name().to_string();

		if self.handlers.contains_key(&protocol_name) {
			return Err(NetworkingError::Protocol(format!(
				"Protocol {} already registered",
				protocol_name
			)));
		}

		self.handlers.insert(protocol_name, handler);
		Ok(())
	}

	/// Unregister a protocol handler
	pub fn unregister_handler(&mut self, protocol_name: &str) -> Result<()> {
		self.handlers.remove(protocol_name).ok_or_else(|| {
			NetworkingError::Protocol(format!("Protocol {} not found", protocol_name))
		})?;

		Ok(())
	}

	/// Get a protocol handler by name
	pub fn get_handler(&self, protocol_name: &str) -> Option<Arc<dyn ProtocolHandler>> {
		self.handlers.get(protocol_name).cloned()
	}

	/// Handle an incoming request
	pub async fn handle_request(
		&self,
		protocol_name: &str,
		from_device: Uuid,
		request_data: Vec<u8>,
	) -> Result<Vec<u8>> {
		let handler = self.get_handler(protocol_name).ok_or_else(|| {
			NetworkingError::Protocol(format!("No handler for protocol {}", protocol_name))
		})?;

		handler.handle_request(from_device, request_data).await
	}

	/// Handle an incoming response
	pub async fn handle_response(
		&self,
		protocol_name: &str,
		from_device: Uuid,
		from_node: NodeId,
		response_data: Vec<u8>,
	) -> Result<()> {
		let handler = self.get_handler(protocol_name).ok_or_else(|| {
			NetworkingError::Protocol(format!("No handler for protocol {}", protocol_name))
		})?;

		handler.handle_response(from_device, from_node, response_data).await
	}

	/// Broadcast an event to all protocol handlers
	pub async fn broadcast_event(&self, event: ProtocolEvent) -> Result<()> {
		for handler in self.handlers.values() {
			if let Err(e) = handler.handle_event(event.clone()).await {
				// Log error but continue with other handlers
				eprintln!(
					"Protocol {} error handling event: {}",
					handler.protocol_name(),
					e
				);
			}
		}

		Ok(())
	}

	/// Get list of registered protocol names
	pub fn get_protocol_names(&self) -> Vec<String> {
		self.handlers.keys().cloned().collect()
	}

	/// Get the number of registered handlers
	pub fn handler_count(&self) -> usize {
		self.handlers.len()
	}
}

impl Default for ProtocolRegistry {
	fn default() -> Self {
		Self::new()
	}
}
