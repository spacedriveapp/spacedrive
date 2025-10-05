//! Output types for paired devices query

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Information about a paired device
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceInfo {
	/// Device ID
	pub id: Uuid,

	/// Device name
	pub name: String,

	/// Device type
	pub device_type: String,

	/// OS version
	pub os_version: String,

	/// App version
	pub app_version: String,

	/// Whether the device is currently connected
	pub is_connected: bool,

	/// When the device was last seen
	pub last_seen: DateTime<Utc>,
}

/// Output from listing paired devices
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListPairedDevicesOutput {
	/// List of paired devices
	pub devices: Vec<PairedDeviceInfo>,

	/// Total number of paired devices
	pub total: usize,

	/// Number of currently connected devices
	pub connected: usize,
}
