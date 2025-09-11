//! Output types for network devices

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoLite {
	pub id: Uuid,
	pub name: String,
	pub os_version: String,
	pub app_version: String,
	pub is_connected: bool,
	pub last_seen: DateTime<Utc>,
}

