//! Volume domain model - persistent storage for tracked volumes
//! 
//! This represents volumes that are tracked in the database, allowing Spacedrive
//! to remember volumes across sessions and track their metadata/statistics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// A tracked volume in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    /// Unique identifier
    pub id: Uuid,
    
    /// Library this volume belongs to (None for system-wide volumes)
    pub library_id: Option<Uuid>,
    
    /// Device this volume is attached to
    pub device_id: Uuid,
    
    /// Volume fingerprint for identification
    pub fingerprint: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Current mount point (can change)
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
    
    /// Whether this volume is being tracked by Spacedrive
    pub is_tracked: bool,
    
    /// Hardware identifier (device path, UUID, etc.)
    pub hardware_id: Option<String>,
    
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VolumeType {
    /// Primary system drive
    System,
    
    /// Internal storage (additional drives)
    Internal,
    
    /// External storage (USB, external drives)
    External,
    
    /// Network storage (NFS, SMB, etc.)
    Network,
    
    /// Cloud storage mounts
    Cloud,
    
    /// Virtual/temporary storage
    Virtual,
    
    /// Unknown type
    Unknown,
}

/// Mount type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// Disk type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// Filesystem type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        fingerprint: String,
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
    
    /// Create from runtime volume detection
    pub fn from_runtime_volume(runtime_vol: &crate::volume::Volume, device_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            library_id: None,
            device_id,
            fingerprint: runtime_vol.fingerprint.to_string(),
            name: runtime_vol.name.clone(),
            mount_point: runtime_vol.mount_point.clone(),
            mount_points: runtime_vol.mount_points.clone(),
            volume_type: VolumeType::from_mount_type(&runtime_vol.mount_type),
            mount_type: MountType::from_runtime_mount_type(&runtime_vol.mount_type),
            disk_type: DiskType::from_runtime_disk_type(&runtime_vol.disk_type),
            file_system: FileSystem::from_runtime_filesystem(&runtime_vol.file_system),
            total_capacity: runtime_vol.total_bytes_capacity,
            available_space: runtime_vol.total_bytes_available,
            is_read_only: runtime_vol.is_read_only,
            is_mounted: runtime_vol.is_mounted,
            is_tracked: false,
            hardware_id: runtime_vol.hardware_id.clone(),
            read_speed_mbps: runtime_vol.read_speed_mbps,
            write_speed_mbps: runtime_vol.write_speed_mbps,
            created_at: now,
            updated_at: now,
            last_seen_at: now,
            total_files: None,
            total_directories: None,
            last_stats_update: None,
            display_name: Some(runtime_vol.name.clone()),
            is_favorite: false,
            color: None,
            icon: None,
            error_message: None,
        }
    }
    
    /// Update from runtime volume
    pub fn update_from_runtime(&mut self, runtime_vol: &crate::volume::Volume) {
        self.mount_point = runtime_vol.mount_point.clone();
        self.mount_points = runtime_vol.mount_points.clone();
        self.total_capacity = runtime_vol.total_bytes_capacity;
        self.available_space = runtime_vol.total_bytes_available;
        self.is_read_only = runtime_vol.is_read_only;
        self.is_mounted = runtime_vol.is_mounted;
        self.hardware_id = runtime_vol.hardware_id.clone();
        self.read_speed_mbps = runtime_vol.read_speed_mbps;
        self.write_speed_mbps = runtime_vol.write_speed_mbps;
        self.updated_at = Utc::now();
        self.last_seen_at = Utc::now();
        self.error_message = None;
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
}

impl VolumeType {
    pub fn from_mount_type(mount_type: &crate::volume::MountType) -> Self {
        match mount_type {
            crate::volume::MountType::System => VolumeType::System,
            crate::volume::MountType::External => VolumeType::External,
            crate::volume::MountType::Network => VolumeType::Network,
        }
    }
}

impl MountType {
    pub fn from_runtime_mount_type(mount_type: &crate::volume::MountType) -> Self {
        match mount_type {
            crate::volume::MountType::System => MountType::System,
            crate::volume::MountType::External => MountType::External,
            crate::volume::MountType::Network => MountType::Network,
        }
    }
}

impl DiskType {
    pub fn from_runtime_disk_type(disk_type: &crate::volume::DiskType) -> Self {
        match disk_type {
            crate::volume::DiskType::SSD => DiskType::SSD,
            crate::volume::DiskType::HDD => DiskType::HDD,
            crate::volume::DiskType::Network => DiskType::Network,
            crate::volume::DiskType::Virtual => DiskType::Virtual,
            crate::volume::DiskType::Unknown => DiskType::Unknown,
        }
    }
}

impl FileSystem {
    pub fn from_runtime_filesystem(fs: &crate::volume::FileSystem) -> Self {
        match fs {
            crate::volume::FileSystem::APFS => FileSystem::APFS,
            crate::volume::FileSystem::NTFS => FileSystem::NTFS,
            crate::volume::FileSystem::Ext4 => FileSystem::Ext4,
            crate::volume::FileSystem::Btrfs => FileSystem::Btrfs,
            crate::volume::FileSystem::ZFS => FileSystem::ZFS,
            crate::volume::FileSystem::ReFS => FileSystem::ReFS,
            crate::volume::FileSystem::FAT32 => FileSystem::FAT32,
            crate::volume::FileSystem::ExFAT => FileSystem::ExFAT,
            crate::volume::FileSystem::Other(name) => FileSystem::Other(name.clone()),
        }
    }
    
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
            VolumeType::System => write!(f, "System"),
            VolumeType::Internal => write!(f, "Internal"),
            VolumeType::External => write!(f, "External"),
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
        let volume = Volume::new(
            Uuid::new_v4(),
            "test-fingerprint".to_string(),
            "Test Volume".to_string(),
            PathBuf::from("/mnt/test"),
        );
        
        assert_eq!(volume.name, "Test Volume");
        assert_eq!(volume.fingerprint, "test-fingerprint");
        assert_eq!(volume.display_name(), "Test Volume");
        assert!(!volume.is_tracked);
        assert!(!volume.is_favorite);
    }
    
    #[test]
    fn test_volume_tracking() {
        let mut volume = Volume::new(
            Uuid::new_v4(),
            "test".to_string(),
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
        assert_eq!(FileSystem::from_string("unknown"), FileSystem::Other("unknown".to_string()));
        
        assert_eq!(FileSystem::APFS.to_string(), "APFS");
        assert_eq!(FileSystem::Ext4.to_string(), "ext4");
    }
    
    #[test]
    fn test_utilization_calculation() {
        let mut volume = Volume::new(
            Uuid::new_v4(),
            "test".to_string(),
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
        let mut volume = Volume::new(
            Uuid::new_v4(),
            "test".to_string(),
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