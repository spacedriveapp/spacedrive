//! Persistence for paired devices and their connection info

use super::{DeviceInfo, SessionKeys};
use crate::service::network::{NetworkingError, Result};
use aes_gcm::{
	aead::{Aead, AeadCore, KeyInit, OsRng},
	Aes256Gcm, Key, Nonce,
};
use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

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
}

/// Encrypted device data for disk storage
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedDeviceData {
	/// Encrypted device data using AES-256-GCM
	ciphertext: Vec<u8>,
	/// Nonce for AES-GCM encryption
	nonce: Vec<u8>,
	/// Salt used for key derivation
	salt: Vec<u8>,
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

/// Collection of all paired devices (encrypted on disk)
#[derive(Debug, Serialize, Deserialize)]
struct PersistedPairedDevices {
	devices: HashMap<Uuid, EncryptedDeviceData>,
	last_saved: DateTime<Utc>,
	/// Version for future migration support
	version: u32,
}

/// Device persistence manager
pub struct DevicePersistence {
	data_dir: PathBuf,
	devices_file: PathBuf,
	device_key: [u8; 32],
}

impl DevicePersistence {
	/// Create a new device persistence manager
	pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
		let data_dir = data_dir.as_ref().to_path_buf();
		let networking_dir = data_dir.join("networking");
		let devices_file = networking_dir.join("paired_devices.json");

		// Load device key from fallback file (consistent with DeviceManager)
		let master_key_path = data_dir.join("master_key");
		let device_key = load_or_create_device_key(&master_key_path)
			.map_err(|e| NetworkingError::Protocol(format!("Failed to load device key: {}", e)))?;

		Ok(Self {
			data_dir: networking_dir,
			devices_file,
			device_key,
		})
	}

	#[cfg(test)]
	/// Create a test persistence manager with a fixed key (for testing only)
	pub fn new_for_test(data_dir: impl AsRef<Path>) -> Result<Self> {
		// Just use the regular new() method for tests now
		// The fallback file will ensure consistency across test runs
		Self::new(data_dir)
	}

	/// Derive encryption key from master key for device persistence
	fn derive_encryption_key(&self, salt: &[u8]) -> Result<[u8; 32]> {
		let master_key = &self.device_key;

		let hk = Hkdf::<Sha256>::new(Some(salt), master_key);
		let mut derived_key = [0u8; 32];
		hk.expand(b"spacedrive-device-persistence", &mut derived_key)
			.map_err(|e| NetworkingError::Protocol(format!("Key derivation failed: {}", e)))?;

		Ok(derived_key)
	}

	/// Encrypt device data using master key-derived encryption key
	fn encrypt_device_data(&self, device: &PersistedPairedDevice) -> Result<EncryptedDeviceData> {
		// Generate random salt for key derivation
		let mut salt = [0u8; 32];
		aes_gcm::aead::rand_core::RngCore::fill_bytes(&mut OsRng, &mut salt);

		// Derive encryption key
		let encryption_key = self.derive_encryption_key(&salt)?;
		let key = Key::<Aes256Gcm>::from_slice(&encryption_key);
		let cipher = Aes256Gcm::new(key);

		// Generate nonce
		let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

		// Serialize and encrypt device data
		let plaintext =
			serde_json::to_vec(device).map_err(|e| NetworkingError::Serialization(e))?;

		let ciphertext = cipher
			.encrypt(&nonce, plaintext.as_ref())
			.map_err(|e| NetworkingError::Protocol(format!("Encryption failed: {}", e)))?;

		Ok(EncryptedDeviceData {
			ciphertext,
			nonce: nonce.to_vec(),
			salt: salt.to_vec(),
		})
	}

	/// Decrypt device data using master key-derived encryption key
	fn decrypt_device_data(
		&self,
		encrypted_data: &EncryptedDeviceData,
	) -> Result<PersistedPairedDevice> {
		// Derive encryption key using stored salt
		let encryption_key = self.derive_encryption_key(&encrypted_data.salt)?;
		let key = Key::<Aes256Gcm>::from_slice(&encryption_key);
		let cipher = Aes256Gcm::new(key);

		// Decrypt data
		let nonce = Nonce::from_slice(&encrypted_data.nonce);
		let plaintext = cipher
			.decrypt(nonce, encrypted_data.ciphertext.as_ref())
			.map_err(|e| NetworkingError::Protocol(format!("Decryption failed: {}", e)))?;

		// Deserialize device data
		let device: PersistedPairedDevice =
			serde_json::from_slice(&plaintext).map_err(|e| NetworkingError::Serialization(e))?;

		Ok(device)
	}

	/// Save paired devices to disk (encrypted)
	pub async fn save_paired_devices(
		&self,
		devices: &HashMap<Uuid, PersistedPairedDevice>,
	) -> Result<()> {
		// Ensure data directory exists
		if let Some(parent) = self.devices_file.parent() {
			fs::create_dir_all(parent)
				.await
				.map_err(NetworkingError::Io)?;
		}

		// Encrypt each device individually
		let mut encrypted_devices = HashMap::new();
		for (device_id, device) in devices {
			let encrypted_data = self.encrypt_device_data(device)?;
			encrypted_devices.insert(*device_id, encrypted_data);
		}

		let persisted = PersistedPairedDevices {
			devices: encrypted_devices,
			last_saved: Utc::now(),
			version: 1, // Current version
		};

		// Write to temporary file first, then rename for atomic operation
		let temp_file = self.devices_file.with_extension("tmp");
		let json_data = serde_json::to_string_pretty(&persisted)
			.map_err(|e| NetworkingError::Serialization(e))?;

		fs::write(&temp_file, json_data)
			.await
			.map_err(NetworkingError::Io)?;
		fs::rename(&temp_file, &self.devices_file)
			.await
			.map_err(NetworkingError::Io)?;

		println!("Saved {} paired devices (encrypted)", devices.len());
		Ok(())
	}

	/// Load paired devices from disk (decrypt)
	pub async fn load_paired_devices(&self) -> Result<HashMap<Uuid, PersistedPairedDevice>> {
		if !self.devices_file.exists() {
			return Ok(HashMap::new());
		}

		let json_data = fs::read_to_string(&self.devices_file)
			.await
			.map_err(NetworkingError::Io)?;

		let persisted: PersistedPairedDevices =
			serde_json::from_str(&json_data).map_err(|e| NetworkingError::Serialization(e))?;

		// Check version compatibility
		if persisted.version != 1 {
			return Err(NetworkingError::Protocol(format!(
				"Unsupported device persistence version: {}",
				persisted.version
			)));
		}

		// Decrypt each device individually
		let mut devices = HashMap::new();
		for (device_id, encrypted_data) in persisted.devices {
			match self.decrypt_device_data(&encrypted_data) {
				Ok(device) => {
					// Filter out devices with expired session keys
					if !device.session_keys.is_expired() {
						devices.insert(device_id, device);
					}
				}
				Err(e) => {
					eprintln!("Failed to decrypt device {}: {}", device_id, e);
					// Continue loading other devices even if one fails
				}
			}
		}

		Ok(devices)
	}

	/// Add a newly paired device
	pub async fn add_paired_device(
		&self,
		device_id: Uuid,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
		relay_url: Option<String>,
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

	/// Remove a paired device
	pub async fn remove_paired_device(&self, device_id: Uuid) -> Result<bool> {
		let mut devices = self.load_paired_devices().await?;
		let removed = devices.remove(&device_id).is_some();

		if removed {
			self.save_paired_devices(&devices).await?;
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
				eprintln!(
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
		if self.devices_file.exists() {
			fs::remove_file(&self.devices_file)
				.await
				.map_err(NetworkingError::Io)?;
		}
		Ok(())
	}

	/// Get file path
	pub fn devices_file_path(&self) -> &Path {
		&self.devices_file
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::service::network::utils::identity::NetworkFingerprint;
	use tempfile::TempDir;

	async fn create_test_persistence() -> (DevicePersistence, TempDir) {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let persistence = DevicePersistence::new_for_test(temp_dir.path())
			.expect("Failed to create test persistence");
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
			.add_paired_device(device_id, device_info.clone(), session_keys.clone(), None)
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
			.add_paired_device(device_id, device_info, session_keys, None)
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
			.add_paired_device(device_id, device_info, session_keys, None)
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
			.add_paired_device(device_id, device_info.clone(), session_keys.clone(), None)
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

	#[tokio::test]
	async fn test_file_encryption_format() {
		let (persistence, temp_dir) = create_test_persistence().await;

		let device_id = Uuid::new_v4();
		let device_info = create_test_device_info();
		let session_keys = SessionKeys::from_shared_secret(vec![1, 2, 3, 4]);

		// Add device
		persistence
			.add_paired_device(device_id, device_info, session_keys, None)
			.await
			.unwrap();

		// Read the raw file - it should be encrypted (not contain plaintext session keys)
		let file_path = temp_dir
			.path()
			.join("networking")
			.join("paired_devices.json");
		let raw_content = tokio::fs::read_to_string(&file_path).await.unwrap();

		println!("Raw file content: {}", raw_content);

		// The file should not contain the plaintext session key bytes
		assert!(!raw_content.contains("\"shared_secret\":[1,2,3,4]"));

		// But should contain encrypted structure
		assert!(raw_content.contains("\"ciphertext\""));
		assert!(raw_content.contains("\"nonce\""));
		assert!(raw_content.contains("\"salt\""));
		assert!(raw_content.contains("\"version\": 1"));

		println!("Device data is properly encrypted on disk");
	}
}

/// Load device key from file, or create a new one
fn load_or_create_device_key(path: &PathBuf) -> std::io::Result<[u8; 32]> {
	use rand::RngCore;

	// Try to load from file
	if path.exists() {
		let data = std::fs::read(path)?;
		if data.len() == 32 {
			let mut key = [0u8; 32];
			key.copy_from_slice(&data);
			return Ok(key);
		}
	}

	// Create new key
	let mut key = [0u8; 32];
	rand::thread_rng().fill_bytes(&mut key);

	// Save to file
	std::fs::write(path, &key)?;

	Ok(key)
}
