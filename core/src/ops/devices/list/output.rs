//! Output types for library devices query

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Device information from the library database
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryDeviceInfo {
	/// Unique device identifier
	pub id: Uuid,

	/// Device name
	pub name: String,

	/// Operating system
	pub os: String,

	/// Operating system version (if available)
	pub os_version: Option<String>,

	/// Hardware model (if available)
	pub hardware_model: Option<String>,

	/// Whether this device is currently online
	pub is_online: bool,

	/// Last time this device was seen
	pub last_seen_at: DateTime<Utc>,

	/// When this device was first registered in the library
	pub created_at: DateTime<Utc>,

	/// When this device info was last updated
	pub updated_at: DateTime<Utc>,

	/// Whether this is the current device
	pub is_current: bool,

	/// Network addresses for P2P connections (if available)
	pub network_addresses: Vec<String>,

	/// Device capabilities (if available)
	pub capabilities: Option<serde_json::Value>,

	/// Sync leadership status per library (if available)
	pub sync_leadership: Option<serde_json::Value>,
}
