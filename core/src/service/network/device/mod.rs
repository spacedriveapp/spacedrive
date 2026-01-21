//! Device registry and connection management

pub mod connection;
pub mod persistence;
pub mod registry;

use chrono::{DateTime, Utc};
use iroh::{NodeAddr, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Note: The connection module has a more complex DeviceConnection for active connections
// This simpler one is used in DeviceState for tracking connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
	pub latency_ms: Option<u32>,
	pub rx_bytes: u64,
	pub tx_bytes: u64,
}
pub use persistence::{DevicePersistence, PairingType, PersistedPairedDevice, TrustLevel};
pub use registry::DeviceRegistry;

/// Information about a device on the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
	pub device_id: Uuid,
	pub device_name: String,
	pub device_slug: String,
	pub device_type: DeviceType,
	pub os_version: String,
	pub app_version: String,
	pub network_fingerprint: crate::service::network::utils::identity::NetworkFingerprint,
	pub last_seen: DateTime<Utc>,
}

/// Type of device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
	Desktop,
	Laptop,
	Mobile,
	Server,
	Other(String),
}

impl Default for DeviceType {
	fn default() -> Self {
		Self::Desktop
	}
}

/// State of a device in the registry
#[derive(Debug, Clone)]
pub enum DeviceState {
	/// Device discovered via Iroh discovery but not yet connected
	Discovered {
		node_id: NodeId,
		node_addr: NodeAddr,
		discovered_at: DateTime<Utc>,
	},
	/// Device currently in pairing process
	Pairing {
		node_id: NodeId,
		session_id: Uuid,
		node_addr: NodeAddr,
		started_at: DateTime<Utc>,
	},
	/// Device successfully paired but not currently connected
	Paired {
		info: DeviceInfo,
		session_keys: SessionKeys,
		paired_at: DateTime<Utc>,
	},
	/// Device currently connected and active
	Connected {
		info: DeviceInfo,
		connection: ConnectionInfo,
		session_keys: SessionKeys,
		connected_at: DateTime<Utc>,
	},
	/// Device was connected but is now disconnected
	Disconnected {
		info: DeviceInfo,
		session_keys: SessionKeys,
		last_seen: DateTime<Utc>,
		reason: DisconnectionReason,
	},
}

/// Reason for disconnection
#[derive(Debug, Clone)]
pub enum DisconnectionReason {
	UserInitiated,
	NetworkError(String),
	Timeout,
	AuthenticationFailed,
	ProtocolError(String),
	ConnectionLost,
}

/// Session keys for encrypted communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionKeys {
	pub shared_secret: Vec<u8>,
	pub send_key: Vec<u8>,
	pub receive_key: Vec<u8>,
	pub created_at: DateTime<Utc>,
	pub expires_at: Option<DateTime<Utc>>,
}

impl SessionKeys {
	/// Generate new session keys from a shared secret
	/// This should be called by the initiator. The joiner should call this and then swap_keys().
	pub fn from_shared_secret(shared_secret: Vec<u8>) -> Self {
		// Use HKDF to derive send/receive keys from shared secret
		use hkdf::Hkdf;
		use sha2::Sha256;

		// Derive send key
		let hk_send = Hkdf::<Sha256>::new(None, &shared_secret);
		let mut send_key = [0u8; 32];
		hk_send
			.expand(b"spacedrive-send-key", &mut send_key)
			.unwrap();

		// Derive receive key with fresh HKDF instance
		let hk_recv = Hkdf::<Sha256>::new(None, &shared_secret);
		let mut receive_key = [0u8; 32];
		hk_recv
			.expand(b"spacedrive-receive-key", &mut receive_key)
			.unwrap();

		Self {
			shared_secret,
			send_key: send_key.to_vec(),
			receive_key: receive_key.to_vec(),
			created_at: Utc::now(),
			expires_at: Some(Utc::now() + chrono::Duration::hours(24)), // 24 hour expiry
		}
	}

	/// Swap send and receive keys
	/// This should be called by the joiner so that initiator's send_key = joiner's receive_key
	pub fn swap_keys(mut self) -> Self {
		std::mem::swap(&mut self.send_key, &mut self.receive_key);
		self
	}

	/// Check if keys are expired
	pub fn is_expired(&self) -> bool {
		if let Some(expires_at) = self.expires_at {
			Utc::now() > expires_at
		} else {
			false
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_session_keys_are_different() {
		// Create a test shared secret
		let shared_secret = vec![1u8; 32];

		// Generate session keys
		let keys = SessionKeys::from_shared_secret(shared_secret);

		// Verify send_key and receive_key are DIFFERENT
		assert_ne!(
			keys.send_key,
			keys.receive_key,
			"BUG: send_key and receive_key should be different! send={:?}, recv={:?}",
			&keys.send_key[..8],
			&keys.receive_key[..8]
		);
	}

	#[test]
	fn test_swap_keys_works() {
		let shared_secret = vec![1u8; 32];
		let keys = SessionKeys::from_shared_secret(shared_secret);

		let original_send = keys.send_key.clone();
		let original_recv = keys.receive_key.clone();

		let swapped = keys.swap_keys();

		// After swap, send should equal original receive
		assert_eq!(swapped.send_key, original_recv);
		// After swap, receive should equal original send
		assert_eq!(swapped.receive_key, original_send);
	}
}
