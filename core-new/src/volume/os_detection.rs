//! Platform-specific volume detection

use crate::volume::{
    error::{VolumeError, VolumeResult},
    types::{DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig},
};
use std::path::PathBuf;
use tokio::task;
use tracing::{debug, warn, instrument};

/// Detect all volumes on the current system
#[instrument(skip(config))]
pub async fn detect_volumes(config: &VolumeDetectionConfig) -> VolumeResult<Vec<Volume>> {
    debug!("Starting volume detection");
    
    #[cfg(target_os = "macos")]
    let volumes = macos::detect_volumes(config).await?;
    
    #[cfg(target_os = "linux")]
    let volumes = linux::detect_volumes(config).await?;
    
    #[cfg(target_os = "windows")]
    let volumes = windows::detect_volumes(config).await?;
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let volumes = unsupported::detect_volumes(config).await?;
    
    debug!("Detected {} volumes", volumes.len());
    Ok(volumes)
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::process::Command;
    
    pub async fn detect_volumes(config: &VolumeDetectionConfig) -> VolumeResult<Vec<Volume>> {
        // Clone config for move into task
        let config = config.clone();
        
        // Run in blocking task since Command is sync
        task::spawn_blocking(move || {
            let mut volumes = Vec::new();
            
            // Use diskutil to get volume information
            let output = Command::new("diskutil")
                .args(["list", "-plist"])
                .output()
                .map_err(|e| VolumeError::platform(format!("Failed to run diskutil: {}", e)))?;
            
            if !output.status.success() {
                return Err(VolumeError::platform("diskutil command failed"));
            }
            
            // For now, use simple df command to get basic volume info
            let df_output = Command::new("df")
                .args(["-h"])
                .output()
                .map_err(|e| VolumeError::platform(format!("Failed to run df: {}", e)))?;
            
            let df_text = String::from_utf8_lossy(&df_output.stdout);
            
            for line in df_text.lines().skip(1) { // Skip header
                if let Some(volume) = parse_df_line(line, config)? {
                    volumes.push(volume);
                }
            }
            
            Ok(volumes)
        })
        .await
        .map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
    }
    
    fn parse_df_line(line: &str, config: &VolumeDetectionConfig) -> VolumeResult<Option<Volume>> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            return Ok(None);
        }
        
        let filesystem = parts[0];
        let size_str = parts[1];
        let used_str = parts[2];
        let available_str = parts[3];
        let mount_point = parts[8];
        
        // Skip system filesystems unless requested
        if !config.include_system && is_system_filesystem(filesystem) {
            return Ok(None);
        }
        
        // Skip virtual filesystems unless requested
        if !config.include_virtual && is_virtual_filesystem(filesystem) {
            return Ok(None);
        }
        
        let mount_path = PathBuf::from(mount_point);
        
        // Parse sizes (df output like "931Gi", "465Gi", etc.)
        let total_bytes = parse_size_string(size_str)?;
        let available_bytes = parse_size_string(available_str)?;
        
        let name = if mount_point == "/" {
            "Macintosh HD".to_string()
        } else {
            mount_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };
        
        let mount_type = if mount_point == "/" {
            MountType::System
        } else if mount_point.starts_with("/Volumes/") {
            MountType::External
        } else if filesystem.starts_with("//") {
            MountType::Network
        } else {
            MountType::System
        };
        
        let disk_type = detect_disk_type(&mount_path)?;
        let file_system = detect_filesystem(&mount_path)?;
        
        let volume = Volume::new(
            name,
            mount_type,
            mount_path,
            vec![], // Additional mount points would need diskutil parsing
            disk_type,
            file_system,
            total_bytes,
            available_bytes,
            false, // Read-only detection would need additional checks
            Some(filesystem.to_string()), // Use filesystem as hardware ID
        );
        
        Ok(Some(volume))
    }
    
    fn detect_disk_type(mount_point: &PathBuf) -> VolumeResult<DiskType> {
        // Try to detect SSD vs HDD using system_profiler or diskutil
        let output = Command::new("diskutil")
            .args(["info", mount_point.to_str().unwrap_or("/")])
            .output();
            
        match output {
            Ok(output) if output.status.success() => {
                let info = String::from_utf8_lossy(&output.stdout);
                if info.contains("Solid State") {
                    Ok(DiskType::SSD)
                } else if info.contains("Rotational") {
                    Ok(DiskType::HDD)
                } else {
                    Ok(DiskType::Unknown)
                }
            }
            _ => Ok(DiskType::Unknown),
        }
    }
    
    fn detect_filesystem(mount_point: &PathBuf) -> VolumeResult<FileSystem> {
        let output = Command::new("diskutil")
            .args(["info", mount_point.to_str().unwrap_or("/")])
            .output();
            
        match output {
            Ok(output) if output.status.success() => {
                let info = String::from_utf8_lossy(&output.stdout);
                if info.contains("APFS") {
                    Ok(FileSystem::APFS)
                } else if info.contains("HFS+") {
                    Ok(FileSystem::Other("HFS+".to_string()))
                } else if info.contains("ExFAT") {
                    Ok(FileSystem::ExFAT)
                } else if info.contains("FAT32") {
                    Ok(FileSystem::FAT32)
                } else {
                    Ok(FileSystem::Other("Unknown".to_string()))
                }
            }
            _ => Ok(FileSystem::Other("Unknown".to_string())),
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::process::Command;
    
    pub async fn detect_volumes(config: &VolumeDetectionConfig) -> VolumeResult<Vec<Volume>> {
        task::spawn_blocking(move || {
            let mut volumes = Vec::new();
            
            // Use df to get mounted filesystems
            let output = Command::new("df")
                .args(["-h", "-T"]) // -T shows filesystem type
                .output()
                .map_err(|e| VolumeError::platform(format!("Failed to run df: {}", e)))?;
            
            if !output.status.success() {
                return Err(VolumeError::platform("df command failed"));
            }
            
            let df_text = String::from_utf8_lossy(&output.stdout);
            
            for line in df_text.lines().skip(1) { // Skip header
                if let Some(volume) = parse_df_line(line, config)? {
                    volumes.push(volume);
                }
            }
            
            Ok(volumes)
        })
        .await
        .map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
    }
    
    fn parse_df_line(line: &str, config: &VolumeDetectionConfig) -> VolumeResult<Option<Volume>> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            return Ok(None);
        }
        
        let filesystem_device = parts[0];
        let filesystem_type = parts[1];
        let size_str = parts[2];
        let used_str = parts[3];
        let available_str = parts[4];
        let mount_point = parts[6];
        
        // Skip system filesystems unless requested
        if !config.include_system && is_system_filesystem(filesystem_device) {
            return Ok(None);
        }
        
        // Skip virtual filesystems unless requested
        if !config.include_virtual && is_virtual_filesystem(filesystem_type) {
            return Ok(None);
        }
        
        let mount_path = PathBuf::from(mount_point);
        
        let total_bytes = parse_size_string(size_str)?;
        let available_bytes = parse_size_string(available_str)?;
        
        let name = if mount_point == "/" {
            "Root".to_string()
        } else {
            mount_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };
        
        let mount_type = determine_mount_type(mount_point, filesystem_device);
        let disk_type = detect_disk_type_linux(filesystem_device)?;
        let file_system = FileSystem::from_string(filesystem_type);
        
        let volume = Volume::new(
            name,
            mount_type,
            mount_path,
            vec![],
            disk_type,
            file_system,
            total_bytes,
            available_bytes,
            false, // Would need additional check for read-only
            Some(filesystem_device.to_string()),
        );
        
        Ok(Some(volume))
    }
    
    fn detect_disk_type_linux(device: &str) -> VolumeResult<DiskType> {
        // Try to detect using /sys/block/*/queue/rotational
        if let Some(device_name) = device.strip_prefix("/dev/") {
            let base_device = device_name.trim_end_matches(char::is_numeric);
            let rotational_path = format!("/sys/block/{}/queue/rotational", base_device);
            
            if let Ok(contents) = std::fs::read_to_string(rotational_path) {
                return match contents.trim() {
                    "0" => Ok(DiskType::SSD),
                    "1" => Ok(DiskType::HDD),
                    _ => Ok(DiskType::Unknown),
                };
            }
        }
        
        Ok(DiskType::Unknown)
    }
    
    fn determine_mount_type(mount_point: &str, device: &str) -> MountType {
        if mount_point == "/" || mount_point.starts_with("/boot") {
            MountType::System
        } else if device.starts_with("//") || device.contains("nfs") {
            MountType::Network
        } else if mount_point.starts_with("/media/") || mount_point.starts_with("/mnt/") {
            MountType::External
        } else {
            MountType::System
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::process::Command;
    
    pub async fn detect_volumes(_config: &VolumeDetectionConfig) -> VolumeResult<Vec<Volume>> {
        task::spawn_blocking(|| {
            // Use PowerShell to get volume information
            let output = Command::new("powershell")
                .args([
                    "-Command",
                    "Get-Volume | Select-Object DriveLetter,FileSystemLabel,Size,SizeRemaining,FileSystem | ConvertTo-Json"
                ])
                .output()
                .map_err(|e| VolumeError::platform(format!("Failed to run PowerShell: {}", e)))?;
            
            if !output.status.success() {
                return Err(VolumeError::platform("PowerShell command failed"));
            }
            
            // For now, return empty until we implement full Windows support
            warn!("Windows volume detection not fully implemented yet");
            Ok(Vec::new())
        })
        .await
        .map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod unsupported {
    use super::*;
    
    pub async fn detect_volumes(_config: &VolumeDetectionConfig) -> VolumeResult<Vec<Volume>> {
        warn!("Volume detection not supported on this platform");
        Ok(Vec::new())
    }
}

// Common utility functions
fn is_system_filesystem(filesystem: &str) -> bool {
    matches!(filesystem, "/" | "/dev" | "/proc" | "/sys" | "/tmp" | "/var/tmp")
}

fn is_virtual_filesystem(filesystem: &str) -> bool {
    matches!(
        filesystem.to_lowercase().as_str(),
        "devfs" | "sysfs" | "proc" | "tmpfs" | "ramfs" | "devtmpfs" | "overlay" | "fuse"
    )
}

fn parse_size_string(size_str: &str) -> VolumeResult<u64> {
    if size_str == "-" {
        return Ok(0);
    }
    
    let size_str = size_str.replace(",", ""); // Remove commas
    let (number_part, unit) = if let Some(pos) = size_str.find(char::is_alphabetic) {
        (&size_str[..pos], &size_str[pos..])
    } else {
        (size_str.as_str(), "")
    };
    
    let number: f64 = number_part.parse()
        .map_err(|_| VolumeError::InvalidData(format!("Invalid size: {}", size_str)))?;
    
    let multiplier = match unit.to_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" | "KI" => 1024,
        "M" | "MB" | "MI" => 1024 * 1024,
        "G" | "GB" | "GI" => 1024 * 1024 * 1024,
        "T" | "TB" | "TI" => 1024u64.pow(4),
        "P" | "PB" | "PI" => 1024u64.pow(5),
        _ => {
            warn!("Unknown size unit: {}", unit);
            1
        }
    };
    
    Ok((number * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_string() {
        assert_eq!(parse_size_string("1024").unwrap(), 1024);
        assert_eq!(parse_size_string("1K").unwrap(), 1024);
        assert_eq!(parse_size_string("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_size_string("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size_string("1.5G").unwrap(), (1.5 * 1024.0 * 1024.0 * 1024.0) as u64);
        assert_eq!(parse_size_string("-").unwrap(), 0);
    }

    #[test]
    fn test_filesystem_detection() {
        assert!(is_virtual_filesystem("tmpfs"));
        assert!(is_virtual_filesystem("proc"));
        assert!(!is_virtual_filesystem("ext4"));
        
        assert!(is_system_filesystem("/"));
        assert!(is_system_filesystem("/proc"));
        assert!(!is_system_filesystem("/home"));
    }
}