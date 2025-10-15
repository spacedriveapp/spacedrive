//! Volume domain model - unified volume representation
//!
//! This represents volumes in Spacedrive, combining runtime detection capabilities
//! with database tracking and user preferences. Supports local, network, and cloud volumes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Unique fingerprint for a storage volume
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Type)]
pub struct VolumeFingerprint(pub String);

impl VolumeFingerprint {
	/// Create a new volume fingerprint from volume properties
	pub fn new(name: &str, total_bytes: u64, file_system: &str) -> Self {
		let mut hasher = blake3::Hasher::new();
		hasher.update(b"content_based:");
		hasher.update(name.as_bytes());
		hasher.update(&total_bytes.to_be_bytes());
		hasher.update(file_system.as_bytes());
		hasher.update(&(name.len() as u64).to_be_bytes());
		Self(hasher.finalize().to_hex().to_string())
	}

	/// Create a fingerprint from a Spacedrive identifier UUID
	pub fn from_spacedrive_id(spacedrive_id: Uuid) -> Self {
		let mut hasher = blake3::Hasher::new();
		hasher.update(b"spacedrive_id:");
		hasher.update(spacedrive_id.as_bytes());
		Self(hasher.finalize().to_hex().to_string())
	}

	/// Generate 8-character short ID for display
	pub fn short_id(&self) -> String {
		self.0.chars().take(8).collect()
	}

	/// Generate 12-character medium ID for disambiguation
	pub fn medium_id(&self) -> String {
		self.0.chars().take(12).collect()
	}

	/// Create fingerprint from hex string
	pub fn from_hex(hex: impl Into<String>) -> Self {
		Self(hex.into())
	}

	/// Create fingerprint from string (alias for from_hex)
	pub fn from_string(s: &str) -> Result<Self, String> {
		Ok(Self(s.to_string()))
	}

	/// Check if a string could be a short ID
	pub fn is_short_id(s: &str) -> bool {
		s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
	}

	/// Check if a string could be a medium ID
	pub fn is_medium_id(s: &str) -> bool {
		s.len() == 12 && s.chars().all(|c| c.is_ascii_hexdigit())
	}

	/// Check if this fingerprint matches a short or medium ID
	pub fn matches_short_id(&self, short_id: &str) -> bool {
		if Self::is_short_id(short_id) {
			self.short_id() == short_id
		} else if Self::is_medium_id(short_id) {
			self.medium_id() == short_id
		} else {
			false
		}
	}
}

impl fmt::Display for VolumeFingerprint {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Represents an APFS container (physical storage with multiple volumes)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct ApfsContainer {
	pub container_id: String,
	pub uuid: String,
	pub physical_store: String,
	pub total_capacity: u64,
	pub capacity_in_use: u64,
	pub capacity_free: u64,
	pub volumes: Vec<ApfsVolumeInfo>,
}

/// APFS volume information within a container
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct ApfsVolumeInfo {
	pub disk_id: String,
	pub uuid: String,
	pub role: ApfsVolumeRole,
	pub name: String,
	pub mount_point: Option<PathBuf>,
	pub capacity_consumed: u64,
	pub sealed: bool,
	pub filevault: bool,
}

/// APFS volume roles in the container
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum ApfsVolumeRole {
	System,
	Data,
	Preboot,
	Recovery,
	VM,
	Other(String),
}

impl fmt::Display for ApfsVolumeRole {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ApfsVolumeRole::System => write!(f, "System"),
			ApfsVolumeRole::Data => write!(f, "Data"),
			ApfsVolumeRole::Preboot => write!(f, "Preboot"),
			ApfsVolumeRole::Recovery => write!(f, "Recovery"),
			ApfsVolumeRole::VM => write!(f, "VM"),
			ApfsVolumeRole::Other(role) => write!(f, "{}", role),
		}
	}
}

/// Path mapping for resolving virtual paths to actual storage locations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct PathMapping {
	pub virtual_path: PathBuf,
	pub actual_path: PathBuf,
}

/// Spacedrive volume identifier file content
/// This file is created in the root of writable volumes for persistent identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacedriveVolumeId {
	pub id: Uuid,
	pub created: DateTime<Utc>,
	pub device_name: Option<String>,
	pub volume_name: String,
	pub device_id: Uuid,
	pub library_id: Uuid, // TODO: Populate this, super helpful when another library comes across this file. Thinking about it now we should probably make this file accept multiple of these entries in case two libraries need to track the same volume.
}

/// Summary information about a volume (for updates and caching)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
	pub uuid: Uuid,
	pub device_id: Uuid,
	pub fingerprint: VolumeFingerprint,
	pub display_name: Option<String>,
	pub tracked_at: DateTime<Utc>,
	pub last_seen_at: DateTime<Utc>,
	pub is_online: bool,
	pub total_capacity: Option<u64>,
	pub available_capacity: Option<u64>,
	pub read_speed_mbps: Option<u32>,
	pub write_speed_mbps: Option<u32>,
	pub last_speed_test_at: Option<DateTime<Utc>>,
	pub file_system: Option<String>,
	pub mount_point: Option<String>,
	pub is_removable: Option<bool>,
	pub is_network_drive: Option<bool>,
	pub device_model: Option<String>,
	pub volume_type: String,
	pub is_user_visible: Option<bool>,
	pub auto_track_eligible: Option<bool>,
}

/// Events emitted by the Volume Manager when volume state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolumeEvent {
	VolumeAdded(Volume),
	VolumeRemoved {
		fingerprint: VolumeFingerprint,
	},
	VolumeUpdated {
		fingerprint: VolumeFingerprint,
		old: VolumeInfo,
		new: VolumeInfo,
	},
	VolumeSpeedTested {
		fingerprint: VolumeFingerprint,
		read_speed_mbps: u64,
		write_speed_mbps: u64,
	},
	VolumeMountChanged {
		fingerprint: VolumeFingerprint,
		is_mounted: bool,
	},
	VolumeError {
		fingerprint: VolumeFingerprint,
		error: String,
	},
}

/// Configuration for volume detection and monitoring
#[derive(Debug, Clone)]
pub struct VolumeDetectionConfig {
	pub include_system: bool,
	pub include_virtual: bool,
	pub run_speed_test: bool,
	pub refresh_interval_secs: u64,
}

impl Default for VolumeDetectionConfig {
	fn default() -> Self {
		Self {
			include_system: true,
			include_virtual: false,
			run_speed_test: false,
			refresh_interval_secs: 30,
		}
	}
}

/// A volume in Spacedrive - unified model for runtime and database
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Volume {
	/// Unique identifier (used in SdPath addressing)
	pub id: Uuid,

	/// Volume fingerprint for identification
	pub fingerprint: VolumeFingerprint,

	/// Device this volume is attached to
	pub device_id: Uuid,

	/// Human-readable name
	pub name: String,

	/// Library this volume belongs to (None for untracked volumes)
	pub library_id: Option<Uuid>,

	/// Whether this volume is being tracked by Spacedrive
	pub is_tracked: bool,

	/// Primary mount point
	pub mount_point: PathBuf,

	/// Additional mount points for the same volume
	pub mount_points: Vec<PathBuf>,

	/// Volume type/category
	pub volume_type: VolumeType,

	/// Mount type classification
	pub mount_type: MountType,

	/// Disk type (SSD, HDD, etc.)
	pub disk_type: DiskType,

	/// Filesystem type
	pub file_system: FileSystem,

	/// Total capacity in bytes
	pub total_capacity: u64,

	/// Currently available space in bytes
	pub available_space: u64,

	/// Whether volume is read-only
	pub is_read_only: bool,

	/// Whether volume is currently mounted/available
	pub is_mounted: bool,

	/// Hardware identifier (device path, UUID, etc.)
	pub hardware_id: Option<String>,

	/// I/O backend for this volume (not serialized)
	#[serde(skip)]
	#[specta(skip)]
	pub backend: Option<Arc<dyn crate::volume::VolumeBackend>>,

	/// APFS container information (macOS only)
	pub apfs_container: Option<ApfsContainer>,

	/// Container-relative volume ID for same-container detection
	pub container_volume_id: Option<String>,

	/// Path resolution mappings (for firmlinks/symlinks)
	pub path_mappings: Vec<PathMapping>,

	/// Whether this volume should be visible in default views
	pub is_user_visible: bool,

	/// Whether this volume should be auto-tracked
	pub auto_track_eligible: bool,

	/// Performance metrics
	pub read_speed_mbps: Option<u64>,
	pub write_speed_mbps: Option<u64>,

	/// Timestamps
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub last_seen_at: DateTime<Utc>,

	/// Statistics
	pub total_files: Option<u64>,
	pub total_directories: Option<u64>,
	pub last_stats_update: Option<DateTime<Utc>>,

	/// User preferences
	pub display_name: Option<String>,
	pub is_favorite: bool,
	pub color: Option<String>,
	pub icon: Option<String>,

	/// Error state
	pub error_message: Option<String>,
}

/// Volume type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum VolumeType {
	/// Primary system drive containing OS and user data
	Primary,
	/// Dedicated user data volumes (separate from OS)
	UserData,
	/// External or removable storage devices
	External,
	/// Secondary internal storage (additional drives/partitions)
	Secondary,
	/// System/OS internal volumes (hidden from normal view)
	System,
	/// Network attached storage
	Network,
	/// Cloud storage mounts
	Cloud,
	/// Virtual/temporary storage
	Virtual,
	/// Unknown or unclassified volumes
	Unknown,
}

impl VolumeType {
	/// Should this volume type be auto-tracked by default?
	pub fn auto_track_by_default(&self) -> bool {
		matches!(self, VolumeType::Primary)
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
			VolumeType::Cloud => "Cloud Storage",
			VolumeType::Virtual => "Virtual Storage",
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
			VolumeType::Cloud => "[CLD]",
			VolumeType::Virtual => "[VRT]",
			VolumeType::Unknown => "[UNK]",
		}
	}
}

/// Mount type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum MountType {
	/// System mount (root, boot, etc.)
	System,

	/// External device mount
	External,

	/// Network mount
	Network,

	/// User mount
	User,
}

impl fmt::Display for MountType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			MountType::System => write!(f, "System"),
			MountType::External => write!(f, "External"),
			MountType::Network => write!(f, "Network"),
			MountType::User => write!(f, "User"),
		}
	}
}

impl MountType {
	pub fn from_string(mount_type: &str) -> Self {
		match mount_type.to_uppercase().as_str() {
			"SYSTEM" => Self::System,
			"EXTERNAL" => Self::External,
			"NETWORK" => Self::Network,
			"USER" => Self::User,
			_ => Self::System,
		}
	}
}

/// Disk type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Type)]
pub enum DiskType {
	/// Solid State Drive
	SSD,

	/// Hard Disk Drive
	HDD,

	/// Network storage
	Network,

	/// Virtual/RAM disk
	Virtual,

	/// Unknown type
	Unknown,
}

impl fmt::Display for DiskType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DiskType::SSD => write!(f, "SSD"),
			DiskType::HDD => write!(f, "HDD"),
			DiskType::Network => write!(f, "Network"),
			DiskType::Virtual => write!(f, "Virtual"),
			DiskType::Unknown => write!(f, "Unknown"),
		}
	}
}

impl DiskType {
	pub fn from_string(disk_type: &str) -> Self {
		match disk_type.to_uppercase().as_str() {
			"SSD" => Self::SSD,
			"HDD" => Self::HDD,
			"NETWORK" => Self::Network,
			"VIRTUAL" => Self::Virtual,
			_ => Self::Unknown,
		}
	}
}

/// Filesystem type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Type)]
pub enum FileSystem {
	/// Apple File System
	APFS,

	/// NT File System (Windows)
	NTFS,

	/// Fourth Extended Filesystem (Linux)
	Ext4,

	/// B-tree Filesystem (Linux)
	Btrfs,

	/// ZFS
	ZFS,

	/// Resilient File System (Windows)
	ReFS,

	/// File Allocation Table 32
	FAT32,

	/// Extended File Allocation Table
	ExFAT,

	/// Hierarchical File System Plus (macOS legacy)
	HFSPlus,

	/// Network File System
	NFS,

	/// Server Message Block
	SMB,

	/// Other filesystem
	Other(String),
}

impl Volume {
	/// Create a new tracked volume
	pub fn new(
		device_id: Uuid,
		fingerprint: VolumeFingerprint,
		name: String,
		mount_point: PathBuf,
	) -> Self {
		let now = Utc::now();
		Self {
			id: Uuid::new_v4(),
			library_id: None,
			device_id,
			fingerprint,
			name: name.clone(),
			mount_point,
			mount_points: Vec::new(),
			volume_type: VolumeType::Unknown,
			mount_type: MountType::System,
			disk_type: DiskType::Unknown,
			file_system: FileSystem::Other("Unknown".to_string()),
			total_capacity: 0,
			available_space: 0,
			is_read_only: false,
			is_mounted: true,
			is_tracked: false,
			hardware_id: None,
			backend: None,
			apfs_container: None,
			container_volume_id: None,
			path_mappings: Vec::new(),
			is_user_visible: true,
			auto_track_eligible: true,
			read_speed_mbps: None,
			write_speed_mbps: None,
			created_at: now,
			updated_at: now,
			last_seen_at: now,
			total_files: None,
			total_directories: None,
			last_stats_update: None,
			display_name: Some(name),
			is_favorite: false,
			color: None,
			icon: None,
			error_message: None,
		}
	}

	/// Mark volume as tracked
	pub fn track(&mut self, library_id: Option<Uuid>) {
		self.is_tracked = true;
		self.library_id = library_id;
		self.updated_at = Utc::now();
	}

	/// Mark volume as untracked
	pub fn untrack(&mut self) {
		self.is_tracked = false;
		self.library_id = None;
		self.updated_at = Utc::now();
	}

	/// Set display preferences
	pub fn set_display_preferences(
		&mut self,
		display_name: Option<String>,
		color: Option<String>,
		icon: Option<String>,
	) {
		self.display_name = display_name;
		self.color = color;
		self.icon = icon;
		self.updated_at = Utc::now();
	}

	/// Mark as favorite
	pub fn set_favorite(&mut self, is_favorite: bool) {
		self.is_favorite = is_favorite;
		self.updated_at = Utc::now();
	}

	/// Update statistics
	pub fn update_statistics(&mut self, total_files: u64, total_directories: u64) {
		self.total_files = Some(total_files);
		self.total_directories = Some(total_directories);
		self.last_stats_update = Some(Utc::now());
		self.updated_at = Utc::now();
	}

	/// Set error state
	pub fn set_error(&mut self, error: String) {
		self.error_message = Some(error);
		self.is_mounted = false;
		self.updated_at = Utc::now();
	}

	/// Clear error state
	pub fn clear_error(&mut self) {
		self.error_message = None;
		self.updated_at = Utc::now();
	}

	/// Get display name (fallback to name)
	pub fn display_name(&self) -> &str {
		self.display_name.as_ref().unwrap_or(&self.name)
	}

	/// Check if volume supports copy-on-write
	pub fn supports_cow(&self) -> bool {
		matches!(
			self.file_system,
			FileSystem::APFS | FileSystem::Btrfs | FileSystem::ZFS | FileSystem::ReFS
		)
	}

	/// Get capacity utilization percentage
	pub fn utilization_percentage(&self) -> f64 {
		if self.total_capacity == 0 {
			return 0.0;
		}
		let used = self.total_capacity.saturating_sub(self.available_space);
		(used as f64 / self.total_capacity as f64) * 100.0
	}

	/// Check if volume needs space warning
	pub fn needs_space_warning(&self, threshold_percent: f64) -> bool {
		self.utilization_percentage() > threshold_percent
	}

	/// Field alias: uuid -> id (for backward compatibility)
	pub fn uuid(&self) -> Uuid {
		self.id
	}

	/// Field alias: total_bytes_capacity -> total_capacity
	pub fn total_bytes_capacity(&self) -> u64 {
		self.total_capacity
	}

	/// Field alias: total_bytes_available -> available_space
	pub fn total_bytes_available(&self) -> u64 {
		self.available_space
	}

	/// Field alias: read_only -> is_read_only
	pub fn read_only(&self) -> bool {
		self.is_read_only
	}

	/// Field alias: error_status -> error_message
	pub fn error_status(&self) -> Option<&String> {
		self.error_message.as_ref()
	}

	/// Field alias: last_updated -> updated_at
	pub fn last_updated(&self) -> DateTime<Utc> {
		self.updated_at
	}

	/// Update volume information
	pub fn update_info(&mut self, info: VolumeInfo) {
		self.is_mounted = info.is_mounted;
		self.available_space = info.total_bytes_available;
		self.read_speed_mbps = info.read_speed_mbps;
		self.write_speed_mbps = info.write_speed_mbps;
		self.error_message = info.error_status;
		self.updated_at = Utc::now();
	}

	/// Check if this volume supports fast copy operations (CoW)
	pub fn supports_fast_copy(&self) -> bool {
		self.supports_cow()
	}

	/// Get the optimal chunk size for copying to/from this volume
	pub fn optimal_chunk_size(&self) -> usize {
		match self.disk_type {
			DiskType::SSD => 1024 * 1024, // 1MB for SSDs
			DiskType::HDD => 256 * 1024,  // 256KB for HDDs
			_ => 64 * 1024,               // 64KB default
		}
	}

	/// Estimate copy speed between this and another volume
	pub fn estimate_copy_speed(&self, other: &Volume) -> Option<u64> {
		let self_read = self.read_speed_mbps?;
		let other_write = other.write_speed_mbps?;
		Some(self_read.min(other_write))
	}

	/// Check if a path is contained within this volume
	pub fn contains_path(&self, path: &PathBuf) -> bool {
		crate::volume::fs::contains_path(self, path)
	}

	/// Parse cloud service and identifier from mount point
	/// Returns None for non-cloud volumes or unparseable mount points
	///
	/// # Examples
	/// - "s3://my-bucket" → Some((S3, "my-bucket"))
	/// - "gdrive://My Drive" → Some((GoogleDrive, "My Drive"))
	/// - "/mnt/local" → None
	pub fn parse_cloud_identity(&self) -> Option<(crate::volume::backend::CloudServiceType, String)> {
		use crate::volume::backend::CloudServiceType;

		let mount_str = self.mount_point.to_string_lossy();
		let parts: Vec<&str> = mount_str.splitn(2, "://").collect();

		if parts.len() != 2 {
			return None;
		}

		let service = CloudServiceType::from_scheme(parts[0])?;
		let identifier = parts[1].trim_start_matches('/').to_string();

		Some((service, identifier))
	}
}

impl From<&Volume> for VolumeInfo {
	fn from(volume: &Volume) -> Self {
		Self {
			is_mounted: volume.is_mounted,
			total_bytes_available: volume.available_space,
			read_speed_mbps: volume.read_speed_mbps,
			write_speed_mbps: volume.write_speed_mbps,
			error_status: volume.error_message.clone(),
		}
	}
}

impl TrackedVolume {
	/// Convert a TrackedVolume back to a Volume for display purposes
	/// This is used for offline volumes that aren't currently detected
	pub fn to_offline_volume(&self) -> Volume {
		Volume {
			id: self.uuid,
			fingerprint: self.fingerprint.clone(),
			device_id: self.device_id,
			name: self
				.display_name
				.clone()
				.unwrap_or_else(|| "Unknown".to_string()),
			library_id: None,
			is_tracked: true,
			mount_point: PathBuf::from(
				self.mount_point
					.clone()
					.unwrap_or_else(|| "Not connected".to_string()),
			),
			mount_points: Vec::new(),
			volume_type: match self.volume_type.as_str() {
				"Primary" => VolumeType::Primary,
				"UserData" => VolumeType::UserData,
				"External" => VolumeType::External,
				"Secondary" => VolumeType::Secondary,
				"System" => VolumeType::System,
				"Network" => VolumeType::Network,
				"Cloud" => VolumeType::Cloud,
				"Virtual" => VolumeType::Virtual,
				_ => VolumeType::Unknown,
			},
			mount_type: MountType::External,
			disk_type: DiskType::Unknown,
			file_system: FileSystem::from_string(
				&self
					.file_system
					.clone()
					.unwrap_or_else(|| "Unknown".to_string()),
			),
			total_capacity: self.total_capacity.unwrap_or(0),
			available_space: self.available_capacity.unwrap_or(0),
			is_read_only: false,
			is_mounted: false,
			hardware_id: self.device_model.clone(),
			backend: None,
			apfs_container: None,
			container_volume_id: None,
			path_mappings: Vec::new(),
			is_user_visible: self.is_user_visible.unwrap_or(true),
			auto_track_eligible: self.auto_track_eligible.unwrap_or(false),
			read_speed_mbps: self.read_speed_mbps.map(|s| s as u64),
			write_speed_mbps: self.write_speed_mbps.map(|s| s as u64),
			created_at: self.tracked_at,
			updated_at: self.last_seen_at,
			last_seen_at: self.last_seen_at,
			total_files: None,
			total_directories: None,
			last_stats_update: None,
			display_name: self.display_name.clone(),
			is_favorite: false,
			color: None,
			icon: None,
			error_message: None,
		}
	}
}

impl FileSystem {
	/// Convert to string for storage
	pub fn to_string(&self) -> String {
		match self {
			FileSystem::APFS => "APFS".to_string(),
			FileSystem::NTFS => "NTFS".to_string(),
			FileSystem::Ext4 => "ext4".to_string(),
			FileSystem::Btrfs => "btrfs".to_string(),
			FileSystem::ZFS => "ZFS".to_string(),
			FileSystem::ReFS => "ReFS".to_string(),
			FileSystem::FAT32 => "FAT32".to_string(),
			FileSystem::ExFAT => "exFAT".to_string(),
			FileSystem::HFSPlus => "HFS+".to_string(),
			FileSystem::NFS => "NFS".to_string(),
			FileSystem::SMB => "SMB".to_string(),
			FileSystem::Other(name) => name.clone(),
		}
	}

	/// Create from string
	pub fn from_string(s: &str) -> Self {
		match s.to_uppercase().as_str() {
			"APFS" => FileSystem::APFS,
			"NTFS" => FileSystem::NTFS,
			"EXT4" => FileSystem::Ext4,
			"BTRFS" => FileSystem::Btrfs,
			"ZFS" => FileSystem::ZFS,
			"REFS" => FileSystem::ReFS,
			"FAT32" => FileSystem::FAT32,
			"EXFAT" => FileSystem::ExFAT,
			"HFS+" => FileSystem::HFSPlus,
			"NFS" => FileSystem::NFS,
			"SMB" => FileSystem::SMB,
			_ => FileSystem::Other(s.to_string()),
		}
	}
}

impl std::fmt::Display for VolumeType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			VolumeType::Primary => write!(f, "Primary"),
			VolumeType::UserData => write!(f, "UserData"),
			VolumeType::External => write!(f, "External"),
			VolumeType::Secondary => write!(f, "Secondary"),
			VolumeType::System => write!(f, "System"),
			VolumeType::Network => write!(f, "Network"),
			VolumeType::Cloud => write!(f, "Cloud"),
			VolumeType::Virtual => write!(f, "Virtual"),
			VolumeType::Unknown => write!(f, "Unknown"),
		}
	}
}

impl std::fmt::Display for FileSystem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn test_volume_creation() {
		let fingerprint = VolumeFingerprint("test-fingerprint".to_string());
		let volume = Volume::new(
			Uuid::new_v4(),
			fingerprint.clone(),
			"Test Volume".to_string(),
			PathBuf::from("/mnt/test"),
		);

		assert_eq!(volume.name, "Test Volume");
		assert_eq!(volume.fingerprint, fingerprint);
		assert_eq!(volume.display_name(), "Test Volume");
		assert!(!volume.is_tracked);
		assert!(!volume.is_favorite);
	}

	#[test]
	fn test_volume_tracking() {
		let fingerprint = VolumeFingerprint("test".to_string());
		let mut volume = Volume::new(
			Uuid::new_v4(),
			fingerprint,
			"Test".to_string(),
			PathBuf::from("/test"),
		);

		let library_id = Uuid::new_v4();
		volume.track(Some(library_id));

		assert!(volume.is_tracked);
		assert_eq!(volume.library_id, Some(library_id));

		volume.untrack();
		assert!(!volume.is_tracked);
		assert_eq!(volume.library_id, None);
	}

	#[test]
	fn test_filesystem_conversion() {
		assert_eq!(FileSystem::from_string("APFS"), FileSystem::APFS);
		assert_eq!(FileSystem::from_string("ext4"), FileSystem::Ext4);
		assert_eq!(
			FileSystem::from_string("unknown"),
			FileSystem::Other("unknown".to_string())
		);

		assert_eq!(FileSystem::APFS.to_string(), "APFS");
		assert_eq!(FileSystem::Ext4.to_string(), "ext4");
	}

	#[test]
	fn test_utilization_calculation() {
		let fingerprint = VolumeFingerprint("test".to_string());
		let mut volume = Volume::new(
			Uuid::new_v4(),
			fingerprint,
			"Test".to_string(),
			PathBuf::from("/test"),
		);

		volume.total_capacity = 1000;
		volume.available_space = 300;

		assert!((volume.utilization_percentage() - 70.0).abs() < f64::EPSILON);
		assert!(volume.needs_space_warning(60.0));
		assert!(!volume.needs_space_warning(80.0));
	}

	#[test]
	fn test_cow_support() {
		let fingerprint = VolumeFingerprint("test".to_string());
		let mut volume = Volume::new(
			Uuid::new_v4(),
			fingerprint,
			"Test".to_string(),
			PathBuf::from("/test"),
		);

		volume.file_system = FileSystem::APFS;
		assert!(volume.supports_cow());

		volume.file_system = FileSystem::NTFS;
		assert!(!volume.supports_cow());

		volume.file_system = FileSystem::Btrfs;
		assert!(volume.supports_cow());
	}
}
