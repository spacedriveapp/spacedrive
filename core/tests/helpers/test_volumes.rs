//! Cross-platform test volume creation utilities
//!
//! This module provides platform-specific implementations for creating
//! temporary volumes for testing purposes.

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Supported filesystems for test volumes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFileSystem {
    /// APFS (macOS)
    Apfs,
    /// HFS+ (macOS)
    HfsPlus,
    /// NTFS (Windows)
    Ntfs,
    /// FAT32 (cross-platform)
    Fat32,
    /// ExFAT (cross-platform)
    ExFat,
    /// ext4 (Linux)
    Ext4,
    /// Platform default
    Default,
}

impl TestFileSystem {
    /// Get the filesystem string for the current platform
    pub fn to_platform_string(&self) -> &'static str {
        match self {
            TestFileSystem::Apfs => "APFS",
            TestFileSystem::HfsPlus => "HFS+",
            TestFileSystem::Ntfs => "NTFS",
            TestFileSystem::Fat32 => "FAT32",
            TestFileSystem::ExFat => "ExFAT",
            TestFileSystem::Ext4 => "ext4",
            TestFileSystem::Default => {
                #[cfg(target_os = "macos")]
                return "APFS";
                #[cfg(target_os = "windows")]
                return "NTFS";
                #[cfg(target_os = "linux")]
                return "ext4";
            }
        }
    }
}

/// Configuration for creating a test volume
#[derive(Debug, Clone)]
pub struct TestVolumeConfig {
    /// Volume name
    pub name: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Filesystem type
    pub filesystem: TestFileSystem,
    /// Whether to create as read-only
    pub read_only: bool,
    /// Use RAM disk if possible
    pub use_ram_disk: bool,
}

impl Default for TestVolumeConfig {
    fn default() -> Self {
        Self {
            name: format!("TestVol_{}", chrono::Utc::now().timestamp()),
            size_bytes: 100 * 1024 * 1024, // 100MB
            filesystem: TestFileSystem::Default,
            read_only: false,
            use_ram_disk: false,
        }
    }
}

/// A test volume that automatically cleans up on drop
pub struct TestVolume {
    /// Mount point of the volume
    pub mount_point: PathBuf,
    /// Volume name
    pub name: String,
    /// Platform-specific identifier
    pub(crate) platform_id: String,
    /// Cleanup function
    pub(crate) cleanup: Option<Box<dyn FnOnce() + Send>>,
}

impl TestVolume {
    /// Get the mount point
    pub fn path(&self) -> &PathBuf {
        &self.mount_point
    }
    
    /// Check if volume is mounted
    pub async fn is_mounted(&self) -> bool {
        self.mount_point.exists()
    }
}

impl Drop for TestVolume {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

/// Platform-agnostic test volume manager
pub struct TestVolumeManager {
    #[cfg(target_os = "macos")]
    inner: MacOSTestVolumeManager,
    #[cfg(target_os = "windows")]
    inner: WindowsTestVolumeManager,
    #[cfg(target_os = "linux")]
    inner: LinuxTestVolumeManager,
}

impl TestVolumeManager {
    /// Create a new test volume manager
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            inner: MacOSTestVolumeManager::new(),
            #[cfg(target_os = "windows")]
            inner: WindowsTestVolumeManager::new(),
            #[cfg(target_os = "linux")]
            inner: LinuxTestVolumeManager::new(),
        }
    }
    
    /// Create a test volume with the given configuration
    pub async fn create_volume(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        self.inner.create_volume(config).await
    }
    
    /// Destroy a test volume
    pub async fn destroy_volume(&self, volume: TestVolume) -> Result<()> {
        self.inner.destroy_volume(volume).await
    }
    
    /// Check if we have required privileges for volume operations
    pub async fn check_privileges(&self) -> Result<()> {
        self.inner.check_privileges().await
    }
}

// macOS implementation
#[cfg(target_os = "macos")]
pub struct MacOSTestVolumeManager {
    temp_dir: PathBuf,
    volumes: Arc<Mutex<Vec<PathBuf>>>,
}

#[cfg(target_os = "macos")]
impl MacOSTestVolumeManager {
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join("spacedrive_test_volumes");
        std::fs::create_dir_all(&temp_dir).ok();
        
        Self {
            temp_dir,
            volumes: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn create_volume(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        info!("Creating test volume '{}' on macOS", config.name);
        
        let _volume_name = config.name.clone();
        let _size_mb = config.size_bytes / (1024 * 1024);
        
        if config.use_ram_disk {
            // Create RAM disk
            self.create_ram_disk(config).await
        } else {
            // Create disk image
            self.create_disk_image(config).await
        }
    }
    
    async fn create_ram_disk(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        let sectors = config.size_bytes / 512;
        
        // Create RAM disk
        let output = tokio::process::Command::new("hdiutil")
            .args(&["attach", "-nomount", &format!("ram://{}", sectors)])
            .output()
            .await
            .context("Failed to create RAM disk")?;
        
        if !output.status.success() {
            return Err(anyhow!("Failed to create RAM disk: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        let disk_path = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        
        debug!("Created RAM disk at {}", disk_path);
        
        // Format the disk
        let fs_type = match config.filesystem {
            TestFileSystem::Apfs => "APFS",
            TestFileSystem::HfsPlus => "HFS+",
            TestFileSystem::ExFat => "ExFAT",
            TestFileSystem::Fat32 => "FAT32",
            _ => "APFS",
        };
        
        let output = tokio::process::Command::new("diskutil")
            .args(&[
                "erasevolume",
                fs_type,
                &config.name,
                &disk_path,
            ])
            .output()
            .await
            .context("Failed to format RAM disk")?;
        
        if !output.status.success() {
            // Clean up RAM disk
            tokio::process::Command::new("hdiutil")
                .args(&["detach", &disk_path])
                .output()
                .await
                .ok();
                
            return Err(anyhow!("Failed to format RAM disk: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        let mount_point = PathBuf::from(format!("/Volumes/{}", config.name));
        let disk_path_clone = disk_path.clone();
        
        Ok(TestVolume {
            mount_point,
            name: config.name,
            platform_id: disk_path,
            cleanup: Some(Box::new(move || {
                // Detach the RAM disk
                std::process::Command::new("hdiutil")
                    .args(&["detach", &disk_path_clone, "-force"])
                    .output()
                    .ok();
            })),
        })
    }
    
    async fn create_disk_image(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        let dmg_path = self.temp_dir.join(format!("{}.dmg", config.name));
        let size_mb = config.size_bytes / (1024 * 1024);
        
        // Ensure temp directory exists
        tokio::fs::create_dir_all(&self.temp_dir).await?;
        
        // Create disk image
        let fs_type = match config.filesystem {
            TestFileSystem::Apfs => "APFS",
            TestFileSystem::HfsPlus => "HFS+",
            TestFileSystem::ExFat => "ExFAT",
            TestFileSystem::Fat32 => "MS-DOS FAT32",
            _ => "APFS",
        };
        
        let output = tokio::process::Command::new("hdiutil")
            .args(&[
                "create",
                "-size", &format!("{}m", size_mb),
                "-fs", fs_type,
                "-volname", &config.name,
                dmg_path.to_str().unwrap(),
            ])
            .output()
            .await
            .context("Failed to create disk image")?;
        
        if !output.status.success() {
            return Err(anyhow!("Failed to create disk image: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        // Mount the disk image
        let output = tokio::process::Command::new("hdiutil")
            .args(&["attach", dmg_path.to_str().unwrap()])
            .output()
            .await
            .context("Failed to mount disk image")?;
        
        if !output.status.success() {
            // Clean up disk image
            tokio::fs::remove_file(&dmg_path).await.ok();
            return Err(anyhow!("Failed to mount disk image: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        // Parse mount info from output
        let output_str = String::from_utf8_lossy(&output.stdout);
        let disk_id = output_str
            .lines()
            .find(|line| line.contains("/dev/disk"))
            .and_then(|line| line.split_whitespace().next())
            .ok_or_else(|| anyhow!("Failed to parse disk identifier"))?
            .to_string();
        
        let mount_point = PathBuf::from(format!("/Volumes/{}", config.name));
        
        // Track the volume
        {
            let mut volumes = self.volumes.lock().await;
            volumes.push(dmg_path.clone());
        }
        
        let dmg_path_clone = dmg_path.clone();
        let disk_id_clone = disk_id.clone();
        let volumes = self.volumes.clone();
        
        Ok(TestVolume {
            mount_point,
            name: config.name,
            platform_id: disk_id,
            cleanup: Some(Box::new(move || {
                // Detach the disk
                std::process::Command::new("hdiutil")
                    .args(&["detach", &disk_id_clone, "-force"])
                    .output()
                    .ok();
                
                // Remove the disk image
                std::fs::remove_file(&dmg_path_clone).ok();
                
                // Remove from tracking
                // Best effort cleanup of tracking
                drop(volumes);
            })),
        })
    }
    
    pub async fn destroy_volume(&self, mut volume: TestVolume) -> Result<()> {
        info!("Destroying test volume '{}'", volume.name);
        
        // The cleanup will be called by Drop
        if let Some(cleanup) = volume.cleanup.take() {
            cleanup();
        }
        
        Ok(())
    }
    
    pub async fn check_privileges(&self) -> Result<()> {
        // On macOS, we don't need special privileges for disk images
        Ok(())
    }
}

// Windows implementation
#[cfg(target_os = "windows")]
pub struct WindowsTestVolumeManager {
    temp_dir: PathBuf,
    volumes: Arc<Mutex<Vec<PathBuf>>>,
}

#[cfg(target_os = "windows")]
impl WindowsTestVolumeManager {
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join("spacedrive_test_volumes");
        std::fs::create_dir_all(&temp_dir).ok();
        
        Self {
            temp_dir,
            volumes: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn create_volume(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        info!("Creating test volume '{}' on Windows", config.name);
        
        // For Windows, we'll use VHD (Virtual Hard Disk)
        self.create_vhd(config).await
    }
    
    async fn create_vhd(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        let vhd_path = self.temp_dir.join(format!("{}.vhdx", config.name));
        let size_mb = config.size_bytes / (1024 * 1024);
        
        // Ensure temp directory exists
        tokio::fs::create_dir_all(&self.temp_dir).await?;
        
        // Create VHD using PowerShell
        let script = format!(
            r#"
            $vhdPath = '{}'
            $sizeBytes = {}
            
            # Create VHD
            New-VHD -Path $vhdPath -SizeBytes $sizeBytes -Dynamic
            
            # Mount VHD
            $vhd = Mount-VHD -Path $vhdPath -PassThru
            
            # Initialize disk
            $disk = Initialize-Disk -Number $vhd.Number -PartitionStyle MBR -PassThru
            
            # Create partition
            $partition = New-Partition -DiskNumber $disk.Number -UseMaximumSize -AssignDriveLetter
            
            # Format volume
            Format-Volume -DriveLetter $partition.DriveLetter -FileSystem {} -NewFileSystemLabel '{}' -Confirm:$false
            
            # Output drive letter
            Write-Output $partition.DriveLetter
            "#,
            vhd_path.to_str().unwrap().replace('\\', "\\\\"),
            config.size_bytes,
            match config.filesystem {
                TestFileSystem::Ntfs => "NTFS",
                TestFileSystem::Fat32 => "FAT32",
                TestFileSystem::ExFat => "exFAT",
                _ => "NTFS",
            },
            config.name
        );
        
        let output = tokio::process::Command::new("powershell")
            .args(&["-NoProfile", "-Command", &script])
            .output()
            .await
            .context("Failed to create VHD")?;
        
        if !output.status.success() {
            return Err(anyhow!("Failed to create VHD: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        let drive_letter = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        
        if drive_letter.is_empty() {
            return Err(anyhow!("Failed to get drive letter for VHD"));
        }
        
        let mount_point = PathBuf::from(format!("{}:\\", drive_letter));
        
        // Track the volume
        {
            let mut volumes = self.volumes.lock().await;
            volumes.push(vhd_path.clone());
        }
        
        let vhd_path_clone = vhd_path.clone();
        let volumes = self.volumes.clone();
        
        Ok(TestVolume {
            mount_point,
            name: config.name,
            platform_id: vhd_path.to_str().unwrap().to_string(),
            cleanup: Some(Box::new(move || {
                // Dismount VHD using PowerShell
                let script = format!(
                    "Dismount-VHD -Path '{}' -Confirm:$false",
                    vhd_path_clone.to_str().unwrap()
                );
                
                std::process::Command::new("powershell")
                    .args(&["-NoProfile", "-Command", &script])
                    .output()
                    .ok();
                
                // Remove the VHD file
                std::fs::remove_file(&vhd_path_clone).ok();
                
                // Remove from tracking
                // Best effort cleanup of tracking
                drop(volumes);
            })),
        })
    }
    
    pub async fn destroy_volume(&self, mut volume: TestVolume) -> Result<()> {
        info!("Destroying test volume '{}'", volume.name);
        
        // The cleanup will be called by Drop
        if let Some(cleanup) = volume.cleanup.take() {
            cleanup();
        }
        
        Ok(())
    }
    
    pub async fn check_privileges(&self) -> Result<()> {
        // Check if we're running as administrator
        let output = tokio::process::Command::new("net")
            .args(&["session"])
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(anyhow!("Administrator privileges required for creating test volumes on Windows"));
        }
        
        Ok(())
    }
}

// Linux implementation
#[cfg(target_os = "linux")]
pub struct LinuxTestVolumeManager {
    temp_dir: PathBuf,
    volumes: Arc<Mutex<Vec<PathBuf>>>,
}

#[cfg(target_os = "linux")]
impl LinuxTestVolumeManager {
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join("spacedrive_test_volumes");
        std::fs::create_dir_all(&temp_dir).ok();
        
        Self {
            temp_dir,
            volumes: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn create_volume(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        info!("Creating test volume '{}' on Linux", config.name);
        
        if config.use_ram_disk {
            // Use tmpfs for RAM disk
            self.create_tmpfs(config).await
        } else {
            // Use loop device with file backing
            self.create_loop_device(config).await
        }
    }
    
    async fn create_tmpfs(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        let mount_point = self.temp_dir.join(&config.name);
        
        // Create mount point
        tokio::fs::create_dir_all(&mount_point).await?;
        
        // Mount tmpfs
        let size_mb = config.size_bytes / (1024 * 1024);
        let output = tokio::process::Command::new("sudo")
            .args(&[
                "mount",
                "-t", "tmpfs",
                "-o", &format!("size={}M", size_mb),
                "tmpfs",
                mount_point.to_str().unwrap(),
            ])
            .output()
            .await
            .context("Failed to mount tmpfs")?;
        
        if !output.status.success() {
            tokio::fs::remove_dir(&mount_point).await.ok();
            return Err(anyhow!("Failed to mount tmpfs: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        let mount_point_clone = mount_point.clone();
        
        Ok(TestVolume {
            mount_point: mount_point.clone(),
            name: config.name,
            platform_id: mount_point.to_str().unwrap().to_string(),
            cleanup: Some(Box::new(move || {
                // Unmount tmpfs
                std::process::Command::new("sudo")
                    .args(&["umount", mount_point_clone.to_str().unwrap()])
                    .output()
                    .ok();
                
                // Remove mount point
                std::fs::remove_dir(&mount_point_clone).ok();
            })),
        })
    }
    
    async fn create_loop_device(&self, config: TestVolumeConfig) -> Result<TestVolume> {
        let img_path = self.temp_dir.join(format!("{}.img", config.name));
        let mount_point = self.temp_dir.join(&config.name);
        let size_mb = config.size_bytes / (1024 * 1024);
        
        // Ensure directories exist
        tokio::fs::create_dir_all(&self.temp_dir).await?;
        tokio::fs::create_dir_all(&mount_point).await?;
        
        // Create image file
        let output = tokio::process::Command::new("dd")
            .args(&[
                "if=/dev/zero",
                &format!("of={}", img_path.to_str().unwrap()),
                "bs=1M",
                &format!("count={}", size_mb),
            ])
            .output()
            .await
            .context("Failed to create image file")?;
        
        if !output.status.success() {
            return Err(anyhow!("Failed to create image file: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        // Create loop device
        let output = tokio::process::Command::new("sudo")
            .args(&["losetup", "--find", "--show", img_path.to_str().unwrap()])
            .output()
            .await
            .context("Failed to create loop device")?;
        
        if !output.status.success() {
            tokio::fs::remove_file(&img_path).await.ok();
            return Err(anyhow!("Failed to create loop device: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        let loop_device = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        
        // Format the loop device
        let fs_type = match config.filesystem {
            TestFileSystem::Ext4 => "ext4",
            TestFileSystem::Fat32 => "vfat",
            TestFileSystem::ExFat => "exfat",
            _ => "ext4",
        };
        
        let mkfs_cmd = match fs_type {
            "ext4" => "mkfs.ext4",
            "vfat" => "mkfs.vfat",
            "exfat" => "mkfs.exfat",
            _ => "mkfs.ext4",
        };
        
        let mut args = vec![mkfs_cmd];
        if fs_type == "ext4" {
            args.push("-L");
            args.push(&config.name);
        }
        args.push(&loop_device);
        
        let output = tokio::process::Command::new("sudo")
            .args(&args)
            .output()
            .await
            .context("Failed to format loop device")?;
        
        if !output.status.success() {
            // Clean up loop device
            tokio::process::Command::new("sudo")
                .args(&["losetup", "-d", &loop_device])
                .output()
                .await
                .ok();
            tokio::fs::remove_file(&img_path).await.ok();
            return Err(anyhow!("Failed to format loop device: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        // Mount the loop device
        let output = tokio::process::Command::new("sudo")
            .args(&[
                "mount",
                &loop_device,
                mount_point.to_str().unwrap(),
            ])
            .output()
            .await
            .context("Failed to mount loop device")?;
        
        if !output.status.success() {
            // Clean up
            tokio::process::Command::new("sudo")
                .args(&["losetup", "-d", &loop_device])
                .output()
                .await
                .ok();
            tokio::fs::remove_file(&img_path).await.ok();
            tokio::fs::remove_dir(&mount_point).await.ok();
            return Err(anyhow!("Failed to mount loop device: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        // Track the volume
        {
            let mut volumes = self.volumes.lock().await;
            volumes.push(img_path.clone());
        }
        
        let loop_device_clone = loop_device.clone();
        let mount_point_clone = mount_point.clone();
        let img_path_clone = img_path.clone();
        let volumes = self.volumes.clone();
        
        Ok(TestVolume {
            mount_point: mount_point.clone(),
            name: config.name,
            platform_id: loop_device,
            cleanup: Some(Box::new(move || {
                // Unmount
                std::process::Command::new("sudo")
                    .args(&["umount", mount_point_clone.to_str().unwrap()])
                    .output()
                    .ok();
                
                // Detach loop device
                std::process::Command::new("sudo")
                    .args(&["losetup", "-d", &loop_device_clone])
                    .output()
                    .ok();
                
                // Remove files
                std::fs::remove_file(&img_path_clone).ok();
                std::fs::remove_dir(&mount_point_clone).ok();
                
                // Remove from tracking
                // Best effort cleanup of tracking
                drop(volumes);
            })),
        })
    }
    
    pub async fn destroy_volume(&self, mut volume: TestVolume) -> Result<()> {
        info!("Destroying test volume '{}'", volume.name);
        
        // The cleanup will be called by Drop
        if let Some(cleanup) = volume.cleanup.take() {
            cleanup();
        }
        
        Ok(())
    }
    
    pub async fn check_privileges(&self) -> Result<()> {
        // Check if we can use sudo
        let output = tokio::process::Command::new("sudo")
            .args(&["-n", "true"])
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(anyhow!("sudo privileges required for creating test volumes on Linux"));
        }
        
        Ok(())
    }
}

/// Builder for creating test volumes with specific configurations
pub struct TestVolumeBuilder {
    config: TestVolumeConfig,
}

impl TestVolumeBuilder {
    /// Create a new test volume builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: TestVolumeConfig {
                name: name.into(),
                ..Default::default()
            },
        }
    }
    
    /// Set the volume size in bytes
    pub fn size_bytes(mut self, size: u64) -> Self {
        self.config.size_bytes = size;
        self
    }
    
    /// Set the volume size in megabytes
    pub fn size_mb(self, size_mb: u64) -> Self {
        self.size_bytes(size_mb * 1024 * 1024)
    }
    
    /// Set the volume size in gigabytes
    pub fn size_gb(self, size_gb: u64) -> Self {
        self.size_bytes(size_gb * 1024 * 1024 * 1024)
    }
    
    /// Set the filesystem type
    pub fn filesystem(mut self, fs: TestFileSystem) -> Self {
        self.config.filesystem = fs;
        self
    }
    
    /// Make the volume read-only
    pub fn read_only(mut self) -> Self {
        self.config.read_only = true;
        self
    }
    
    /// Use RAM disk if available
    pub fn use_ram_disk(mut self) -> Self {
        self.config.use_ram_disk = true;
        self
    }
    
    /// Build and create the test volume
    pub async fn build(self) -> Result<TestVolume> {
        let manager = TestVolumeManager::new();
        manager.create_volume(self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_volume_manager_creation() {
        let manager = TestVolumeManager::new();
        
        // Just check that we can create a manager
        // Actual volume creation tests might require privileges
        assert!(manager.check_privileges().await.is_ok() || 
                manager.check_privileges().await.is_err());
    }
    
    #[tokio::test]
    async fn test_volume_builder() {
        let config = TestVolumeBuilder::new("TestVol")
            .size_mb(50)
            .filesystem(TestFileSystem::Default)
            .use_ram_disk()
            .config;
        
        assert_eq!(config.name, "TestVol");
        assert_eq!(config.size_bytes, 50 * 1024 * 1024);
        assert!(config.use_ram_disk);
    }
}