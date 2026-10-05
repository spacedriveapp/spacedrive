use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::ops::libraries::list::output::LibraryInfo;
use crate::ops::network::status::output::NetworkStatus;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CoreStatus {
	pub version: String,
	pub built_at: String,
	pub library_count: usize,
	pub device_info: DeviceInfo,
	pub libraries: Vec<LibraryInfo>,
	pub services: ServiceStatus,
	pub network: NetworkStatus,
	pub system: SystemInfo,
	/// Replica fetches from paired devices: the pause switch, the cap and
	/// every transfer in flight. Defaulted so a `--device` status query
	/// against an older daemon still deserializes.
	#[serde(default)]
	pub replication: crate::service::mounts::replication::ReplicationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceInfo {
	pub id: Uuid,
	pub name: String,
	pub slug: String,
	pub os: String,
	pub hardware_model: Option<String>,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ServiceStatus {
	pub location_watcher: ServiceState,
	pub networking: ServiceState,
	pub volume_monitor: ServiceState,
	pub file_sharing: ServiceState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ServiceState {
	pub running: bool,
	pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairedDeviceInfo {
	pub id: Uuid,
	pub name: String,
	pub os: String,
	pub is_online: bool,
	pub last_seen: DateTime<Utc>,
	pub paired_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SystemInfo {
	pub uptime: Option<u64>, // seconds
	pub data_directory: String,
	pub instance_name: Option<String>,
	pub current_library: Option<String>,
}

#[cfg(test)]
mod tests {
	use super::CoreStatus;

	/// The `core.status` payload as daemons before #3109 sent it, without
	/// the `replication` block. A newer CLI asking such a daemon over
	/// `--device` must still read it.
	#[test]
	fn status_without_replication_block_deserializes() {
		let json = serde_json::json!({
			"version": "2.0.0-alpha.2",
			"built_at": "2026-10-04T04:34:26Z",
			"library_count": 1,
			"device_info": {
				"id": "a688cd39-b65a-4392-9767-7a08aa8fd68f",
				"name": "Old Laptop",
				"slug": "old-laptop",
				"os": "macos",
				"hardware_model": null,
				"created_at": "2026-09-01T00:00:00Z"
			},
			"libraries": [],
			"services": {
				"location_watcher": { "running": true, "details": null },
				"networking": { "running": true, "details": null },
				"volume_monitor": { "running": true, "details": null },
				"file_sharing": { "running": true, "details": null }
			},
			"network": {
				"running": true,
				"node_id": "abc",
				"addresses": [],
				"paired_devices": 1,
				"connected_devices": 1,
				"version": "2.0.0-alpha.2",
				"relay_url": null
			},
			"system": {
				"uptime": null,
				"data_directory": "default",
				"instance_name": null,
				"current_library": null
			}
		});

		let status: CoreStatus = serde_json::from_value(json).unwrap();
		assert!(!status.replication.paused);
		assert_eq!(status.replication.max_bytes_per_sec, 0);
		assert!(status.replication.transfers.is_empty());
	}
}
