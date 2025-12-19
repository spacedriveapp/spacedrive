//! Device configuration persistence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Device configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
	/// Unique device identifier
	pub id: Uuid,

	/// User-friendly device name
	pub name: String,

	/// Unique slug for URI addressing
	pub slug: String,

	/// Per-library slug overrides for collision resolution
	/// Maps library_id -> slug
	/// When a device joins a library where its slug conflicts with another device,
	/// the slug for that specific library is stored here without affecting other libraries
	#[serde(default)]
	pub library_slug_overrides: HashMap<Uuid, String>,

	/// When this device was first initialized
	pub created_at: DateTime<Utc>,

	/// Hardware model (if detectable)
	pub hardware_model: Option<String>,

	/// Operating system
	pub os: String,

	/// Operating system version (if detectable)
	#[serde(default)]
	pub os_version: Option<String>,

	// --- Hardware Specifications ---
	/// CPU model name
	#[serde(default)]
	pub cpu_model: Option<String>,

	/// CPU architecture
	#[serde(default)]
	pub cpu_architecture: Option<String>,

	/// Number of physical CPU cores
	#[serde(default)]
	pub cpu_cores_physical: Option<u32>,

	/// Number of logical CPU cores
	#[serde(default)]
	pub cpu_cores_logical: Option<u32>,

	/// CPU base frequency in MHz
	#[serde(default)]
	pub cpu_frequency_mhz: Option<i64>,

	/// Total system memory in bytes
	#[serde(default)]
	pub memory_total_bytes: Option<i64>,

	/// Device form factor
	#[serde(default)]
	pub form_factor: Option<String>,

	/// Device manufacturer
	#[serde(default)]
	pub manufacturer: Option<String>,

	/// GPU model names
	#[serde(default)]
	pub gpu_models: Option<Vec<String>>,

	/// Boot disk type
	#[serde(default)]
	pub boot_disk_type: Option<String>,

	/// Boot disk capacity in bytes
	#[serde(default)]
	pub boot_disk_capacity_bytes: Option<i64>,

	/// Total swap space in bytes
	#[serde(default)]
	pub swap_total_bytes: Option<i64>,

	/// Spacedrive version that created this config
	pub version: String,
}

impl DeviceConfig {
	/// Create a new device configuration
	pub fn new(name: String, os: String) -> Self {
		// Generate slug from name
		let slug = crate::domain::device::Device::generate_slug(&name);

		Self {
			id: Uuid::new_v4(),
			name,
			slug,
			library_slug_overrides: HashMap::new(),
			created_at: Utc::now(),
			hardware_model: None,
			os,
			os_version: None,
			// Hardware specs - will be populated later
			cpu_model: None,
			cpu_architecture: None,
			cpu_cores_physical: None,
			cpu_cores_logical: None,
			cpu_frequency_mhz: None,
			memory_total_bytes: None,
			form_factor: None,
			manufacturer: None,
			gpu_models: None,
			boot_disk_type: None,
			boot_disk_capacity_bytes: None,
			swap_total_bytes: None,
			version: env!("CARGO_PKG_VERSION").to_string(),
		}
	}

	/// Get the configuration file path for the current platform
	pub fn config_path() -> Result<PathBuf, super::DeviceError> {
		let base_path = if cfg!(target_os = "macos") {
			dirs::data_dir()
				.ok_or(super::DeviceError::ConfigPathNotFound)?
				.join("com.spacedrive")
		} else if cfg!(target_os = "linux") {
			dirs::config_dir()
				.ok_or(super::DeviceError::ConfigPathNotFound)?
				.join("spacedrive")
		} else if cfg!(target_os = "windows") {
			dirs::config_dir()
				.ok_or(super::DeviceError::ConfigPathNotFound)?
				.join("Spacedrive")
		} else {
			return Err(super::DeviceError::UnsupportedPlatform);
		};

		Ok(base_path.join("device.json"))
	}

	/// Load configuration from disk
	pub fn load() -> Result<Self, super::DeviceError> {
		let path = Self::config_path()?;

		if !path.exists() {
			return Err(super::DeviceError::NotInitialized);
		}

		let content = std::fs::read_to_string(&path)?;
		let config: Self = serde_json::from_str(&content)?;

		Ok(config)
	}

	/// Save configuration to disk
	pub fn save(&self) -> Result<(), super::DeviceError> {
		let path = Self::config_path()?;

		// Ensure parent directory exists
		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent)?;
		}

		let content = serde_json::to_string_pretty(self)?;
		std::fs::write(&path, content)?;

		Ok(())
	}

	/// Load configuration from a specific directory
	pub fn load_from(data_dir: &PathBuf) -> Result<Self, super::DeviceError> {
		let path = data_dir.join("device.json");

		if !path.exists() {
			return Err(super::DeviceError::NotInitialized);
		}

		let content = std::fs::read_to_string(&path)?;
		let config: Self = serde_json::from_str(&content)?;

		Ok(config)
	}

	/// Save configuration to a specific directory
	pub fn save_to(&self, data_dir: &PathBuf) -> Result<(), super::DeviceError> {
		// Ensure directory exists
		std::fs::create_dir_all(data_dir)?;

		let path = data_dir.join("device.json");
		let content = serde_json::to_string_pretty(self)?;
		std::fs::write(&path, content)?;

		Ok(())
	}
}
