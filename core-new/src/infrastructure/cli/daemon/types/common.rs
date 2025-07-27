//! Common types shared between commands and responses

use crate::volume::{Volume, VolumeFingerprint};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct LibraryInfo {
	pub id: Uuid,
	pub name: String,
	pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocationInfo {
	pub id: Uuid,
	pub name: String,
	pub path: PathBuf,
	pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobInfo {
	pub id: Uuid,
	pub name: String,
	pub status: String,
	pub progress: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectedDeviceInfo {
	pub device_id: Uuid,
	pub device_name: String,
	pub device_type: String,
	pub os_version: String,
	pub app_version: String,
	pub peer_id: String,
	pub status: String,
	pub connection_active: bool,
	pub last_seen: String,
	pub connected_at: Option<String>,
	pub bytes_sent: u64,
	pub bytes_received: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairingRequestInfo {
	pub request_id: Uuid,
	pub device_id: Uuid,
	pub device_name: String,
	pub received_at: String,
}

/// Daemon instance information
#[derive(Debug)]
pub struct DaemonInstance {
	pub name: Option<String>, // None for default instance
	pub socket_path: PathBuf,
	pub is_running: bool,
}

impl DaemonInstance {
	/// Get instance display name ("default" for None, or the actual name)
	pub fn display_name(&self) -> &str {
		self.name.as_deref().unwrap_or("default")
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowseEntry {
	pub name: String,
	pub path: std::path::PathBuf,
	pub is_dir: bool,
	pub size: Option<u64>,
	pub modified: Option<String>,
	pub file_type: Option<String>,
}

/// Represents a volume in the list with tracking information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VolumeListItem {
	/// The volume data (either from live detection or reconstructed from tracking info)
	pub volume: Volume,
	/// Whether this volume is tracked in the current library
	pub is_tracked: bool,
	/// Custom name assigned when tracking (if any)
	pub tracked_name: Option<String>,
	/// Whether the volume is currently online/connected
	pub is_online: bool,
	/// When the volume was last seen (for offline volumes)
	pub last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}
