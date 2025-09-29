//! Individual device connection handling

use super::{DeviceInfo, SessionKeys};
use crate::service::network::{NetworkingError, Result};
use chrono::{DateTime, Utc};
use iroh::NodeId;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Represents an active connection to a remote device
#[derive(Debug, Clone)]
pub struct DeviceConnection {
	/// The node ID of the remote device
	pub node_id: NodeId,

	/// Device information
	pub device_info: DeviceInfo,

	/// Session keys for encryption
	pub session_keys: SessionKeys,

	/// Connection statistics
	pub stats: ConnectionStats,

	/// Channel for sending messages to this device
	pub message_sender: mpsc::UnboundedSender<OutgoingMessage>,
}

/// Statistics about a connection
#[derive(Debug, Clone)]
pub struct ConnectionStats {
	pub connected_at: DateTime<Utc>,
	pub bytes_sent: u64,
	pub bytes_received: u64,
	pub messages_sent: u64,
	pub messages_received: u64,
	pub last_activity: DateTime<Utc>,
}

impl Default for ConnectionStats {
	fn default() -> Self {
		let now = Utc::now();
		Self {
			connected_at: now,
			bytes_sent: 0,
			bytes_received: 0,
			messages_sent: 0,
			messages_received: 0,
			last_activity: now,
		}
	}
}

/// Message to be sent to a remote device
#[derive(Debug)]
pub struct OutgoingMessage {
	pub protocol: String,
	pub data: Vec<u8>,
	pub response_channel: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
}

impl DeviceConnection {
	/// Create a new device connection
	pub fn new(
		node_id: NodeId,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
	) -> (Self, mpsc::UnboundedReceiver<OutgoingMessage>) {
		let (message_sender, message_receiver) = mpsc::unbounded_channel();

		let connection = Self {
			node_id,
			device_info,
			session_keys,
			stats: ConnectionStats::default(),
			message_sender,
		};

		(connection, message_receiver)
	}

	/// Send a message to this device
	pub async fn send_message(&self, protocol: &str, data: Vec<u8>) -> Result<()> {
		let message = OutgoingMessage {
			protocol: protocol.to_string(),
			data,
			response_channel: None,
		};

		self.message_sender
			.send(message)
			.map_err(|_| NetworkingError::ConnectionFailed("Connection closed".to_string()))?;

		Ok(())
	}

	/// Send a message and wait for a response
	pub async fn send_request(&self, protocol: &str, data: Vec<u8>) -> Result<Vec<u8>> {
		let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

		let message = OutgoingMessage {
			protocol: protocol.to_string(),
			data,
			response_channel: Some(response_sender),
		};

		self.message_sender
			.send(message)
			.map_err(|_| NetworkingError::ConnectionFailed("Connection closed".to_string()))?;

		response_receiver
			.await
			.map_err(|_| NetworkingError::Timeout("Request timeout".to_string()))?
	}

	/// Check if the connection is still valid
	pub fn is_valid(&self) -> bool {
		!self.message_sender.is_closed() && !self.session_keys.is_expired()
	}

	/// Update connection statistics
	pub fn update_stats(&mut self, bytes_sent: u64, bytes_received: u64) {
		self.stats.bytes_sent += bytes_sent;
		self.stats.bytes_received += bytes_received;
		self.stats.last_activity = Utc::now();
	}

	/// Encrypt data using session keys
	pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
		// Simple XOR encryption for now - in production use proper AEAD
		let key = &self.session_keys.send_key;
		let mut encrypted = Vec::with_capacity(data.len());

		for (i, byte) in data.iter().enumerate() {
			encrypted.push(byte ^ key[i % key.len()]);
		}

		Ok(encrypted)
	}

	/// Decrypt data using session keys
	pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
		// Simple XOR decryption for now - in production use proper AEAD
		let key = &self.session_keys.receive_key;
		let mut decrypted = Vec::with_capacity(data.len());

		for (i, byte) in data.iter().enumerate() {
			decrypted.push(byte ^ key[i % key.len()]);
		}

		Ok(decrypted)
	}
}
