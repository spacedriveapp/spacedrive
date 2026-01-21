//! Persistence for paired devices and their connection info

use super::{DeviceInfo, SessionKeys};
use crate::crypto::key_manager::KeyManager;
use crate::service::network::{NetworkingError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Pairing type for a device relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingType {
	Direct,
	Proxied,
}

impl Default for PairingType {
	fn default() -> Self {
		Self::Direct
	}
}

/// Persisted paired device data (plain data structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPairedDevice {
	pub device_info: DeviceInfo,
	pub session_keys: SessionKeys,
	pub paired_at: DateTime<Utc>,
	pub last_connected_at: Option<DateTime<Utc>>,
	pub connection_attempts: u32,
	pub trust_level: TrustLevel,
	/// Cached relay URL for reconnection optimization (discovered via pkarr or connection)
	#[serde(default)]
	pub relay_url: Option<String>,
	#[serde(default)]
	pub pairing_type: PairingType,
	#[serde(default)]
	pub vouched_by: Option<Uuid>,
	#[serde(default)]
	pub vouched_at: Option<DateTime<Utc>>,
}

/// Trust level for persistent connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrustLevel {
	/// Device was manually paired and should auto-reconnect
	Trusted,
	/// Device connection failed multiple times, deprioritize
	Unreliable,
	/// Device manually disconnected, don't auto-reconnect
	Blocked,
}

impl Default for TrustLevel {
	fn default() -> Self {
		Self::Trusted
	}
}

/// Device persistence manager
#[derive(Clone)]
pub struct DevicePersistence {
	key_manager: Arc<KeyManager>,
}

impl DevicePersistence {
	/// Create a new device persistence manager
	pub fn new(key_manager: Arc<KeyManager>) -> Self {
		Self { key_manager }
	}

	/// Generate key for a paired device
	fn device_key(device_id: Uuid) -> String {
		format!("paired_device_{}", device_id)
	}

	/// Key for the list of all paired device IDs
	const DEVICE_LIST_KEY: &'static str = "paired_devices_list";

	/// Get list of paired device IDs
	async fn get_device_list(&self) -> Result<Vec<Uuid>> {
		match self.key_manager.get_secret(Self::DEVICE_LIST_KEY).await {
			Ok(data) => {
				let device_ids: Vec<Uuid> =
					serde_json::from_slice(&data).map_err(|e| NetworkingError::Serialization(e))?;
				Ok(device_ids)
			}
			Err(_) => Ok(Vec::new()),
		}
	}

	/// Save list of paired device IDs
	async fn save_device_list(&self, device_ids: &[Uuid]) -> Result<()> {
		let data = serde_json::to_vec(device_ids).map_err(|e| NetworkingError::Serialization(e))?;
		self.key_manager
			.set_secret(Self::DEVICE_LIST_KEY, &data)
			.await
			.map_err(|e| NetworkingError::Protocol(format!("Failed to save device list: {}", e)))?;
		Ok(())
	}

	/// Save paired devices to key manager (encrypted)
	pub async fn save_paired_devices(
		&self,
		devices: &HashMap<Uuid, PersistedPairedDevice>,
	) -> Result<()> {
		let device_ids: Vec<Uuid> = devices.keys().copied().collect();
		self.save_device_list(&device_ids).await?;

		for (device_id, device) in devices {
			let key = Self::device_key(*device_id);
			let data = serde_json::to_vec(device).map_err(|e| NetworkingError::Serialization(e))?;
			self.key_manager
				.set_secret(&key, &data)
				.await
				.map_err(|e| {
					NetworkingError::Protocol(format!("Failed to save device {}: {}", device_id, e))
				})?;
		}

		println!("Saved {} paired devices (encrypted)", devices.len());
		Ok(())
	}

	/// Load paired devices from key manager (decrypt)
	pub async fn load_paired_devices(&self) -> Result<HashMap<Uuid, PersistedPairedDevice>> {
		let device_ids = self.get_device_list().await?;
		tracing::debug!("Loading {} device IDs from persistence", device_ids.len());
		let mut devices = HashMap::new();

		for device_id in device_ids {
			let key = Self::device_key(device_id);
			match self.key_manager.get_secret(&key).await {
				Ok(data) => match serde_json::from_slice::<PersistedPairedDevice>(&data) {
					Ok(device) => {
						if !device.session_keys.is_expired() {
							// Validate that send_key and receive_key are different
							if device.session_keys.send_key == device.session_keys.receive_key {
								tracing::error!(
									"Device {} has IDENTICAL send_key and receive_key - corrupted pairing! Re-pair this device.",
									device_id
								);
								// Skip loading this device - it's unusable
								continue;
							}

							tracing::debug!(
								"Loaded paired device: {} ({})",
								device.device_info.device_name,
								device_id
							);
							devices.insert(device_id, device);
						} else {
							tracing::warn!(
								"Device {} has expired session keys, skipping",
								device_id
							);
						}
					}
					Err(e) => {
						tracing::error!("Failed to deserialize device {}: {}", device_id, e);
					}
				},
				Err(e) => {
					tracing::error!("Failed to load device {}: {}", device_id, e);
				}
			}
		}

		tracing::debug!(
			"Successfully loaded {} paired devices from persistence",
			devices.len()
		);
		Ok(devices)
	}

	/// Add a newly paired device
	pub async fn add_paired_device(
		&self,
		device_id: Uuid,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
		relay_url: Option<String>,
		pairing_type: PairingType,
		vouched_by: Option<Uuid>,
		vouched_at: Option<DateTime<Utc>>,
	) -> Result<()> {
		let mut devices = self.load_paired_devices().await?;

		let paired_device = PersistedPairedDevice {
			device_info,
			session_keys,
			paired_at: Utc::now(),
			last_connected_at: None,
			connection_attempts: 0,
			trust_level: TrustLevel::Trusted,
			relay_url,
			pairing_type,
			vouched_by,
			vouched_at,
		};

		devices.insert(device_id, paired_device);
		self.save_paired_devices(&devices).await?;

		Ok(())
	}

	/// Update connection info for a device
	pub async fn update_device_connection(
		&self,
		device_id: Uuid,
		connected: bool,
		addresses: Option<Vec<String>>,
	) -> Result<()> {
		let mut devices = self.load_paired_devices().await?;

		if let Some(device) = devices.get_mut(&device_id) {
			if connected {
				device.last_connected_at = Some(Utc::now());
				device.connection_attempts = 0; // Reset on successful connection

				// Restore trust to Trusted on successful connection
				if matches!(device.trust_level, TrustLevel::Unreliable) {
					device.trust_level = TrustLevel::Trusted;
				}
			} else {
				device.connection_attempts += 1;

				// Mark as unreliable after 5 failed attempts
				if device.connection_attempts >= 5 {
					device.trust_level = TrustLevel::Unreliable;
				}
			}

			self.save_paired_devices(&devices).await?;
		}

		Ok(())
	}

	/// Get a single paired device by ID
	pub async fn get_paired_device(
		&self,
		device_id: Uuid,
	) -> Result<Option<PersistedPairedDevice>> {
		let mut devices = self.load_paired_devices().await?;
		Ok(devices.remove(&device_id))
	}

	/// Remove a paired device
	pub async fn remove_paired_device(&self, device_id: Uuid) -> Result<bool> {
		tracing::debug!(
			"Attempting to remove paired device {} from persistence",
			device_id
		);

		let mut devices = self.load_paired_devices().await?;
		let removed = devices.remove(&device_id).is_some();

		if removed {
			tracing::info!("Device {} found in paired devices, removing...", device_id);

			// Delete the individual device key from KeyManager
			let key = Self::device_key(device_id);
			tracing::debug!("Deleting device key '{}' from KeyManager", key);

			if let Err(e) = self.key_manager.delete_secret(&key).await {
				tracing::warn!("Failed to delete device key {}: {}", key, e);
			} else {
				tracing::info!("Device key '{}' deleted from KeyManager", key);
			}

			// Update the device list (removes from paired_devices_list)
			tracing::debug!(
				"Updating paired devices list (now {} devices)",
				devices.len()
			);
			self.save_paired_devices(&devices).await?;

			tracing::info!(
				"Device {} successfully removed from persistence ({} devices remaining)",
				device_id,
				devices.len()
			);
		} else {
			tracing::warn!("Device {} not found in paired devices list", device_id);
		}

		Ok(removed)
	}

	/// Set device trust level
	pub async fn set_device_trust_level(
		&self,
		device_id: Uuid,
		trust_level: TrustLevel,
	) -> Result<()> {
		let mut devices = self.load_paired_devices().await?;

		if let Some(device) = devices.get_mut(&device_id) {
			device.trust_level = trust_level;
			self.save_paired_devices(&devices).await?;
		}

		Ok(())
	}

	/// Get devices that should auto-reconnect
	pub async fn get_auto_reconnect_devices(&self) -> Result<Vec<(Uuid, PersistedPairedDevice)>> {
		let devices = self.load_paired_devices().await?;

		let auto_reconnect: Vec<(Uuid, PersistedPairedDevice)> = devices
			.into_iter()
			.filter(|(device_id, device)| {
				let is_expired = device.session_keys.is_expired();
				let is_blocked = matches!(device.trust_level, TrustLevel::Blocked);

				// Simple rule: reconnect if paired, not blocked, and keys valid
				// Connection failures (Unreliable) don't prevent reconnection attempts
				let should_reconnect = !is_expired && !is_blocked;

				// Debug logging
				info!(
					"[AUTO-RECONNECT] Device {}: trust={:?}, expired={}, blocked={}, include={}",
					device.device_info.device_name,
					device.trust_level,
					is_expired,
					is_blocked,
					should_reconnect
				);

				should_reconnect
			})
			.collect();

		Ok(auto_reconnect)
	}

	/// Clean up expired devices
	pub async fn cleanup_expired_devices(&self) -> Result<usize> {
		let initial_devices = self.load_paired_devices().await?;
		let initial_count = initial_devices.len();

		// The load_paired_devices method already filters out expired devices
		// Just save the filtered result
		self.save_paired_devices(&initial_devices).await?;

		Ok(initial_count - initial_devices.len())
	}

	/// Clear all paired devices
	pub async fn clear_all_devices(&self) -> Result<()> {
		let device_ids = self.get_device_list().await?;

		for device_id in device_ids {
			let key = Self::device_key(device_id);
			if let Err(e) = self.key_manager.delete_secret(&key).await {
				eprintln!("Failed to delete device {}: {}", device_id, e);
			}
		}

		self.key_manager
			.delete_secret(Self::DEVICE_LIST_KEY)
			.await
			.map_err(|e| {
				NetworkingError::Protocol(format!("Failed to clear device list: {}", e))
			})?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::crypto::key_manager::KeyManager;
	use crate::service::network::utils::identity::NetworkFingerprint;
	use tempfile::TempDir;

	async fn create_test_persistence() -> (DevicePersistence, TempDir) {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let device_key_fallback = temp_dir.path().join("device_key");
		let key_manager = Arc::new(
			KeyManager::new_with_fallback(temp_dir.path().to_path_buf(), Some(device_key_fallback))
				.expect("Failed to create key manager"),
		);
		let persistence = DevicePersistence::new(key_manager);
		(persistence, temp_dir)
	}

	fn create_test_device_info() -> DeviceInfo {
		DeviceInfo {
			device_id: Uuid::new_v4(),
			device_name: "Test Device".to_string(),
			device_slug: "test-device".to_string(),
			device_type: super::super::DeviceType::Desktop,
			os_version: "Test OS 1.0".to_string(),
			app_version: "1.0.0".to_string(),
			network_fingerprint: NetworkFingerprint {
				node_id: "test_node_id".to_string(),
				public_key_hash: "test_hash".to_string(),
			},
			last_seen: Utc::now(),
		}
	}

	#[tokio::test]
	async fn test_add_and_load_paired_device() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		let device_id = Uuid::new_v4();
		let device_info = create_test_device_info();
		let session_keys = SessionKeys::from_shared_secret(vec![1, 2, 3, 4]);

		// Add paired device
		persistence
			.add_paired_device(
				device_id,
				device_info.clone(),
				session_keys.clone(),
				None,
				PairingType::Direct,
				None,
				None,
			)
			.await
			.unwrap();

		// Load devices
		let devices = persistence.load_paired_devices().await.unwrap();

		assert_eq!(devices.len(), 1);
		assert!(devices.contains_key(&device_id));

		let loaded_device = &devices[&device_id];
		assert_eq!(loaded_device.device_info.device_id, device_info.device_id);
		assert!(matches!(loaded_device.trust_level, TrustLevel::Trusted));
	}

	#[tokio::test]
	async fn test_auto_reconnect_devices() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		let device_id = Uuid::new_v4();
		let device_info = create_test_device_info();
		let session_keys = SessionKeys::from_shared_secret(vec![1, 2, 3, 4]);

		persistence
			.add_paired_device(
				device_id,
				device_info,
				session_keys,
				None,
				PairingType::Direct,
				None,
				None,
			)
			.await
			.unwrap();

		let auto_reconnect = persistence.get_auto_reconnect_devices().await.unwrap();
		assert_eq!(auto_reconnect.len(), 1);
		assert_eq!(auto_reconnect[0].0, device_id);
	}

	#[tokio::test]
	async fn test_trust_level_management() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		let device_id = Uuid::new_v4();
		let device_info = create_test_device_info();
		let session_keys = SessionKeys::from_shared_secret(vec![1, 2, 3, 4]);

		persistence
			.add_paired_device(
				device_id,
				device_info,
				session_keys,
				None,
				PairingType::Direct,
				None,
				None,
			)
			.await
			.unwrap();

		// Block the device
		persistence
			.set_device_trust_level(device_id, TrustLevel::Blocked)
			.await
			.unwrap();

		// Should not appear in auto-reconnect list
		let auto_reconnect = persistence.get_auto_reconnect_devices().await.unwrap();
		assert_eq!(auto_reconnect.len(), 0);
	}

	#[tokio::test]
	async fn test_encryption_decryption() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		let device_id = Uuid::new_v4();
		let device_info = create_test_device_info();
		let session_keys = SessionKeys::from_shared_secret(vec![1, 2, 3, 4]);

		// Add device (this will encrypt and save)
		persistence
			.add_paired_device(
				device_id,
				device_info.clone(),
				session_keys.clone(),
				None,
				PairingType::Direct,
				None,
				None,
			)
			.await
			.unwrap();

		// Load devices (this will decrypt)
		let loaded_devices = persistence.load_paired_devices().await.unwrap();

		assert_eq!(loaded_devices.len(), 1);
		assert!(loaded_devices.contains_key(&device_id));

		let loaded_device = &loaded_devices[&device_id];
		assert_eq!(loaded_device.device_info.device_id, device_info.device_id);
		assert_eq!(
			loaded_device.session_keys.shared_secret,
			session_keys.shared_secret
		);
	}
}
