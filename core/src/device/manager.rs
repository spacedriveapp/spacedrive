//! Device manager for handling device lifecycle

use super::config::DeviceConfig;
use crate::crypto::device_key_manager::{DeviceKeyError, DeviceKeyManager};
use crate::domain::device::{Device, OperatingSystem};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during device management
#[derive(Error, Debug)]
pub enum DeviceError {
	#[error("Device not initialized")]
	NotInitialized,

	#[error("Config path not found")]
	ConfigPathNotFound,

	#[error("Unsupported platform")]
	UnsupportedPlatform,

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Lock poisoned")]
	LockPoisoned,

	#[error("Master key error: {0}")]
	MasterKey(#[from] DeviceKeyError),
}

/// Manages the current device state
pub struct DeviceManager {
	/// Current device configuration
	config: Arc<RwLock<DeviceConfig>>,
	/// Master encryption key manager
	device_key_manager: DeviceKeyManager,
	/// Custom data directory (if any)
	data_dir: Option<PathBuf>,
	/// In-memory cache of all devices in the library (slug -> device_id)
	/// Populated when library loads, provides O(1) slug resolution for all devices
	device_cache: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl DeviceManager {
	// Simple new function for testing
	pub fn new() -> Result<Self, DeviceError> {
		let data_dir = PathBuf::new();
		return Self::init(&data_dir, None);
	}
	/// Initialize the device manager with a custom data directory and optional device name
	pub fn init(data_dir: &PathBuf, device_name: Option<String>) -> Result<Self, DeviceError> {
		let mut config = match DeviceConfig::load_from(data_dir) {
			Ok(config) => config,
			Err(DeviceError::NotInitialized) => {
				// Create new device configuration
				let os = detect_os();
				let name = device_name.clone().unwrap_or_else(get_device_name);
				let mut config = DeviceConfig::new(name, os);

				// Try to detect hardware model
				config.hardware_model = detect_hardware_model();

				// Save the new configuration
				config.save_to(data_dir)?;
				config
			}
			Err(e) => return Err(e),
		};

		// Update device name if provided (allows picking up name changes from system settings)
		if let Some(name) = device_name {
			if config.name != name {
				config.name = name;
				config.save_to(data_dir)?;
			}
		}

		// Use fallback file for master key when using custom data directory
		let master_key_path = data_dir.join("master_key");
		let device_key_manager = DeviceKeyManager::new_with_fallback(master_key_path)?;
		// Initialize master key on first run
		device_key_manager.get_or_create_master_key()?;

		Ok(Self {
			config: Arc::new(RwLock::new(config)),
			device_key_manager,
			data_dir: Some(data_dir.clone()),
			device_cache: Arc::new(RwLock::new(HashMap::new())),
		})
	}

	/// Get the current device ID
	pub fn device_id(&self) -> Result<Uuid, DeviceError> {
		self.config
			.read()
			.map(|c| c.id)
			.map_err(|_| DeviceError::LockPoisoned)
	}

	/// Resolve device UUID from slug
	/// Checks in-memory cache (all library devices) then falls back to current device config
	pub fn resolve_by_slug(&self, slug: &str) -> Option<Uuid> {
		// Check cache first (covers all library devices)
		if let Ok(cache) = self.device_cache.read() {
			if let Some(&device_id) = cache.get(slug) {
				return Some(device_id);
			}
		}

		// Fallback: check if it's the current device
		let config = self.config.read().ok()?;
		if config.slug == slug {
			Some(config.id)
		} else {
			None
		}
	}

	/// Resolve device slug from UUID
	/// Inverse of resolve_by_slug - checks cache and current device config
	pub fn get_device_slug(&self, device_id: Uuid) -> Option<String> {
		// Check if it's the current device first
		if let Ok(config) = self.config.read() {
			if config.id == device_id {
				return Some(config.slug.clone());
			}
		}

		// Search cache for matching UUID
		if let Ok(cache) = self.device_cache.read() {
			for (slug, &cached_id) in cache.iter() {
				if cached_id == device_id {
					return Some(slug.clone());
				}
			}
		}

		None
	}

	/// Load devices from library database into cache
	/// Called when a library is opened to enable slug resolution for all devices
	pub async fn load_library_devices(
		&self,
		db: &sea_orm::DatabaseConnection,
	) -> Result<(), DeviceError> {
		use crate::infra::db::entities::device;
		use sea_orm::EntityTrait;

		let devices = device::Entity::find().all(db).await.map_err(|e| {
			DeviceError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
		})?;

		let mut cache = self.device_cache.write().map_err(|_| DeviceError::LockPoisoned)?;
		cache.clear();

		for device in devices {
			cache.insert(device.slug, device.uuid);
		}

		tracing::debug!("Loaded {} devices into DeviceManager cache", cache.len());

		Ok(())
	}

	/// Clear the device cache (when library closes)
	pub fn clear_device_cache(&self) -> Result<(), DeviceError> {
		let mut cache = self.device_cache.write().map_err(|_| DeviceError::LockPoisoned)?;
		let count = cache.len();
		cache.clear();
		tracing::debug!("Cleared {} devices from DeviceManager cache", count);
		Ok(())
	}

	/// Add a device to the cache (when new device pairs or syncs)
	pub fn cache_device(&self, slug: String, device_id: Uuid) -> Result<(), DeviceError> {
		let mut cache = self.device_cache.write().map_err(|_| DeviceError::LockPoisoned)?;
		cache.insert(slug.clone(), device_id);
		tracing::debug!("Cached device: {} -> {}", slug, device_id);
		Ok(())
	}

	/// Get the current device as a domain Device object
	pub async fn current_device(&self) -> Device {
		let config = self.config.read().unwrap();
		Device {
			id: config.id,
			name: config.name.clone(),
			slug: config.slug.clone(),
			os: parse_os(&config.os),
			os_version: None,
			hardware_model: config.hardware_model.clone(),
			network_addresses: vec![],
			capabilities: serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			}),
			is_online: true,
			last_seen_at: chrono::Utc::now(),
			sync_enabled: true,
			last_sync_at: None,
			last_state_watermark: None,
			last_shared_watermark: None,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}

	/// Get the current device configuration
	pub fn config(&self) -> Result<DeviceConfig, DeviceError> {
		self.config
			.read()
			.map(|c| c.clone())
			.map_err(|_| DeviceError::LockPoisoned)
	}

	/// Create a Device domain object from current configuration
	pub fn to_device(&self) -> Result<Device, DeviceError> {
		let config = self.config()?;

		// Create device with loaded configuration
		let mut device = Device::new(config.name.clone());
		device.id = config.id;
		device.slug = config.slug.clone();
		device.os = parse_os(&config.os);
		device.hardware_model = config.hardware_model.clone();
		device.created_at = config.created_at;

		Ok(device)
	}

	/// Update device name
	pub fn set_name(&self, name: String) -> Result<(), DeviceError> {
		let mut config = self.config.write().map_err(|_| DeviceError::LockPoisoned)?;

		config.name = name;

		// Save to the appropriate location based on whether we have a custom data dir
		if let Some(data_dir) = &self.data_dir {
			config.save_to(data_dir)?;
		} else {
			config.save()?;
		}

		Ok(())
	}

	/// Get the master encryption key
	pub fn master_key(&self) -> Result<[u8; 32], DeviceError> {
		Ok(self.device_key_manager.get_master_key()?)
	}

	/// Get the master encryption key as hex string
	pub fn master_key_hex(&self) -> Result<String, DeviceError> {
		Ok(self.device_key_manager.get_master_key_hex()?)
	}

	/// Regenerate the master encryption key (dangerous operation)
	pub fn regenerate_device_key(&self) -> Result<[u8; 32], DeviceError> {
		Ok(self.device_key_manager.regenerate_master_key()?)
	}
}

/// Get the device name from the system
fn get_device_name() -> String {
	whoami::devicename()
}

/// Detect the operating system
fn detect_os() -> String {
	if cfg!(target_os = "macos") {
		"macOS".to_string()
	} else if cfg!(target_os = "windows") {
		"Windows".to_string()
	} else if cfg!(target_os = "linux") {
		"Linux".to_string()
	} else if cfg!(target_os = "ios") {
		"iOS".to_string()
	} else if cfg!(target_os = "android") {
		"Android".to_string()
	} else {
		"Unknown".to_string()
	}
}

/// Parse OS string back to enum
fn parse_os(os: &str) -> OperatingSystem {
	match os {
		"macOS" => OperatingSystem::MacOS,
		"Windows" => OperatingSystem::Windows,
		"Linux" => OperatingSystem::Linux,
		"iOS" => OperatingSystem::IOs,
		"Android" => OperatingSystem::Android,
		_ => OperatingSystem::Other,
	}
}

/// Try to detect hardware model
fn detect_hardware_model() -> Option<String> {
	#[cfg(target_os = "macos")]
	{
		// Try to get model from system_profiler
		use std::process::Command;

		let output = Command::new("system_profiler")
			.args(&["SPHardwareDataType", "-json"])
			.output()
			.ok()?;

		if output.status.success() {
			let json_str = String::from_utf8_lossy(&output.stdout);
			// Simple extraction - in production would use proper JSON parsing
			if let Some(start) = json_str.find("\"machine_model\":") {
				let substr = &json_str[start + 17..];
				if let Some(end) = substr.find('"') {
					return Some(substr[..end].to_string());
				}
			}
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	#[test]
	fn test_device_config() {
		let config = DeviceConfig::new("Test Device".to_string(), "Linux".to_string());
		assert_eq!(config.name, "Test Device");
		assert_eq!(config.os, "Linux");
		assert!(config.hardware_model.is_none());
	}
}
