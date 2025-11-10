//! Output types for library discovery

use crate::library::config::LibraryStatistics;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Information about a library discovered on a remote device
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLibraryInfo {
	/// Library ID
	pub id: Uuid,

	/// Library name
	pub name: String,

	/// Library description (if any)
	pub description: Option<String>,

	/// When the library was created
	pub created_at: DateTime<Utc>,

	/// Statistics about the library
	pub statistics: LibraryStatistics,
}

/// Output from discovering remote libraries
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverRemoteLibrariesOutput {
	/// Remote device ID that was queried
	pub device_id: Uuid,

	/// Remote device name
	pub device_name: String,

	/// List of libraries available on the remote device
	pub libraries: Vec<RemoteLibraryInfo>,

	/// Whether the device is currently online
	pub is_online: bool,
}
