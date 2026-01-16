//! Volume list output

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Encryption information for a volume (frontend-friendly subset of VolumeEncryption)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeEncryptionInfo {
	/// Whether encryption is enabled on this volume
	pub enabled: bool,
	/// Type of encryption (FileVault, BitLocker, LUKS, etc.)
	pub encryption_type: String,
	/// Whether the volume is currently unlocked
	pub is_unlocked: bool,
}

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
	/// Encryption status (FileVault, BitLocker, LUKS, etc.)
	/// Only available for currently-mounted volumes on the local device
	pub encryption: Option<VolumeEncryptionInfo>,
	/// Read speed in MB/s
	pub read_speed_mbps: Option<u32>,
	/// Write speed in MB/s
	pub write_speed_mbps: Option<u32>,
	/// Device ID that owns this volume
	pub device_id: Uuid,
	/// Device slug for constructing SdPaths
	pub device_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListOutput {
	pub volumes: Vec<VolumeItem>,
}
