//! Unified device model - no more node/device/instance confusion
//!
//! A Device represents a machine running Spacedrive. This unifies the old
//! concepts of Node, Device, and Instance into one clear model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A device running Spacedrive
///
/// This is the canonical device type used throughout the application.
/// It represents both database-registered devices and network-paired devices.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

	// --- Phase 1: Core Hardware Specifications ---
	/// CPU model name (e.g., "Apple M3 Max", "Intel Core i9-13900K")
	pub cpu_model: Option<String>,

	/// CPU architecture (e.g., "arm64", "x86_64")
	pub cpu_architecture: Option<String>,

	/// Number of physical CPU cores
	pub cpu_cores_physical: Option<u32>,

	/// Number of logical CPU cores (with hyperthreading)
	pub cpu_cores_logical: Option<u32>,

	/// CPU base frequency in MHz
	pub cpu_frequency_mhz: Option<i64>,

	/// Total system memory in bytes
	pub memory_total_bytes: Option<i64>,

	/// Device form factor
	pub form_factor: Option<DeviceFormFactor>,

	/// Device manufacturer (e.g., "Apple", "Dell", "Lenovo")
	pub manufacturer: Option<String>,

	// --- Phase 2: Extended Hardware ---
	/// GPU model names (can have multiple GPUs)
	pub gpu_models: Option<Vec<String>>,

	/// Boot disk type (e.g., "SSD", "HDD", "NVMe")
	pub boot_disk_type: Option<String>,

	/// Boot disk capacity in bytes
	pub boot_disk_capacity_bytes: Option<i64>,

	/// Total swap space in bytes
	pub swap_total_bytes: Option<i64>,

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

	/// When this device was first added
	pub created_at: DateTime<Utc>,

	/// When this device info was last updated
	pub updated_at: DateTime<Utc>,

	// --- Ephemeral fields (computed at query time, not persisted) ---
	/// Whether this is the current device (computed)
	#[serde(default)]
	pub is_current: bool,

	/// Whether this device is paired via network but not in library DB
	#[serde(default)]
	pub is_paired: bool,

	/// Whether this device is currently connected via network
	#[serde(default)]
	pub is_connected: bool,

	/// Connection method when connected (Direct, Relay, or Mixed)
	#[serde(default)]
	#[specta(optional)]
	pub connection_method: Option<ConnectionMethod>,
}

/// Network connection method for a device
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Type)]
pub enum ConnectionMethod {
	/// Direct connection on local network (mDNS/same subnet)
	/// Fastest option - wire speed, no internet required
	LocalNetwork,
	/// Direct UDP connection over internet (NAT traversal)
	/// Fast, but requires internet. Uses no relay bandwidth.
	DirectInternet,
	/// Connection proxied through relay server
	/// Reliable fallback. Relay hosts the bandwidth.
	RelayProxy,
}

impl ConnectionMethod {
	/// Convert from Iroh's ConnectionType
	///
	/// For Mixed connections (UDP + relay simultaneously), we report the
	/// Direct path since that's what Iroh is attempting to use primarily.
	pub fn from_iroh_connection_type(conn_type: iroh::endpoint::ConnectionType) -> Option<Self> {
		use iroh::endpoint::ConnectionType;
		match conn_type {
			ConnectionType::Direct(addr) => {
				if is_local_address(&addr) {
					Some(Self::LocalNetwork)
				} else {
					Some(Self::DirectInternet)
				}
			}
			ConnectionType::Relay(_) => Some(Self::RelayProxy),
			// Mixed means both UDP and relay are active, but UDP is preferred
			// Report the UDP path since that's what Iroh will use when confirmed
			ConnectionType::Mixed(addr, _relay) => {
				if is_local_address(&addr) {
					Some(Self::LocalNetwork)
				} else {
					Some(Self::DirectInternet)
				}
			}
			ConnectionType::None => None,
		}
	}
}

/// Check if a socket address is a local/private network address
fn is_local_address(addr: &std::net::SocketAddr) -> bool {
	match addr.ip() {
		std::net::IpAddr::V4(ipv4) => {
			ipv4.is_private() // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
				|| ipv4.is_loopback() // 127.0.0.0/8
				|| ipv4.is_link_local() // 169.254.0.0/16
		}
		std::net::IpAddr::V6(ipv6) => {
			ipv6.is_loopback() // ::1
				|| ipv6.is_unicast_link_local() // fe80::/10
				|| is_ipv6_unique_local(&ipv6) // fc00::/7
		}
	}
}

/// Check if IPv6 address is in unique local range (fc00::/7)
fn is_ipv6_unique_local(ipv6: &std::net::Ipv6Addr) -> bool {
	matches!(ipv6.segments()[0] & 0xfe00, 0xfc00)
}

/// Operating system types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Type)]
pub enum OperatingSystem {
	MacOS,
	Windows,
	Linux,
	IOs,
	Android,
	Other,
}

/// Device form factor types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Type)]
pub enum DeviceFormFactor {
	Desktop,
	Laptop,
	Mobile,
	Tablet,
	Server,
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
		let system_info = detect_system_info();
		Self {
			id: Uuid::new_v4(),
			name,
			slug,
			os: detect_operating_system(),
			os_version: detect_os_version(),
			hardware_model: detect_hardware_model(),
			// Phase 1 fields
			cpu_model: system_info.cpu_model,
			cpu_architecture: system_info.cpu_architecture,
			cpu_cores_physical: system_info.cpu_cores_physical,
			cpu_cores_logical: system_info.cpu_cores_logical,
			cpu_frequency_mhz: system_info.cpu_frequency_mhz,
			memory_total_bytes: system_info.memory_total_bytes,
			form_factor: system_info.form_factor,
			manufacturer: system_info.manufacturer,
			// Phase 2 fields
			gpu_models: system_info.gpu_models,
			boot_disk_type: system_info.boot_disk_type,
			boot_disk_capacity_bytes: system_info.boot_disk_capacity_bytes,
			swap_total_bytes: system_info.swap_total_bytes,
			network_addresses: Vec::new(),
			capabilities: serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			}),
			is_online: true,
			last_seen_at: now,
			sync_enabled: true,
			created_at: now,
			updated_at: now,
			// Ephemeral fields
			is_current: false,
			is_paired: false,
			is_connected: false,
			connection_method: None,
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
	pub fn is_current_device(&self, current_device_id: Uuid) -> bool {
		self.id == current_device_id
	}

	/// Create a Device from network DeviceInfo
	///
	/// This converts the network layer's DeviceInfo into the canonical Device model.
	/// Used for paired devices that may not be registered in the library database.
	pub fn from_network_info(
		info: &crate::service::network::device::DeviceInfo,
		is_connected: bool,
		connection_method: Option<ConnectionMethod>,
	) -> Self {
		use crate::service::network::device::DeviceType;

		// Map DeviceType to OperatingSystem (best effort)
		let os = match &info.device_type {
			DeviceType::Desktop | DeviceType::Laptop => {
				// Try to infer from os_version string
				let os_lower = info.os_version.to_lowercase();
				if os_lower.contains("mac") || os_lower.contains("darwin") {
					OperatingSystem::MacOS
				} else if os_lower.contains("windows") {
					OperatingSystem::Windows
				} else if os_lower.contains("linux") {
					OperatingSystem::Linux
				} else {
					OperatingSystem::Other
				}
			}
			DeviceType::Mobile => {
				let os_lower = info.os_version.to_lowercase();
				if os_lower.contains("ios") || os_lower.contains("iphone") {
					OperatingSystem::IOs
				} else if os_lower.contains("android") {
					OperatingSystem::Android
				} else {
					OperatingSystem::Other
				}
			}
			DeviceType::Server => OperatingSystem::Linux,
			DeviceType::Other(_) => OperatingSystem::Other,
		};

		Self {
			id: info.device_id,
			name: info.device_name.clone(),
			slug: info.device_slug.clone(),
			os,
			os_version: Some(info.os_version.clone()),
			hardware_model: None,
			// Phase 1 fields - not available from network info
			cpu_model: None,
			cpu_architecture: None,
			cpu_cores_physical: None,
			cpu_cores_logical: None,
			cpu_frequency_mhz: None,
			memory_total_bytes: None,
			form_factor: None,
			manufacturer: None,
			// Phase 2 fields
			gpu_models: None,
			boot_disk_type: None,
			boot_disk_capacity_bytes: None,
			swap_total_bytes: None,
			network_addresses: Vec::new(),
			capabilities: serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			}),
			is_online: is_connected,
			last_seen_at: info.last_seen,
			sync_enabled: true,
			created_at: info.last_seen,
			updated_at: info.last_seen,
			// Ephemeral fields
			is_current: false,
			is_paired: true,
			is_connected,
			connection_method,
		}
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

/// System information collected at device initialization
struct SystemInfo {
	cpu_model: Option<String>,
	cpu_architecture: Option<String>,
	cpu_cores_physical: Option<u32>,
	cpu_cores_logical: Option<u32>,
	cpu_frequency_mhz: Option<i64>,
	memory_total_bytes: Option<i64>,
	swap_total_bytes: Option<i64>,
	form_factor: Option<DeviceFormFactor>,
	manufacturer: Option<String>,
	gpu_models: Option<Vec<String>>,
	boot_disk_type: Option<String>,
	boot_disk_capacity_bytes: Option<i64>,
}

/// System information for DeviceConfig (uses String for form_factor instead of enum)
pub struct SystemInfoConfig {
	pub cpu_model: Option<String>,
	pub cpu_architecture: Option<String>,
	pub cpu_cores_physical: Option<u32>,
	pub cpu_cores_logical: Option<u32>,
	pub cpu_frequency_mhz: Option<i64>,
	pub memory_total_bytes: Option<i64>,
	pub swap_total_bytes: Option<i64>,
	pub form_factor: Option<String>,
	pub manufacturer: Option<String>,
	pub gpu_models: Option<Vec<String>>,
	pub boot_disk_type: Option<String>,
	pub boot_disk_capacity_bytes: Option<i64>,
}

/// Detect comprehensive system information using sysinfo
fn detect_system_info() -> SystemInfo {
	// Skip sysinfo on mobile platforms - it was causing crashes on Android
	// (likely due to SELinux denying access to /proc files) and is unreliable on iOS.
	// TODO: Implement with native APIs (android.os.Build, UIDevice) for richer device info.
	#[cfg(any(target_os = "android", target_os = "ios"))]
	{
		return SystemInfo {
			cpu_model: None,
			cpu_architecture: Some(std::env::consts::ARCH.to_string()),
			cpu_cores_physical: None,
			cpu_cores_logical: None,
			cpu_frequency_mhz: None,
			memory_total_bytes: None,
			swap_total_bytes: None,
			form_factor: None,
			manufacturer: None,
			gpu_models: None,
			boot_disk_type: None,
			boot_disk_capacity_bytes: None,
		};
	}

	#[cfg(not(any(target_os = "android", target_os = "ios")))]
	{
		use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

		let mut sys = System::new_with_specifics(
			RefreshKind::new()
				.with_cpu(CpuRefreshKind::everything())
				.with_memory(MemoryRefreshKind::everything()),
		);

		// Refresh to get accurate data
		sys.refresh_cpu_all();
		sys.refresh_memory();

		// CPU information
		let cpu_model = sys
			.cpus()
			.first()
			.map(|cpu| cpu.brand().to_string())
			.filter(|s| !s.is_empty());

		let cpu_architecture = Some(std::env::consts::ARCH.to_string());

		let cpu_cores_physical = sys.physical_core_count().map(|c| c as u32);

		let cpu_cores_logical = Some(sys.cpus().len() as u32);

		let cpu_frequency_mhz = sys
			.cpus()
			.first()
			.map(|cpu| cpu.frequency() as i64)
			.filter(|&freq| freq > 0);

		// Memory information
		let memory_total_bytes = {
			let total = sys.total_memory();
			if total > 0 {
				Some(total as i64)
			} else {
				None
			}
		};

		let swap_total_bytes = {
			let total = sys.total_swap();
			if total > 0 {
				Some(total as i64)
			} else {
				None
			}
		};

		// Form factor detection
		let form_factor = detect_form_factor();

		// Manufacturer detection
		let manufacturer = detect_manufacturer();

		// Phase 2: GPU and storage detection
		let gpu_models = detect_gpu_models();
		let boot_disk_type = detect_boot_disk_type();
		let boot_disk_capacity_bytes = detect_boot_disk_capacity();

		SystemInfo {
			cpu_model,
			cpu_architecture,
			cpu_cores_physical,
			cpu_cores_logical,
			cpu_frequency_mhz,
			memory_total_bytes,
			swap_total_bytes,
			form_factor,
			manufacturer,
			gpu_models,
			boot_disk_type,
			boot_disk_capacity_bytes,
		}
	} // end #[cfg(not(any(target_os = "android", target_os = "ios")))]
}

/// Public function to detect system info for DeviceConfig
pub fn detect_system_info_for_config() -> SystemInfoConfig {
	let info = detect_system_info();
	SystemInfoConfig {
		cpu_model: info.cpu_model,
		cpu_architecture: info.cpu_architecture,
		cpu_cores_physical: info.cpu_cores_physical,
		cpu_cores_logical: info.cpu_cores_logical,
		cpu_frequency_mhz: info.cpu_frequency_mhz,
		memory_total_bytes: info.memory_total_bytes,
		swap_total_bytes: info.swap_total_bytes,
		form_factor: info.form_factor.map(|f| f.to_string()),
		manufacturer: info.manufacturer,
		gpu_models: info.gpu_models,
		boot_disk_type: info.boot_disk_type,
		boot_disk_capacity_bytes: info.boot_disk_capacity_bytes,
	}
}

/// Detect device form factor
fn detect_form_factor() -> Option<DeviceFormFactor> {
	#[cfg(target_os = "macos")]
	{
		use std::process::Command;

		// Use system_profiler to get the actual model name
		if let Ok(output) = Command::new("system_profiler")
			.args(["SPHardwareDataType", "-json"])
			.output()
		{
			if output.status.success() {
				if let Ok(json_str) = String::from_utf8(output.stdout) {
					if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
						if let Some(model_name) =
							json["SPHardwareDataType"][0]["machine_model"].as_str()
						{
							let model_lower = model_name.to_lowercase();
							if model_lower.contains("macbook") {
								return Some(DeviceFormFactor::Laptop);
							} else if model_lower.contains("imac")
								|| model_lower.contains("mac pro")
								|| model_lower.contains("mac studio")
								|| model_lower.contains("mac mini")
							{
								return Some(DeviceFormFactor::Desktop);
							}
						}
					}
				}
			}
		}

		// Fallback: check hardware model identifier pattern
		// MacBookPro, MacBookAir identifiers contain "MacBook" in the product name lookup
		if let Some(model) = detect_hardware_model() {
			// Mac laptop identifiers are typically MacBookPro##,# or MacBookAir##,#
			if model.starts_with("MacBook") {
				return Some(DeviceFormFactor::Laptop);
			}
		}
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		// Check chassis type using wmic
		if let Ok(output) = Command::new("wmic")
			.args(["computersystem", "get", "PCSystemType"])
			.output()
		{
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				// PCSystemType values: 1=Desktop, 2=Mobile/Laptop, 3=Workstation, 4=Enterprise Server, etc.
				if let Some(line) = stdout.lines().nth(1) {
					if let Ok(system_type) = line.trim().parse::<u32>() {
						return match system_type {
							1 => Some(DeviceFormFactor::Desktop),
							2 => Some(DeviceFormFactor::Laptop),
							4 | 5 | 6 => Some(DeviceFormFactor::Server),
							8 => Some(DeviceFormFactor::Tablet),
							_ => Some(DeviceFormFactor::Other),
						};
					}
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::fs;

		// Check chassis type from DMI
		if let Ok(chassis) = fs::read_to_string("/sys/devices/virtual/dmi/id/chassis_type") {
			let chassis = chassis.trim();
			// Chassis type codes: https://www.dmtf.org/standards/smbios
			return match chassis {
				"3" | "4" | "5" | "6" | "7" | "15" | "16" => Some(DeviceFormFactor::Desktop),
				"8" | "9" | "10" | "11" | "14" | "30" | "31" => Some(DeviceFormFactor::Laptop),
				"17" | "23" => Some(DeviceFormFactor::Server),
				"30" => Some(DeviceFormFactor::Tablet),
				_ => Some(DeviceFormFactor::Other),
			};
		}
	}

	#[cfg(target_os = "ios")]
	{
		return Some(DeviceFormFactor::Mobile);
	}

	#[cfg(target_os = "android")]
	{
		return Some(DeviceFormFactor::Mobile);
	}

	None
}

/// Detect device manufacturer
fn detect_manufacturer() -> Option<String> {
	#[cfg(target_os = "macos")]
	{
		// All macOS devices are made by Apple
		return Some("Apple".to_string());
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		if let Ok(output) = Command::new("wmic")
			.args(["computersystem", "get", "manufacturer"])
			.output()
		{
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				if let Some(manufacturer) = stdout.lines().nth(1) {
					let manufacturer = manufacturer.trim().to_string();
					if !manufacturer.is_empty() {
						return Some(manufacturer);
					}
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::fs;

		if let Ok(manufacturer) = fs::read_to_string("/sys/devices/virtual/dmi/id/sys_vendor") {
			let manufacturer = manufacturer.trim().to_string();
			if !manufacturer.is_empty() && manufacturer != "System manufacturer" {
				return Some(manufacturer);
			}
		}
	}

	#[cfg(target_os = "ios")]
	{
		return Some("Apple".to_string());
	}

	#[cfg(target_os = "android")]
	{
		// Android manufacturer detection would require JNI calls
		// This would be platform-specific implementation
		return None;
	}

	None
}

/// Detect GPU models
fn detect_gpu_models() -> Option<Vec<String>> {
	#[cfg(target_os = "macos")]
	{
		use std::process::Command;

		if let Ok(output) = Command::new("system_profiler")
			.args(["SPDisplaysDataType", "-json"])
			.output()
		{
			if output.status.success() {
				if let Ok(json_str) = String::from_utf8(output.stdout) {
					if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
						if let Some(displays) = json["SPDisplaysDataType"].as_array() {
							let mut gpus = Vec::new();
							for display in displays {
								if let Some(name) = display["sppci_model"].as_str() {
									if !name.is_empty() && !gpus.contains(&name.to_string()) {
										gpus.push(name.to_string());
									}
								}
							}
							if !gpus.is_empty() {
								return Some(gpus);
							}
						}
					}
				}
			}
		}
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		if let Ok(output) = Command::new("wmic")
			.args(["path", "win32_VideoController", "get", "name"])
			.output()
		{
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				let mut gpus = Vec::new();
				for line in stdout.lines().skip(1) {
					let gpu = line.trim().to_string();
					if !gpu.is_empty() && gpu != "Name" {
						gpus.push(gpu);
					}
				}
				if !gpus.is_empty() {
					return Some(gpus);
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::process::Command;

		// Try lspci first
		if let Ok(output) = Command::new("lspci").output() {
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				let mut gpus = Vec::new();
				for line in stdout.lines() {
					if line.contains("VGA compatible controller:")
						|| line.contains("3D controller:")
					{
						if let Some(gpu_name) = line.split(':').nth(2) {
							let gpu = gpu_name.trim().to_string();
							if !gpu.is_empty() {
								gpus.push(gpu);
							}
						}
					}
				}
				if !gpus.is_empty() {
					return Some(gpus);
				}
			}
		}
	}

	None
}

/// Detect boot disk type (SSD/HDD/NVMe)
fn detect_boot_disk_type() -> Option<String> {
	#[cfg(target_os = "macos")]
	{
		use std::process::Command;

		if let Ok(output) = Command::new("diskutil").args(["info", "/"]).output() {
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				for line in stdout.lines() {
					if line.contains("Solid State:") {
						if line.contains("Yes") {
							// Check if it's NVMe
							if stdout.contains("NVMe") || stdout.contains("Apple") {
								return Some("NVMe".to_string());
							}
							return Some("SSD".to_string());
						} else {
							return Some("HDD".to_string());
						}
					}
				}
			}
		}
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		// Get the boot drive letter (usually C:)
		if let Ok(output) = Command::new("wmic")
			.args(["diskdrive", "get", "MediaType,DeviceID"])
			.output()
		{
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				for line in stdout.lines().skip(1) {
					if line.contains("Fixed hard disk") {
						if line.contains("SSD") || line.contains("Solid State") {
							return Some("SSD".to_string());
						}
					}
				}
			}
		}

		// Fallback: check if it's SSD via optimization settings
		if let Ok(output) = Command::new("powershell")
			.args(["-Command", "Get-PhysicalDisk | Select MediaType"])
			.output()
		{
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				if stdout.contains("SSD") {
					return Some("SSD".to_string());
				} else if stdout.contains("HDD") {
					return Some("HDD".to_string());
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::fs;

		// Find the boot device
		if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
			for line in mounts.lines() {
				let parts: Vec<&str> = line.split_whitespace().collect();
				if parts.len() >= 2 && parts[1] == "/" {
					if let Some(device) = parts.first() {
						// Extract device name (e.g., /dev/nvme0n1p1 -> nvme0n1)
						let dev_name = device
							.trim_start_matches("/dev/")
							.chars()
							.take_while(|c| c.is_alphabetic() || c.is_numeric())
							.collect::<String>();

						// Check if it's NVMe
						if dev_name.starts_with("nvme") {
							return Some("NVMe".to_string());
						}

						// Check rotational flag for SATA/SAS drives
						let rotational_path = format!("/sys/block/{}/queue/rotational", dev_name);
						if let Ok(rotational) = fs::read_to_string(rotational_path) {
							if rotational.trim() == "0" {
								return Some("SSD".to_string());
							} else {
								return Some("HDD".to_string());
							}
						}
					}
					break;
				}
			}
		}
	}

	None
}

/// Detect boot disk capacity
fn detect_boot_disk_capacity() -> Option<i64> {
	#[cfg(target_os = "macos")]
	{
		use std::process::Command;

		if let Ok(output) = Command::new("diskutil").args(["info", "/"]).output() {
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				for line in stdout.lines() {
					if line.contains("Disk Size:") {
						// Parse something like "Disk Size: 494.4 GB (494384795648 Bytes)"
						if let Some(bytes_part) = line.split('(').nth(1) {
							if let Some(bytes_str) = bytes_part.split_whitespace().next() {
								if let Ok(bytes) = bytes_str.parse::<i64>() {
									return Some(bytes);
								}
							}
						}
					} else if line.contains("Total Size:") {
						// Alternative format
						if let Some(bytes_part) = line.split('(').nth(1) {
							if let Some(bytes_str) = bytes_part.split_whitespace().next() {
								if let Ok(bytes) = bytes_str.parse::<i64>() {
									return Some(bytes);
								}
							}
						}
					}
				}
			}
		}
	}

	#[cfg(target_os = "windows")]
	{
		use std::process::Command;

		if let Ok(output) = Command::new("wmic")
			.args(["diskdrive", "get", "Size"])
			.output()
		{
			if output.status.success() {
				let stdout = String::from_utf8_lossy(&output.stdout);
				for line in stdout.lines().skip(1) {
					if let Ok(size) = line.trim().parse::<i64>() {
						return Some(size);
					}
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	{
		use std::fs;

		// Find the boot device and get its size
		if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
			for line in mounts.lines() {
				let parts: Vec<&str> = line.split_whitespace().collect();
				if parts.len() >= 2 && parts[1] == "/" {
					if let Some(device) = parts.first() {
						// Extract base device name (e.g., /dev/nvme0n1p1 -> nvme0n1)
						let dev_name = device
							.trim_start_matches("/dev/")
							.chars()
							.take_while(|c| c.is_alphabetic() || c.is_numeric())
							.collect::<String>();

						// Read size from sysfs (in 512-byte sectors)
						let size_path = format!("/sys/block/{}/size", dev_name);
						if let Ok(size_str) = fs::read_to_string(size_path) {
							if let Ok(sectors) = size_str.trim().parse::<i64>() {
								return Some(sectors * 512);
							}
						}
					}
					break;
				}
			}
		}
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

impl std::fmt::Display for DeviceFormFactor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			DeviceFormFactor::Desktop => write!(f, "Desktop"),
			DeviceFormFactor::Laptop => write!(f, "Laptop"),
			DeviceFormFactor::Mobile => write!(f, "Mobile"),
			DeviceFormFactor::Tablet => write!(f, "Tablet"),
			DeviceFormFactor::Server => write!(f, "Server"),
			DeviceFormFactor::Other => write!(f, "Other"),
		}
	}
}

/// Parse form factor string to enum
pub fn parse_device_form_factor_from_string(form_factor_str: &str) -> DeviceFormFactor {
	match form_factor_str {
		"Desktop" => DeviceFormFactor::Desktop,
		"Laptop" => DeviceFormFactor::Laptop,
		"Mobile" => DeviceFormFactor::Mobile,
		"Tablet" => DeviceFormFactor::Tablet,
		"Server" => DeviceFormFactor::Server,
		_ => DeviceFormFactor::Other,
	}
}

// Internal alias for backwards compatibility
fn parse_device_form_factor(form_factor_str: &str) -> DeviceFormFactor {
	parse_device_form_factor_from_string(form_factor_str)
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
			cpu_model: Set(device.cpu_model),
			cpu_architecture: Set(device.cpu_architecture),
			cpu_cores_physical: Set(device.cpu_cores_physical),
			cpu_cores_logical: Set(device.cpu_cores_logical),
			cpu_frequency_mhz: Set(device.cpu_frequency_mhz),
			memory_total_bytes: Set(device.memory_total_bytes),
			form_factor: Set(device.form_factor.map(|f| f.to_string())),
			manufacturer: Set(device.manufacturer),
			gpu_models: Set(device.gpu_models.map(|g| serde_json::json!(g))),
			boot_disk_type: Set(device.boot_disk_type),
			boot_disk_capacity_bytes: Set(device.boot_disk_capacity_bytes),
			swap_total_bytes: Set(device.swap_total_bytes),
			network_addresses: Set(serde_json::json!(device.network_addresses)),
			is_online: Set(device.is_online),
			last_seen_at: Set(device.last_seen_at),
			capabilities: Set(device.capabilities),
			created_at: Set(device.created_at),
			sync_enabled: Set(device.sync_enabled),
			updated_at: Set(device.updated_at),
		}
	}
}

impl TryFrom<entities::device::Model> for Device {
	type Error = serde_json::Error;

	fn try_from(model: entities::device::Model) -> Result<Self, Self::Error> {
		let network_addresses: Vec<String> = serde_json::from_value(model.network_addresses)?;

		let gpu_models: Option<Vec<String>> = model
			.gpu_models
			.and_then(|v| serde_json::from_value(v).ok());

		Ok(Device {
			id: model.uuid,
			name: model.name,
			slug: model.slug,
			os: parse_operating_system(&model.os),
			os_version: model.os_version,
			hardware_model: model.hardware_model,
			cpu_model: model.cpu_model,
			cpu_architecture: model.cpu_architecture,
			cpu_cores_physical: model.cpu_cores_physical,
			cpu_cores_logical: model.cpu_cores_logical,
			cpu_frequency_mhz: model.cpu_frequency_mhz,
			memory_total_bytes: model.memory_total_bytes,
			form_factor: model.form_factor.as_deref().map(parse_device_form_factor),
			manufacturer: model.manufacturer,
			gpu_models,
			boot_disk_type: model.boot_disk_type,
			boot_disk_capacity_bytes: model.boot_disk_capacity_bytes,
			swap_total_bytes: model.swap_total_bytes,
			network_addresses,
			capabilities: model.capabilities,
			is_online: model.is_online,
			last_seen_at: model.last_seen_at,
			sync_enabled: model.sync_enabled,
			created_at: model.created_at,
			updated_at: model.updated_at,
			// Ephemeral fields - set by caller based on context
			is_current: false,
			is_paired: false,
			is_connected: false,
			connection_method: None, // Populated by caller when connection info available
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

// Implement Identifiable for normalized cache support
impl super::resource::Identifiable for Device {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"device"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::device;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let device_models = device::Entity::find()
			.filter(device::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		let mut results = Vec::new();
		for model in device_models {
			match Device::try_from(model) {
				Ok(device) => results.push(device),
				Err(e) => {
					tracing::warn!("Failed to convert device model: {}", e);
				}
			}
		}

		Ok(results)
	}
}

// Register Device as a simple resource
crate::register_resource!(Device);
