//! Volume list output

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeItem {
	pub id: Uuid,
	pub name: String,
	pub fingerprint: VolumeFingerprint,
	pub volume_type: String,
	pub mount_point: Option<String>,
	/// Whether this volume is currently tracked in the library
	pub is_tracked: bool,
	/// Whether this volume is currently online/mounted
	pub is_online: bool,
	/// Total capacity in bytes
	pub total_capacity: Option<u64>,
	/// Available capacity in bytes
	pub available_capacity: Option<u64>,
	/// Unique bytes (deduplicated by content_identity)
	pub unique_bytes: Option<u64>,
	/// Filesystem type (APFS, NTFS, ext4, etc.)
	pub file_system: Option<String>,
	/// Disk type (SSD, HDD, etc.)
	pub disk_type: Option<String>,
	/// Read speed in MB/s
	pub read_speed_mbps: Option<u32>,
	/// Write speed in MB/s
	pub write_speed_mbps: Option<u32>,
	/// Device ID that owns this volume
	pub device_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListOutput {
	pub volumes: Vec<VolumeItem>,
}
