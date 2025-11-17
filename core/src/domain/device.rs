//! Unified device model - no more node/device/instance confusion
//!
//! A Device represents a machine running Spacedrive. This unifies the old
//! concepts of Node, Device, and Instance into one clear model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A device running Spacedrive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
	/// Unique identifier for this device
	pub id: Uuid,

	/// Human-readable name
	pub name: String,

	/// Unique slug for URI addressing (e.g., "jamies-macbook")
	pub slug: String,

	/// Operating system
	pub os: OperatingSystem,

	/// Operating system version
	pub os_version: Option<String>,

	/// Hardware model (e.g., "MacBook Pro", "iPhone 15")
	pub hardware_model: Option<String>,

	/// Network addresses for P2P connections
	pub network_addresses: Vec<String>,

	/// Device capabilities (indexing, P2P, volume detection, etc.)
	pub capabilities: serde_json::Value,

	/// Whether this device is currently online
	pub is_online: bool,

	/// Last time this device was seen
	pub last_seen_at: DateTime<Utc>,

	/// Whether sync is enabled for this device
	pub sync_enabled: bool,

	/// Last time this device synced
	pub last_sync_at: Option<DateTime<Utc>>,

	/// When this device was first added
	pub created_at: DateTime<Utc>,

	/// When this device info was last updated
	pub updated_at: DateTime<Utc>,
}

/// Operating system types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OperatingSystem {
	MacOS,
	Windows,
	Linux,
	IOs,
	Android,
	Other,
}

impl Device {
	/// Generate URL-safe slug from device name
	/// Converts to lowercase and replaces non-alphanumeric chars with hyphens
	pub fn generate_slug(name: &str) -> String {
		name.to_lowercase()
			.chars()
			.map(|c| if c.is_alphanumeric() { c } else { '-' })
			.collect::<String>()
			.trim_matches('-')
			.to_string()
	}

	/// Create a new device
	pub fn new(name: String) -> Self {
		let now = Utc::now();
		let slug = Self::generate_slug(&name);
		Self {
			id: Uuid::new_v4(),
			name,
			slug,
			os: detect_operating_system(),
			os_version: detect_os_version(),
			hardware_model: detect_hardware_model(),
			network_addresses: Vec::new(),
			capabilities: serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			}),
			is_online: true,
			last_seen_at: now,
			sync_enabled: true,
			last_sync_at: None,
			created_at: now,
			updated_at: now,
		}
	}

	/// Create the current device
	pub fn current() -> Self {
		Self::new(get_device_name())
	}

	/// Update network addresses
	pub fn update_network_addresses(&mut self, addresses: Vec<String>) {
		self.network_addresses = addresses;
		self.updated_at = Utc::now();
	}

	/// Mark device as online
	pub fn mark_online(&mut self) {
		self.is_online = true;
		self.last_seen_at = Utc::now();
		self.updated_at = Utc::now();
	}

	/// Mark device as offline
	pub fn mark_offline(&mut self) {
		self.is_online = false;
		self.updated_at = Utc::now();
	}

	/// Check if this is the current device
	pub fn is_current(&self, current_device_id: Uuid) -> bool {
		self.id == current_device_id
	}
}

/// Get the device name from the system
fn get_device_name() -> String {
	#[cfg(target_os = "macos")]
	{
		return whoami::devicename();
	}

	#[cfg(any(target_os = "windows", target_os = "linux"))]
	{
		if let Ok(name) = hostname::get() {
			if let Ok(name_str) = name.into_string() {
				return name_str;
			}
		}
	}

	"Unknown Device".to_string()
}

/// Detect the operating system
fn detect_operating_system() -> OperatingSystem {
	#[cfg(target_os = "macos")]
	return OperatingSystem::MacOS;

	#[cfg(target_os = "windows")]
	return OperatingSystem::Windows;

	#[cfg(target_os = "linux")]
	return OperatingSystem::Linux;

	#[cfg(target_os = "ios")]
	return OperatingSystem::IOs;

	#[cfg(target_os = "android")]
	return OperatingSystem::Android;

	#[cfg(not(any(
		target_os = "macos",
		target_os = "windows",
		target_os = "linux",
		target_os = "ios",
		target_os = "android"
	)))]
	return OperatingSystem::Other;
}

/// Get hardware model information
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
		// Use wmic to get computer model
		use std::process::Command;

		let output = Command::new("wmic")
			.args(["computersystem", "get", "model"])
			.output()
			.ok()?;

		if output.status.success() {
			let stdout = String::from_utf8_lossy(&output.stdout);
			// Skip first line (header) and get model
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
		// Try to read from DMI
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

/// Get operating system version
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
			// Extract version from output like "Microsoft Windows [Version 10.0.19045.3570]"
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

		// Try to read from /etc/os-release
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

	#[cfg(target_os = "ios")]
	{
		// iOS version detection would require iOS-specific APIs
		// This would typically be done via the iOS SDK
		return None;
	}

	#[cfg(target_os = "android")]
	{
		// Android version detection would require Android-specific APIs
		return None;
	}

	None
}

impl std::fmt::Display for OperatingSystem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			OperatingSystem::MacOS => write!(f, "macOS"),
			OperatingSystem::Windows => write!(f, "Windows"),
			OperatingSystem::Linux => write!(f, "Linux"),
			OperatingSystem::IOs => write!(f, "iOS"),
			OperatingSystem::Android => write!(f, "Android"),
			OperatingSystem::Other => write!(f, "Other"),
		}
	}
}

// Conversion implementations for database entities
use crate::infra::db::entities;
use sea_orm::ActiveValue;

impl From<Device> for entities::device::ActiveModel {
	fn from(device: Device) -> Self {
		use sea_orm::ActiveValue::*;

		entities::device::ActiveModel {
			id: NotSet, // Auto-increment
			uuid: Set(device.id),
			name: Set(device.name),
			slug: Set(device.slug),
			os: Set(device.os.to_string()),
			os_version: Set(device.os_version),
			hardware_model: Set(device.hardware_model),
			network_addresses: Set(serde_json::json!(device.network_addresses)),
			is_online: Set(device.is_online),
			last_seen_at: Set(device.last_seen_at),
			capabilities: Set(device.capabilities),
			created_at: Set(device.created_at),
			sync_enabled: Set(device.sync_enabled),
			last_sync_at: Set(device.last_sync_at),
			updated_at: Set(device.updated_at),
		}
	}
}

impl TryFrom<entities::device::Model> for Device {
	type Error = serde_json::Error;

	fn try_from(model: entities::device::Model) -> Result<Self, Self::Error> {
		let network_addresses: Vec<String> = serde_json::from_value(model.network_addresses)?;

		Ok(Device {
			id: model.uuid,
			name: model.name,
			slug: model.slug,
			os: parse_operating_system(&model.os),
			os_version: model.os_version,
			hardware_model: model.hardware_model,
			network_addresses,
			capabilities: model.capabilities,
			is_online: model.is_online,
			last_seen_at: model.last_seen_at,
			sync_enabled: model.sync_enabled,
			last_sync_at: model.last_sync_at,
			created_at: model.created_at,
			updated_at: model.updated_at,
		})
	}
}

/// Parse OS string to enum
fn parse_operating_system(os_str: &str) -> OperatingSystem {
	match os_str {
		"macOS" => OperatingSystem::MacOS,
		"Windows" => OperatingSystem::Windows,
		"Linux" => OperatingSystem::Linux,
		"iOS" => OperatingSystem::IOs,
		"Android" => OperatingSystem::Android,
		_ => OperatingSystem::Other,
	}
}
