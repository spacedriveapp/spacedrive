//! Basic messaging protocol handler

use super::{ProtocolEvent, ProtocolHandler};
use crate::services::networking::{NetworkingError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Basic messaging protocol handler
pub struct MessagingProtocolHandler;

/// Basic message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
	/// Ping message for connection testing
	Ping {
		timestamp: chrono::DateTime<chrono::Utc>,
		payload: Option<Vec<u8>>,
	},
	/// Pong response
	Pong {
		timestamp: chrono::DateTime<chrono::Utc>,
		original_timestamp: chrono::DateTime<chrono::Utc>,
	},
	/// Generic data message
	Data {
		message_id: Uuid,
		content_type: String,
		payload: Vec<u8>,
	},
	/// Acknowledgment message
	Ack {
		message_id: Uuid,
		success: bool,
		error: Option<String>,
	},
}

impl MessagingProtocolHandler {
	/// Create a new messaging protocol handler
	pub fn new() -> Self {
		Self
	}

	async fn handle_ping(
		&self,
		_from_device: Uuid,
		timestamp: chrono::DateTime<chrono::Utc>,
		_payload: Option<Vec<u8>>,
	) -> Result<Vec<u8>> {
		let response = Message::Pong {
			timestamp: chrono::Utc::now(),
			original_timestamp: timestamp,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_pong(
		&self,
		_from_device: Uuid,
		_timestamp: chrono::DateTime<chrono::Utc>,
		original_timestamp: chrono::DateTime<chrono::Utc>,
	) -> Result<Vec<u8>> {
		let now = chrono::Utc::now();
		let rtt = now.signed_duration_since(original_timestamp);

		println!("Ping RTT: {}ms", rtt.num_milliseconds());

		// Return empty response for pong
		Ok(Vec::new())
	}

	async fn handle_data(
		&self,
		from_device: Uuid,
		message_id: Uuid,
		content_type: String,
		payload: Vec<u8>,
	) -> Result<Vec<u8>> {
		// Process the data message
		println!(
			"Received data message from {}: {} ({} bytes)",
			from_device,
			content_type,
			payload.len()
		);

		// Send acknowledgment
		let response = Message::Ack {
			message_id,
			success: true,
			error: None,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_ack(
		&self,
		_from_device: Uuid,
		message_id: Uuid,
		success: bool,
		error: Option<String>,
	) -> Result<Vec<u8>> {
		if success {
			println!("Message {} acknowledged successfully", message_id);
		} else {
			println!("Message {} failed: {:?}", message_id, error);
		}

		// Return empty response for ack
		Ok(Vec::new())
	}
}

impl Default for MessagingProtocolHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl ProtocolHandler for MessagingProtocolHandler {
	fn protocol_name(&self) -> &str {
		"messaging"
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		let message: Message =
			serde_json::from_slice(&request_data).map_err(|e| NetworkingError::Serialization(e))?;

		match message {
			Message::Ping { timestamp, payload } => {
				self.handle_ping(from_device, timestamp, payload).await
			}
			Message::Pong {
				timestamp,
				original_timestamp,
			} => {
				self.handle_pong(from_device, timestamp, original_timestamp)
					.await
			}
			Message::Data {
				message_id,
				content_type,
				payload,
			} => {
				self.handle_data(from_device, message_id, content_type, payload)
					.await
			}
			Message::Ack {
				message_id,
				success,
				error,
			} => {
				self.handle_ack(from_device, message_id, success, error)
					.await
			}
		}
	}

	async fn handle_response(&self, _from_device: Uuid, _from_peer: libp2p::PeerId, _response_data: Vec<u8>) -> Result<()> {
		// Messaging protocol handles responses in handle_request
		Ok(())
	}

	async fn handle_event(&self, _event: ProtocolEvent) -> Result<()> {
		// Basic messaging doesn't need special event handling
		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}
