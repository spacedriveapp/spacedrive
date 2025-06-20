//! Volume type definitions

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// A fingerprint of a volume, used to identify it uniquely across sessions
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct VolumeFingerprint(pub String);

impl VolumeFingerprint {
	/// Create a new volume fingerprint from volume properties
	pub fn new(volume: &Volume) -> Self {
		let mut hasher = blake3::Hasher::new();
		hasher.update(volume.mount_point.to_string_lossy().as_bytes());
		hasher.update(volume.name.as_bytes());
		hasher.update(&volume.total_bytes_capacity.to_be_bytes());
		hasher.update(volume.file_system.to_string().as_bytes());

		// Include hardware identifier if available
		if let Some(ref hw_id) = volume.hardware_id {
			hasher.update(hw_id.as_bytes());
		}

		Self(hasher.finalize().to_hex().to_string())
	}

	/// Create fingerprint from hex string
	pub fn from_hex(hex: impl Into<String>) -> Self {
		Self(hex.into())
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

	/// Human-readable volume name
	pub name: String,
	/// Type of mount (system, external, etc)
	pub mount_type: MountType,
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
		name: String,
		mount_type: MountType,
		mount_point: PathBuf,
		mount_points: Vec<PathBuf>,
		disk_type: DiskType,
		file_system: FileSystem,
		total_bytes_capacity: u64,
		total_bytes_available: u64,
		read_only: bool,
		hardware_id: Option<String>,
	) -> Self {
		let volume = Self {
			fingerprint: VolumeFingerprint::from_hex(""), // Will be set after creation
			name,
			mount_type,
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
			last_updated: chrono::Utc::now(),
		};

		// Generate fingerprint after creation
		let mut volume = volume;
		volume.fingerprint = VolumeFingerprint::new(&volume);
		volume
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
			"Test Volume".to_string(),
			MountType::External,
			PathBuf::from("/mnt/test"),
			vec![],
			DiskType::SSD,
			FileSystem::EXT4,
			1000000000,
			500000000,
			false,
			Some("test-hw-id".to_string()),
		);

		let fingerprint = VolumeFingerprint::new(&volume);
		assert!(!fingerprint.0.is_empty());

		// Same volume should produce same fingerprint
		let fingerprint2 = VolumeFingerprint::new(&volume);
		assert_eq!(fingerprint, fingerprint2);
	}

	#[test]
	fn test_volume_contains_path() {
		let volume = Volume::new(
			"Test".to_string(),
			MountType::System,
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
