//! Persistent device connections module
//!
//! Provides always-on connections to paired devices with automatic reconnection,
//! encrypted storage, and comprehensive protocol support for all device-to-device
//! communication in Spacedrive.

pub mod connection;
pub mod identity;
pub mod manager;
pub mod messages;
pub mod pairing_bridge;
pub mod service;
pub mod storage;

// Re-export main types for easy access
pub use storage::{EncryptedData, SecureStorage};

pub use identity::{
	ActiveSession, ConnectionConfig, ConnectionRecord, ConnectionResult, EncryptedSessionKeys,
	PairedDeviceRecord, PersistentNetworkIdentity, RetryPolicy, SessionKeys, SessionState,
	TransportType, TrustLevel,
};

pub use messages::{
	CollabEvent, ConflictResolution, CrudOperation, DeviceMessage, FileMetadata, FsEvent,
	IndexingProgress, LibraryMetadata, LocationChange, NotificationAction, NotificationLevel,
	Permission, SearchQuery, SearchResult, SyncConflict, SyncEntry, SyncError, SyncOperation,
	SyncResult, UserInfo,
};

pub use connection::{
	ConnectionEvent, ConnectionMetrics, ConnectionState, DeviceConnection, MaintenanceAction,
	MessagePriority,
};

pub use manager::{
	ConnectionManagerConfig, NetworkEvent, PersistentConnectionManager, RetryInfo, RetryScheduler,
};

pub use service::{
	DatabaseSyncHandler, FileTransferHandler, NetworkingService, ProtocolHandler,
	RealtimeSyncHandler, SpacedropHandler,
};

pub use pairing_bridge::{
	PairingBridge, PairingRole, PairingSession, PairingStatus,
};

use crate::networking::Result;

/// Initialize persistent networking with default configuration
pub async fn init_persistent_networking(
	device_manager: std::sync::Arc<crate::device::DeviceManager>,
	password: &str,
) -> Result<NetworkingService> {
	NetworkingService::new(device_manager, password).await
}

/// Integration point with existing pairing system
pub async fn handle_successful_pairing(
	networking_service: &NetworkingService,
	device_info: crate::networking::DeviceInfo,
	session_keys: SessionKeys,
) -> Result<()> {
	networking_service
		.add_paired_device(device_info, session_keys)
		.await
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::device::DeviceManager;
	use tempfile::TempDir;
	use uuid::Uuid;

	async fn create_test_device_manager() -> (DeviceManager, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let device_manager = DeviceManager::init_with_path(&temp_dir.path().to_path_buf()).unwrap();
		(device_manager, temp_dir)
	}

	#[tokio::test]
	async fn test_persistent_identity_creation() {
		let (device_manager, _temp_dir) = create_test_device_manager().await;
		let password = "test-password-123";

		let identity = PersistentNetworkIdentity::load_or_create(&device_manager, password)
			.await
			.unwrap();

		assert_eq!(
			identity.identity.device_id,
			device_manager.device_id().unwrap()
		);
		assert!(identity.paired_devices.is_empty());
		assert!(identity.active_sessions.is_empty());
	}

	#[tokio::test]
	async fn test_session_keys_generation() {
		let keys = SessionKeys::new();
		assert_ne!(keys.send_key, [0u8; 32]);
		assert_ne!(keys.receive_key, [0u8; 32]);
		assert_ne!(keys.mac_key, [0u8; 32]);
		assert_ne!(keys.session_id, Uuid::nil());
	}

	#[tokio::test]
	async fn test_message_serialization() {
		let message = DeviceMessage::Keepalive;
		let serialized = serde_json::to_vec(&message).unwrap();
		let deserialized: DeviceMessage = serde_json::from_slice(&serialized).unwrap();

		match deserialized {
			DeviceMessage::Keepalive => {} // Success
			_ => panic!("Message deserialization failed"),
		}
	}

	#[tokio::test]
	async fn test_secure_storage() {
		use std::collections::HashMap;

		let temp_dir = TempDir::new().unwrap();
		let storage = SecureStorage::new(temp_dir.path().to_path_buf());
		let password = "test-password";

		// Test data
		let mut test_data = HashMap::new();
		test_data.insert("key1".to_string(), "value1".to_string());
		test_data.insert("key2".to_string(), "value2".to_string());

		// Store and load
		let test_path = temp_dir.path().join("test.json");
		storage
			.store(&test_path, &test_data, password)
			.await
			.unwrap();

		let loaded_data: Option<HashMap<String, String>> =
			storage.load(&test_path, password).await.unwrap();

		assert_eq!(Some(test_data), loaded_data);
	}

	#[tokio::test]
	async fn test_device_connection_encryption() {
		use crate::networking::{DeviceInfo, NetworkFingerprint, PublicKey};
		use chrono::Utc;

		// Create test device info
		let device_id = Uuid::new_v4();
		let public_key = PublicKey::from_bytes(vec![0u8; 32]).unwrap();
		let device_info = DeviceInfo {
			device_id,
			device_name: "Test Device".to_string(),
			public_key,
			network_fingerprint: NetworkFingerprint::from_device(
				device_id,
				&PublicKey::from_bytes(vec![0u8; 32]).unwrap(),
			),
			last_seen: Utc::now(),
		};

		let session_keys = SessionKeys::new();
		let connection = DeviceConnection::new(device_info, session_keys, None).unwrap();

		// Test message encryption/decryption
		let test_message = DeviceMessage::Keepalive;

		// This would test encryption/decryption if the methods were public
		// For now, just verify the connection was created successfully
		assert_eq!(connection.state(), &ConnectionState::Connecting);
	}
}
