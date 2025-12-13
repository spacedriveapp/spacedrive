//! Device manager for handling device lifecycle

use super::config::DeviceConfig;
use crate::crypto::key_manager::KeyManager;
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
	MasterKey(String),
}

/// Manages the current device state
pub struct DeviceManager {
	/// Current device configuration
	config: Arc<RwLock<DeviceConfig>>,
	/// Key manager for device encryption key
	key_manager: Arc<KeyManager>,
	/// Custom data directory (if any)
	data_dir: Option<PathBuf>,
	/// Pre-library cache: Paired devices from DeviceRegistry (slug -> device_id)
	/// Used for operations before library membership (e.g., pairing, standalone file transfers)
	paired_device_cache: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl DeviceManager {
	/// Initialize the device manager with a custom data directory and optional device name
	pub fn init(
		data_dir: &PathBuf,
		key_manager: Arc<KeyManager>,
		device_name: Option<String>,
	) -> Result<Self, DeviceError> {
		let mut config = match DeviceConfig::load_from(data_dir) {
			Ok(mut config) => {
				// For existing configs, detect and populate missing fields
				let mut needs_save = false;

				if config.hardware_model.is_none() {
					config.hardware_model = detect_hardware_model();
					needs_save = true;
				}

				if config.os_version.is_none() {
					config.os_version = detect_os_version();
					needs_save = true;
				}

				// Save if we detected any new values
				if needs_save {
					config.save_to(data_dir)?;
				}

				config
			}
			Err(DeviceError::NotInitialized) => {
				// Create new device configuration
				let os = detect_os();
				let name = device_name.clone().unwrap_or_else(get_device_name);
				let mut config = DeviceConfig::new(name, os);

				// Try to detect hardware model and OS version
				config.hardware_model = detect_hardware_model();
				config.os_version = detect_os_version();

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

		Ok(Self {
			config: Arc::new(RwLock::new(config)),
			key_manager,
			data_dir: Some(data_dir.clone()),
			paired_device_cache: Arc::new(RwLock::new(HashMap::new())),
		})
	}

	/// Get the current device ID
	pub fn device_id(&self) -> Result<Uuid, DeviceError> {
		self.config
			.read()
			.map(|c| c.id)
			.map_err(|_| DeviceError::LockPoisoned)
	}

	/// Resolve device UUID from slug (pre-library / paired devices)
	/// Used for operations before library membership (e.g., standalone file transfers)
	/// For library operations, use Library::resolve_device_slug() instead
	pub fn resolve_by_slug(&self, slug: &str) -> Option<Uuid> {
		// Priority 1: Check if it's the current device first
		if let Ok(config) = self.config.read() {
			if config.slug == slug {
				return Some(config.id);
			}
		}

		// Priority 2: Check paired devices cache
		if let Ok(cache) = self.paired_device_cache.read() {
			cache.get(slug).copied()
		} else {
			None
		}
	}

	/// Resolve device slug from UUID (pre-library / paired devices)
	/// Inverse of resolve_by_slug - checks current device config and paired device cache
	/// For library operations, query the Library's device cache instead
	pub fn get_device_slug(&self, device_id: Uuid) -> Option<String> {
		// Check if it's the current device first
		if let Ok(config) = self.config.read() {
			if config.id == device_id {
				return Some(config.slug.clone());
			}
		}

		// Search paired device cache for matching UUID
		if let Ok(cache) = self.paired_device_cache.read() {
			for (slug, &cached_id) in cache.iter() {
				if cached_id == device_id {
					return Some(slug.clone());
				}
			}
		}

		None
	}

	/// Load devices from library database into paired device cache
	/// DEPRECATED: Use Library::load_device_cache_from_db() instead for library-scoped resolution
	/// This method is only for pre-library operations (e.g., pairing)
	#[deprecated(note = "Use Library::load_device_cache_from_db() for library-specific devices")]
	pub async fn load_library_devices(
		&self,
		db: &sea_orm::DatabaseConnection,
	) -> Result<(), DeviceError> {
		use crate::infra::db::entities::device;
		use sea_orm::EntityTrait;

		let devices = device::Entity::find().all(db).await.map_err(|e| {
			DeviceError::Io(std::io::Error::new(
				std::io::ErrorKind::Other,
				e.to_string(),
			))
		})?;

		let mut cache = self
			.paired_device_cache
			.write()
			.map_err(|_| DeviceError::LockPoisoned)?;
		cache.clear();

		for device in devices {
			cache.insert(device.slug, device.uuid);
		}

		tracing::debug!(
			"Loaded {} devices into DeviceManager paired cache",
			cache.len()
		);

		Ok(())
	}

	/// Clear the paired device cache
	/// Used for cleanup when pairing state needs to be reset
	pub fn clear_paired_device_cache(&self) -> Result<(), DeviceError> {
		let mut cache = self
			.paired_device_cache
			.write()
			.map_err(|_| DeviceError::LockPoisoned)?;
		let count = cache.len();
		cache.clear();
		tracing::debug!("Cleared {} paired devices from DeviceManager cache", count);
		Ok(())
	}

	/// Add a paired device to the pre-library cache (when new device pairs)
	/// For library-specific device caching, use Library::cache_device() instead
	pub fn cache_paired_device(&self, slug: String, device_id: Uuid) -> Result<(), DeviceError> {
		let mut cache = self
			.paired_device_cache
			.write()
			.map_err(|_| DeviceError::LockPoisoned)?;
		cache.insert(slug.clone(), device_id);
		tracing::debug!("Cached paired device: {} -> {}", slug, device_id);
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
			os_version: config.os_version.clone(),
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
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
			// Ephemeral fields
			is_current: true,
			is_paired: false,
			is_connected: false,
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
		device.os_version = config.os_version.clone();
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

	/// Get the effective slug for this device in a specific library context
	/// Returns library-specific override if set, otherwise returns global slug
	pub fn slug_for_library(&self, library_id: Uuid) -> Result<String, DeviceError> {
		let config = self.config.read().map_err(|_| DeviceError::LockPoisoned)?;

		// Check for library-specific override first
		if let Some(override_slug) = config.library_slug_overrides.get(&library_id) {
			return Ok(override_slug.clone());
		}

		// Fall back to global slug
		Ok(config.slug.clone())
	}

	/// Set a library-specific slug override (for collision resolution)
	/// This allows the device to have different slugs in different libraries
	/// without modifying the global slug or affecting other libraries
	pub fn set_library_slug(&self, library_id: Uuid, slug: String) -> Result<(), DeviceError> {
		let mut config = self.config.write().map_err(|_| DeviceError::LockPoisoned)?;

		config.library_slug_overrides.insert(library_id, slug);

		// Save to the appropriate location based on whether we have a custom data dir
		if let Some(data_dir) = &self.data_dir {
			config.save_to(data_dir)?;
		} else {
			config.save()?;
		}

		Ok(())
	}

	/// Get the master encryption key from KeyManager
	pub async fn master_key(&self) -> Result<[u8; 32], DeviceError> {
		self.key_manager
			.get_device_key()
			.await
			.map_err(|e| DeviceError::MasterKey(e.to_string()))
	}

	/// Get the master encryption key as hex string
	pub async fn master_key_hex(&self) -> Result<String, DeviceError> {
		let key = self.master_key().await?;
		Ok(hex::encode(key))
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
		use std::process::Command;

		let output = Command::new("sysctl")
			.args(["-n", "hw.model"])
			.output()
			.ok()?;

		if output.status.success() {
			let model = String::from_utf8_lossy(&output.stdout).trim().to_string();
			if !model.is_empty() {
				return Some(model);
			}
		}
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		let output = Command::new("wmic")
			.args(["computersystem", "get", "model"])
			.output()
			.ok()?;

		if output.status.success() {
			let stdout = String::from_utf8_lossy(&output.stdout);
			if let Some(model) = stdout.lines().nth(1) {
				let model = model.trim().to_string();
				if !model.is_empty() {
					return Some(model);
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::fs;

		if let Ok(model) = fs::read_to_string("/sys/devices/virtual/dmi/id/product_name") {
			let model = model.trim().to_string();
			if !model.is_empty() && model != "System Product Name" {
				return Some(model);
			}
		}
	}

	None
}

/// Try to detect OS version
fn detect_os_version() -> Option<String> {
	#[cfg(target_os = "macos")]
	{
		use std::process::Command;

		let output = Command::new("sw_vers")
			.args(["-productVersion"])
			.output()
			.ok()?;

		if output.status.success() {
			let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
			if !version.is_empty() {
				return Some(version);
			}
		}
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		let output = Command::new("cmd").args(["/c", "ver"]).output().ok()?;

		if output.status.success() {
			let stdout = String::from_utf8_lossy(&output.stdout);
			if let Some(start) = stdout.find("Version ") {
				let version_str = &stdout[start + 8..];
				if let Some(end) = version_str.find(']') {
					return Some(version_str[..end].trim().to_string());
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::fs;

		if let Ok(contents) = fs::read_to_string("/etc/os-release") {
			for line in contents.lines() {
				if line.starts_with("VERSION=") || line.starts_with("VERSION_ID=") {
					let version = line.split('=').nth(1)?;
					let version = version.trim_matches('"').to_string();
					if !version.is_empty() {
						return Some(version);
					}
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
