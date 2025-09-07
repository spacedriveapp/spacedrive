//! Device registry and connection management

pub mod connection;
pub mod persistence;
pub mod registry;

use chrono::{DateTime, Utc};
use iroh::net::NodeAddr;
use iroh::net::key::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Note: The connection module has a more complex DeviceConnection for active connections
// This simpler one is used in DeviceState
#[derive(Debug, Clone)]
pub struct DeviceConnection {
	pub addresses: Vec<String>,  // Node addresses as strings
	pub latency_ms: Option<u32>,
	pub rx_bytes: u64,
	pub tx_bytes: u64,
}
pub use persistence::{DevicePersistence, PersistedPairedDevice, TrustLevel};
pub use registry::DeviceRegistry;

/// Information about a device on the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
	pub device_id: Uuid,
	pub device_name: String,
	pub device_type: DeviceType,
	pub os_version: String,
	pub app_version: String,
	pub network_fingerprint: crate::service::networking::utils::identity::NetworkFingerprint,
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
		connection: DeviceConnection,
		session_keys: SessionKeys,
		connected_at: DateTime<Utc>,
	},
	/// Device was connected but is now disconnected
	Disconnected {
		info: DeviceInfo,
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
	pub fn from_shared_secret(shared_secret: Vec<u8>) -> Self {
		// Use HKDF to derive send/receive keys from shared secret
		use hkdf::Hkdf;
		use sha2::Sha256;

		let hk = Hkdf::<Sha256>::new(None, &shared_secret);
		let mut send_key = [0u8; 32];
		let mut receive_key = [0u8; 32];

		// Use the same salt for both keys to ensure initiator's send key
		// matches joiner's receive key, enabling successful decryption
		hk.expand(b"spacedrive-symmetric-key", &mut send_key).unwrap();
		hk.expand(b"spacedrive-symmetric-key", &mut receive_key).unwrap();

		Self {
			shared_secret,
			send_key: send_key.to_vec(),
			receive_key: receive_key.to_vec(),
			created_at: Utc::now(),
			expires_at: Some(Utc::now() + chrono::Duration::hours(24)), // 24 hour expiry
		}
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
