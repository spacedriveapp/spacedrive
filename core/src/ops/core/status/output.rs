use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreStatus {
	pub version: String,
	pub built_at: String,
	pub library_count: usize,
	pub device_info: DeviceInfo,
	pub libraries: Vec<LibraryInfo>,
	pub services: ServiceStatus,
	pub network: NetworkStatus,
	pub system: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
	pub id: Uuid,
	pub name: String,
	pub os: String,
	pub hardware_model: Option<String>,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
	pub id: Uuid,
	pub name: String,
	pub is_active: bool,
	pub location_count: usize,
	pub total_entries: Option<u64>,
	pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
	pub location_watcher: ServiceState,
	pub networking: ServiceState,
	pub volume_monitor: ServiceState,
	pub file_sharing: ServiceState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
	pub running: bool,
	pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
	pub enabled: bool,
	pub node_id: Option<String>,
	pub paired_devices: Vec<PairedDeviceInfo>,
	pub active_connections: usize,
	pub discovery_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDeviceInfo {
	pub id: Uuid,
	pub name: String,
	pub os: String,
	pub is_online: bool,
	pub last_seen: DateTime<Utc>,
	pub paired_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
	pub uptime: Option<u64>, // seconds
	pub data_directory: String,
	pub instance_name: Option<String>,
	pub current_library: Option<String>,
}
