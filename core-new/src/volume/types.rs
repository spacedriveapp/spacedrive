//! Volume type definitions

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

/// Classification of volume types for UX and auto-tracking decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeType {
	/// Primary system drive containing OS and user data
	/// Examples: C:\ on Windows, / on Linux, Macintosh HD on macOS
	Primary,

	/// Dedicated user data volumes (separate from OS)
	/// Examples: /System/Volumes/Data on macOS, separate /home on Linux
	UserData,

	/// External or removable storage devices
	/// Examples: USB drives, external HDDs, /Volumes/* on macOS
	External,

	/// Secondary internal storage (additional drives/partitions)
	/// Examples: D:, E: drives on Windows, additional mounted drives
	Secondary,

	/// System/OS internal volumes (hidden from normal view)
	/// Examples: /System/Volumes/* on macOS, Recovery partitions
	System,

	/// Network attached storage
	/// Examples: SMB mounts, NFS, cloud storage
	Network,

	/// Unknown or unclassified volumes
	Unknown,
}

impl VolumeType {
	/// Should this volume type be auto-tracked by default?
	pub fn auto_track_by_default(&self) -> bool {
		match self {
			VolumeType::Primary
			| VolumeType::UserData
			| VolumeType::External
			| VolumeType::Secondary
			| VolumeType::Network => true,
			VolumeType::System | VolumeType::Unknown => false,
		}
	}

	/// Should this volume be shown in the default UI view?
	pub fn show_by_default(&self) -> bool {
		!matches!(self, VolumeType::System | VolumeType::Unknown)
	}

	/// User-friendly display name for the volume type
	pub fn display_name(&self) -> &'static str {
		match self {
			VolumeType::Primary => "Primary Drive",
			VolumeType::UserData => "User Data",
			VolumeType::External => "External Drive",
			VolumeType::Secondary => "Secondary Drive",
			VolumeType::System => "System Volume",
			VolumeType::Network => "Network Drive",
			VolumeType::Unknown => "Unknown",
		}
	}

	/// Icon/indicator for CLI display
	pub fn icon(&self) -> &'static str {
		match self {
			VolumeType::Primary => "[PRI]",
			VolumeType::UserData => "[USR]",
			VolumeType::External => "[EXT]",
			VolumeType::Secondary => "[SEC]",
			VolumeType::System => "[SYS]",
			VolumeType::Network => "[NET]",
			VolumeType::Unknown => "[UNK]",
		}
	}
}

/// A fingerprint of a volume, used to identify it uniquely across sessions
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct VolumeFingerprint(pub String);

impl VolumeFingerprint {
	/// Create a new volume fingerprint from volume properties
	pub fn new(device_id: uuid::Uuid, mount_point: &PathBuf, name: &str) -> Self {
		let mut hasher = blake3::Hasher::new();
		hasher.update(device_id.as_bytes());
		hasher.update(mount_point.to_string_lossy().as_bytes());
		hasher.update(name.as_bytes());
		hasher.update(&(mount_point.to_string_lossy().len() as u64).to_be_bytes());
		hasher.update(&(name.len() as u64).to_be_bytes());

		Self(hasher.finalize().to_hex().to_string())
	}

	/// Create fingerprint from hex string
	pub fn from_hex(hex: impl Into<String>) -> Self {
		Self(hex.into())
	}

	/// Create fingerprint from string (alias for from_hex)
	pub fn from_string(s: &str) -> Result<Self, crate::volume::VolumeError> {
		Ok(Self(s.to_string()))
	}
}

impl fmt::Display for VolumeFingerprint {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Events emitted by the Volume Manager when volume state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolumeEvent {
	/// Emitted when a new volume is discovered
	VolumeAdded(Volume),
	/// Emitted when a volume is removed/unmounted
	VolumeRemoved { fingerprint: VolumeFingerprint },
	/// Emitted when a volume's properties are updated
	VolumeUpdated {
		fingerprint: VolumeFingerprint,
		old: VolumeInfo,
		new: VolumeInfo,
	},
	/// Emitted when a volume's speed test completes
	VolumeSpeedTested {
		fingerprint: VolumeFingerprint,
		read_speed_mbps: u64,
		write_speed_mbps: u64,
	},
	/// Emitted when a volume's mount status changes
	VolumeMountChanged {
		fingerprint: VolumeFingerprint,
		is_mounted: bool,
	},
	/// Emitted when a volume encounters an error
	VolumeError {
		fingerprint: VolumeFingerprint,
		error: String,
	},
}

/// Represents a physical or virtual storage volume in the system
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Volume {
	/// Unique fingerprint for this volume
	pub fingerprint: VolumeFingerprint,

	/// Device this volume belongs to
	pub device_id: uuid::Uuid,

	/// Human-readable volume name
	pub name: String,
	/// Type of mount (system, external, etc)
	pub mount_type: MountType,
	/// Classification of this volume for UX decisions
	pub volume_type: VolumeType,
	/// Primary path where the volume is mounted
	pub mount_point: PathBuf,
	/// Additional mount points (for APFS volumes, etc.)
	pub mount_points: Vec<PathBuf>,
	/// Whether the volume is currently mounted
	pub is_mounted: bool,

	/// Type of storage device (SSD, HDD, etc)
	pub disk_type: DiskType,
	/// Filesystem type (NTFS, EXT4, etc)
	pub file_system: FileSystem,
	/// Whether the volume is mounted read-only
	pub read_only: bool,

	/// Hardware identifier (platform-specific)
	pub hardware_id: Option<String>,
	/// Current error status if any
	pub error_status: Option<String>,

	// Storage information
	/// Total storage capacity in bytes
	pub total_bytes_capacity: u64,
	/// Available storage space in bytes
	pub total_bytes_available: u64,

	// Performance metrics (populated by speed tests)
	/// Read speed in megabytes per second
	pub read_speed_mbps: Option<u64>,
	/// Write speed in megabytes per second
	pub write_speed_mbps: Option<u64>,

	/// Whether this volume should be visible in default views
	pub is_user_visible: bool,

	/// Whether this volume should be auto-tracked
	pub auto_track_eligible: bool,

	/// When this volume information was last updated
	pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Summary information about a volume (for updates and caching)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
	pub is_mounted: bool,
	pub total_bytes_available: u64,
	pub read_speed_mbps: Option<u64>,
	pub write_speed_mbps: Option<u64>,
	pub error_status: Option<String>,
}

/// Information about a tracked volume in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedVolume {
	pub id: i32,
	pub uuid: uuid::Uuid,
	pub device_id: uuid::Uuid,
	pub fingerprint: VolumeFingerprint,
	pub display_name: Option<String>,
	pub tracked_at: chrono::DateTime<chrono::Utc>,
	pub last_seen_at: chrono::DateTime<chrono::Utc>,
	pub is_online: bool,
	pub total_capacity: Option<u64>,
	pub available_capacity: Option<u64>,
	pub read_speed_mbps: Option<u32>,
	pub write_speed_mbps: Option<u32>,
	pub last_speed_test_at: Option<chrono::DateTime<chrono::Utc>>,
	pub file_system: Option<String>,
	pub mount_point: Option<String>,
	pub is_removable: Option<bool>,
	pub is_network_drive: Option<bool>,
	pub device_model: Option<String>,
	pub volume_type: String,
	pub is_user_visible: Option<bool>,
	pub auto_track_eligible: Option<bool>,
}

impl From<&Volume> for VolumeInfo {
	fn from(volume: &Volume) -> Self {
		Self {
			is_mounted: volume.is_mounted,
			total_bytes_available: volume.total_bytes_available,
			read_speed_mbps: volume.read_speed_mbps,
			write_speed_mbps: volume.write_speed_mbps,
			error_status: volume.error_status.clone(),
		}
	}
}

impl Volume {
	/// Create a new Volume instance
	pub fn new(
		device_id: uuid::Uuid,
		name: String,
		mount_type: MountType,
		volume_type: VolumeType,
		mount_point: PathBuf,
		mount_points: Vec<PathBuf>,
		disk_type: DiskType,
		file_system: FileSystem,
		total_bytes_capacity: u64,
		total_bytes_available: u64,
		read_only: bool,
		hardware_id: Option<String>,
	) -> Self {
		let fingerprint = VolumeFingerprint::new(device_id, &mount_point, &name);

		Self {
			fingerprint,
			device_id,
			name,
			mount_type,
			volume_type,
			mount_point,
			mount_points,
			is_mounted: true,
			disk_type,
			file_system,
			read_only,
			hardware_id,
			error_status: None,
			total_bytes_capacity,
			total_bytes_available,
			read_speed_mbps: None,
			write_speed_mbps: None,
			is_user_visible: volume_type.show_by_default(),
			auto_track_eligible: volume_type.auto_track_by_default(),
			last_updated: chrono::Utc::now(),
		}
	}

	/// Update volume information
	pub fn update_info(&mut self, info: VolumeInfo) {
		self.is_mounted = info.is_mounted;
		self.total_bytes_available = info.total_bytes_available;
		self.read_speed_mbps = info.read_speed_mbps;
		self.write_speed_mbps = info.write_speed_mbps;
		self.error_status = info.error_status;
		self.last_updated = chrono::Utc::now();
	}

	/// Check if this volume supports fast copy operations (CoW)
	pub fn supports_fast_copy(&self) -> bool {
		matches!(
			self.file_system,
			FileSystem::APFS | FileSystem::Btrfs | FileSystem::ZFS | FileSystem::ReFS
		)
	}

	/// Get the optimal chunk size for copying to/from this volume
	pub fn optimal_chunk_size(&self) -> usize {
		match self.disk_type {
			DiskType::SSD => 1024 * 1024,   // 1MB for SSDs
			DiskType::HDD => 256 * 1024,    // 256KB for HDDs
			DiskType::Unknown => 64 * 1024, // 64KB default
		}
	}

	/// Estimate copy speed between this and another volume
	pub fn estimate_copy_speed(&self, other: &Volume) -> Option<u64> {
		let self_read = self.read_speed_mbps?;
		let other_write = other.write_speed_mbps?;

		// Bottleneck is the slower of read or write speed
		Some(self_read.min(other_write))
	}

	/// Check if a path is contained within this volume
	pub fn contains_path(&self, path: &PathBuf) -> bool {
		// Check primary mount point
		if path.starts_with(&self.mount_point) {
			return true;
		}

		// Check additional mount points
		for mount_point in &self.mount_points {
			if path.starts_with(mount_point) {
				return true;
			}
		}

		false
	}
}

/// Represents the type of physical storage device
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiskType {
	/// Solid State Drive
	SSD,
	/// Hard Disk Drive
	HDD,
	/// Unknown or virtual disk type
	Unknown,
}

impl fmt::Display for DiskType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DiskType::SSD => write!(f, "SSD"),
			DiskType::HDD => write!(f, "HDD"),
			DiskType::Unknown => write!(f, "Unknown"),
		}
	}
}

impl DiskType {
	pub fn from_string(disk_type: &str) -> Self {
		match disk_type.to_uppercase().as_str() {
			"SSD" => Self::SSD,
			"HDD" => Self::HDD,
			_ => Self::Unknown,
		}
	}
}

/// Represents the filesystem type of the volume
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileSystem {
	/// Windows NTFS filesystem
	NTFS,
	/// FAT32 filesystem
	FAT32,
	/// Linux EXT4 filesystem
	EXT4,
	/// Apple APFS filesystem
	APFS,
	/// ExFAT filesystem
	ExFAT,
	/// Btrfs filesystem (Linux)
	Btrfs,
	/// ZFS filesystem
	ZFS,
	/// Windows ReFS filesystem
	ReFS,
	/// Other/unknown filesystem type
	Other(String),
}

impl fmt::Display for FileSystem {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FileSystem::NTFS => write!(f, "NTFS"),
			FileSystem::FAT32 => write!(f, "FAT32"),
			FileSystem::EXT4 => write!(f, "EXT4"),
			FileSystem::APFS => write!(f, "APFS"),
			FileSystem::ExFAT => write!(f, "ExFAT"),
			FileSystem::Btrfs => write!(f, "Btrfs"),
			FileSystem::ZFS => write!(f, "ZFS"),
			FileSystem::ReFS => write!(f, "ReFS"),
			FileSystem::Other(name) => write!(f, "{}", name),
		}
	}
}

impl FileSystem {
	pub fn from_string(fs: &str) -> Self {
		match fs.to_uppercase().as_str() {
			"NTFS" => Self::NTFS,
			"FAT32" => Self::FAT32,
			"EXT4" => Self::EXT4,
			"APFS" => Self::APFS,
			"EXFAT" => Self::ExFAT,
			"BTRFS" => Self::Btrfs,
			"ZFS" => Self::ZFS,
			"REFS" => Self::ReFS,
			other => Self::Other(other.to_string()),
		}
	}

	/// Check if this filesystem supports reflinks/clones
	pub fn supports_reflink(&self) -> bool {
		matches!(self, Self::APFS | Self::Btrfs | Self::ZFS | Self::ReFS)
	}

	/// Check if this filesystem supports sendfile optimization
	pub fn supports_sendfile(&self) -> bool {
		matches!(self, Self::EXT4 | Self::Btrfs | Self::ZFS | Self::NTFS)
	}
}

/// Represents how the volume is mounted in the system
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MountType {
	/// System/boot volume
	System,
	/// External/removable volume
	External,
	/// Network-attached volume
	Network,
	/// Virtual/container volume
	Virtual,
}

impl fmt::Display for MountType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			MountType::System => write!(f, "System"),
			MountType::External => write!(f, "External"),
			MountType::Network => write!(f, "Network"),
			MountType::Virtual => write!(f, "Virtual"),
		}
	}
}

impl MountType {
	pub fn from_string(mount_type: &str) -> Self {
		match mount_type.to_uppercase().as_str() {
			"SYSTEM" => Self::System,
			"EXTERNAL" => Self::External,
			"NETWORK" => Self::Network,
			"VIRTUAL" => Self::Virtual,
			_ => Self::System,
		}
	}
}

/// Configuration for volume detection and monitoring
#[derive(Debug, Clone)]
pub struct VolumeDetectionConfig {
	/// Whether to include system volumes
	pub include_system: bool,
	/// Whether to include virtual volumes
	pub include_virtual: bool,
	/// Whether to run speed tests on discovery
	pub run_speed_test: bool,
	/// How often to refresh volume information (in seconds)
	pub refresh_interval_secs: u64,
}

impl Default for VolumeDetectionConfig {
	fn default() -> Self {
		Self {
			include_system: true,
			include_virtual: false,
			run_speed_test: false, // Expensive operation, off by default
			refresh_interval_secs: 30,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_volume_fingerprint() {
		let volume = Volume::new(
			uuid::Uuid::new_v4(),
			"Test Volume".to_string(),
			MountType::External,
			VolumeType::External,
			PathBuf::from("/mnt/test"),
			vec![],
			DiskType::SSD,
			FileSystem::EXT4,
			1000000000,
			500000000,
			false,
			Some("test-hw-id".to_string()),
		);

		let fingerprint =
			VolumeFingerprint::new(&volume.device_id, &volume.mount_point, &volume.name);
		assert!(!fingerprint.0.is_empty());

		// Same volume should produce same fingerprint
		let fingerprint2 =
			VolumeFingerprint::new(&volume.device_id, &volume.mount_point, &volume.name);
		assert_eq!(fingerprint, fingerprint2);
	}

	#[test]
	fn test_volume_contains_path() {
		let volume = Volume::new(
			uuid::Uuid::new_v4(),
			"Test".to_string(),
			MountType::System,
			VolumeType::System,
			PathBuf::from("/home"),
			vec![PathBuf::from("/home"), PathBuf::from("/mnt/home")],
			DiskType::SSD,
			FileSystem::EXT4,
			1000000,
			500000,
			false,
			None,
		);

		assert!(volume.contains_path(&PathBuf::from("/home/user/file.txt")));
		assert!(volume.contains_path(&PathBuf::from("/mnt/home/user/file.txt")));
		assert!(!volume.contains_path(&PathBuf::from("/var/log/file.txt")));
	}

	#[test]
	fn test_filesystem_capabilities() {
		assert!(FileSystem::APFS.supports_reflink());
		assert!(FileSystem::Btrfs.supports_reflink());
		assert!(!FileSystem::FAT32.supports_reflink());

		assert!(FileSystem::EXT4.supports_sendfile());
		assert!(!FileSystem::FAT32.supports_sendfile());
	}
}
