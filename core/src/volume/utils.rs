//! Shared utilities for volume detection across platforms

use crate::{
	domain::volume::{SpacedriveVolumeId, SPACEDRIVE_VOLUME_ID_FILE},
	volume::{
		error::{VolumeError, VolumeResult},
		types::FileSystem,
	},
};
use std::path::Path;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Parse size strings from df output (e.g., "1.5G", "931Gi", "1024K")
pub fn parse_size_string(size_str: &str) -> VolumeResult<u64> {
	if size_str == "-" {
		return Ok(0);
	}

	// Skip invalid size strings that don't look like numbers
	if size_str.is_empty() || size_str.chars().all(char::is_alphabetic) {
		return Ok(0);
	}

	let size_str = size_str.replace(",", ""); // Remove commas
	let (number_part, unit) = if let Some(pos) = size_str.find(char::is_alphabetic) {
		(&size_str[..pos], &size_str[pos..])
	} else {
		(size_str.as_str(), "")
	};

	let number: f64 = number_part
		.parse()
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

/// Check if a filesystem should be considered a system filesystem
pub fn is_system_filesystem(filesystem: &str) -> bool {
	matches!(
		filesystem,
		"/" | "/dev" | "/proc" | "/sys" | "/tmp" | "/var/tmp"
	)
}

/// Check if a filesystem is virtual (not backed by physical storage)
pub fn is_virtual_filesystem(filesystem: &str) -> bool {
	let fs_lower = filesystem.to_lowercase();
	matches!(
		fs_lower.as_str(),
		"devfs" | "sysfs" | "proc" | "tmpfs" | "ramfs" | "devtmpfs" | "overlay" | "fuse"
	) || fs_lower.starts_with("map ")
		|| fs_lower.contains("auto_")
}

/// Parse filesystem type from string to FileSystem enum
pub fn parse_filesystem_type(fs_type: &str) -> FileSystem {
	match fs_type.to_lowercase().as_str() {
		"apfs" => FileSystem::APFS,
		"btrfs" => FileSystem::Btrfs,
		"zfs" => FileSystem::ZFS,
		"refs" => FileSystem::ReFS,
		"ntfs" => FileSystem::NTFS,
		"ext2" | "ext3" | "ext4" => FileSystem::Ext4,
		"xfs" => FileSystem::Other("XFS".to_string()),
		"fat32" | "vfat" => FileSystem::FAT32,
		"exfat" => FileSystem::ExFAT,
		"hfs+" | "hfsplus" => FileSystem::Other("HFS+".to_string()),
		_ => FileSystem::Other(fs_type.to_string()),
	}
}

/// Read or create .spacedrive-volume-id file on external volumes (async version)
/// This file provides persistent identification for removable drives
/// Returns the UUID from the file, or None if the volume is read-only
pub async fn read_or_create_dotfile(
	mount_point: &Path,
	device_id: Uuid,
	library_id: Option<Uuid>,
) -> Option<Uuid> {
	let id_file_path = mount_point.join(SPACEDRIVE_VOLUME_ID_FILE);

	// Try to read existing dotfile
	if let Ok(content) = tokio::fs::read_to_string(&id_file_path).await {
		if let Ok(spacedrive_id) = serde_json::from_str::<SpacedriveVolumeId>(&content) {
			debug!(
				"Found existing dotfile ID: {} at {}",
				spacedrive_id.id,
				id_file_path.display()
			);
			return Some(spacedrive_id.id);
		}
	}

	// Try to create new dotfile (if writable)
	if !mount_point.exists() {
		return None;
	}

	let spacedrive_id = SpacedriveVolumeId {
		id: Uuid::new_v4(),
		created: chrono::Utc::now(),
		device_name: None,
		volume_name: mount_point
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "Unknown".to_string()),
		device_id,
		library_id: library_id.unwrap_or(Uuid::nil()),
	};

	if let Ok(content) = serde_json::to_string_pretty(&spacedrive_id) {
		match tokio::fs::write(&id_file_path, content).await {
			Ok(()) => {
				info!(
					"Created dotfile with ID: {} at {}",
					spacedrive_id.id,
					id_file_path.display()
				);
				return Some(spacedrive_id.id);
			}
			Err(e) => {
				debug!(
					"Could not write dotfile to {}: {}",
					id_file_path.display(),
					e
				);
			}
		}
	}

	None
}

/// Read or create .spacedrive-volume-id file on external volumes (sync version)
/// This file provides persistent identification for removable drives
/// Returns the UUID from the file, or None if the volume is read-only
pub fn read_or_create_dotfile_sync(
	mount_point: &Path,
	device_id: Uuid,
	library_id: Option<Uuid>,
) -> Option<Uuid> {
	let id_file_path = mount_point.join(SPACEDRIVE_VOLUME_ID_FILE);

	// Try to read existing dotfile
	if let Ok(content) = std::fs::read_to_string(&id_file_path) {
		if let Ok(spacedrive_id) = serde_json::from_str::<SpacedriveVolumeId>(&content) {
			debug!(
				"Found existing dotfile ID: {} at {}",
				spacedrive_id.id,
				id_file_path.display()
			);
			return Some(spacedrive_id.id);
		}
	}

	// Try to create new dotfile (if writable)
	if !mount_point.exists() {
		return None;
	}

	let spacedrive_id = SpacedriveVolumeId {
		id: Uuid::new_v4(),
		created: chrono::Utc::now(),
		device_name: None,
		volume_name: mount_point
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "Unknown".to_string()),
		device_id,
		library_id: library_id.unwrap_or(Uuid::nil()),
	};

	if let Ok(content) = serde_json::to_string_pretty(&spacedrive_id) {
		match std::fs::write(&id_file_path, content) {
			Ok(()) => {
				info!(
					"Created dotfile with ID: {} at {}",
					spacedrive_id.id,
					id_file_path.display()
				);
				return Some(spacedrive_id.id);
			}
			Err(e) => {
				debug!(
					"Could not write dotfile to {}: {}",
					id_file_path.display(),
					e
				);
			}
		}
	}

	None
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
		assert_eq!(
			parse_size_string("1.5G").unwrap(),
			(1.5 * 1024.0 * 1024.0 * 1024.0) as u64
		);
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

	#[test]
	fn test_parse_filesystem_type() {
		assert!(matches!(parse_filesystem_type("apfs"), FileSystem::APFS));
		assert!(matches!(parse_filesystem_type("btrfs"), FileSystem::Btrfs));
		assert!(matches!(parse_filesystem_type("ext4"), FileSystem::Ext4));
		assert!(matches!(
			parse_filesystem_type("unknown"),
			FileSystem::Other(_)
		));
	}
}
