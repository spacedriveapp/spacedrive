use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// A fingerprint of a volume, used to identify it when it is not persisted in the database
#[derive(Debug, Clone, Hash, Eq, PartialEq, Type)]
pub struct VolumeFingerprint(pub Vec<u8>);

impl fmt::Display for VolumeFingerprint {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", hex::encode(&self.0))
	}
}

impl Serialize for VolumeFingerprint {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_str(&hex::encode(&self.0))
	}
}

impl<'de> Deserialize<'de> for VolumeFingerprint {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		hex::decode(s)
			.map(VolumeFingerprint)
			.map_err(serde::de::Error::custom)
	}
}

/// Trait for types that can generate a volume fingerprint
pub trait Fingerprintable {
	fn fingerprint(&self) -> VolumeFingerprint;
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct VolumePubId(pub Vec<u8>);

impl From<Vec<u8>> for VolumePubId {
	fn from(v: Vec<u8>) -> Self {
		Self(v)
	}
}

impl Into<Vec<u8>> for VolumePubId {
	fn into(self) -> Vec<u8> {
		self.0
	}
}

pub type LibraryId = Uuid;

/// Represents the type of physical storage device
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum DiskType {
	/// Solid State Drive
	SSD,
	/// Hard Disk Drive
	HDD,
	/// Unknown or virtual disk type
	Unknown,
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
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq)]
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
	/// Other/unknown filesystem type
	Other(String),
}

impl FileSystem {
	pub fn from_string(fs: &str) -> Self {
		match fs.to_uppercase().as_str() {
			"NTFS" => FileSystem::NTFS,
			"FAT32" => FileSystem::FAT32,
			"EXT4" => FileSystem::EXT4,
			"APFS" => FileSystem::APFS,
			"EXFAT" => FileSystem::ExFAT,
			other => FileSystem::Other(other.to_string()),
		}
	}
}

/// Represents how the volume is mounted in the system
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq)]
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

/// Core volume information needed across crates
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct VolumeInfo {
	pub fingerprint: Option<VolumeFingerprint>,
	pub pub_id: Option<Vec<u8>>,
	pub name: String,
	pub mount_type: MountType,
	pub mount_point: PathBuf,
	pub is_mounted: bool,
	pub disk_type: DiskType,
	pub file_system: FileSystem,
	pub read_only: bool,
	pub total_bytes_capacity: u64,
	pub total_bytes_available: u64,
}

/// Events emitted by the Volume Manager when volume state changes
#[derive(Debug, Clone, Type, Deserialize, Serialize)]
pub enum VolumeEvent {
	/// Emitted when a new volume is discovered and added
	VolumeAdded(VolumeInfo),
	/// Emitted when a volume is removed from the system
	VolumeRemoved(VolumeInfo),
	/// Emitted when a volume's properties are updated
	VolumeUpdated { old: VolumeInfo, new: VolumeInfo },
	/// Emitted when a volume's speed test completes
	VolumeSpeedTested {
		fingerprint: VolumeFingerprint,
		read_speed: u64,
		write_speed: u64,
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

/// Core volume operations that can be performed across crates
pub trait VolumeOperations {
	/// Check if a path is under any of this volume's mount points
	fn contains_path(&self, path: &Path) -> bool;
	/// Check if the volume is tracked in the database
	fn is_tracked(&self) -> bool;
}
