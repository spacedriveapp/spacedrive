//! macOS-specific volume detection helpers

use crate::volume::{
	classification::{get_classifier, VolumeDetectionInfo},
	error::{VolumeError, VolumeResult},
	types::{DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig, VolumeFingerprint},
	utils,
};
use std::path::PathBuf;
use std::process::Command;
use tokio::task;
use tracing::debug;
use uuid::Uuid;

/// Detect non-APFS volumes using traditional df method
pub async fn detect_non_apfs_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	let config = config.clone(); // Clone to move into async block
	task::spawn_blocking(move || {
		let mut volumes = Vec::new();

		// Use df to get mounted filesystems
		let df_output = Command::new("df")
			.args(["-H"])
			.output()
			.map_err(|e| VolumeError::platform(format!("Failed to run df: {}", e)))?;

		if !df_output.status.success() {
			return Ok(volumes); // Return empty if df fails
		}

		let df_stdout = String::from_utf8_lossy(&df_output.stdout);
		for line in df_stdout.lines().skip(1) {
			// Skip header
			let fields: Vec<&str> = line.split_whitespace().collect();
			if fields.len() >= 9 {
				let filesystem = fields[0];
				let mount_point = fields[8..].join(" ");

				// Skip APFS filesystems (already handled by APFS detection)
				if filesystem.starts_with("/dev/disk") && mount_point.starts_with("/") {
					continue; // Skip APFS volumes
				}

				// Skip certain filesystems
				if should_skip_filesystem(filesystem) {
					debug!("Skipping {} filesystem: {}", filesystem, mount_point);
					continue;
				}

				// Skip system filesystems unless requested
				if !config.include_system && utils::is_system_filesystem(filesystem) {
					continue;
				}

				// Skip virtual filesystems unless requested
				if !config.include_virtual && utils::is_virtual_filesystem(filesystem) {
					continue;
				}

				// Parse sizes (in bytes)
				let total_bytes = utils::parse_size_string(fields[1]).unwrap_or(0);
				let available_bytes = utils::parse_size_string(fields[3]).unwrap_or(0);

				let mount_path = PathBuf::from(&mount_point);
				let name = extract_volume_name(&mount_path);

				let mount_type = if mount_point.starts_with("/Volumes/") {
					MountType::External
				} else if filesystem.contains("://") {
					MountType::Network
				} else {
					MountType::System
				};

				let disk_type = detect_disk_type(&mount_path).unwrap_or(DiskType::Unknown);
				let file_system = detect_filesystem(&mount_path)
					.unwrap_or(FileSystem::Other("Unknown".to_string()));

				let volume = Volume::new(
					device_id,
					name.clone(),
					mount_type,
					classify_volume(&mount_path, &file_system, &name),
					mount_path,
					vec![],
					disk_type,
					file_system.clone(),
					total_bytes,
					available_bytes,
					false,
					Some(filesystem.to_string()),
					VolumeFingerprint::new(&name, total_bytes, &file_system.to_string()),
				);
				volumes.push(volume);
			}
		}
		Ok(volumes)
	})
	.await
	.map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
}

/// Classify a volume using the platform-specific classifier
fn classify_volume(
	mount_point: &PathBuf,
	file_system: &FileSystem,
	name: &str,
) -> crate::volume::types::VolumeType {
	let classifier = get_classifier();
	let detection_info = VolumeDetectionInfo {
		mount_point: mount_point.clone(),
		file_system: file_system.clone(),
		total_bytes_capacity: 0, // We don't have this info yet in some contexts
		is_removable: None,      // Would need additional detection
		is_network_drive: None,  // Would need additional detection
		device_model: None,      // Would need additional detection
	};

	classifier.classify(&detection_info)
}

/// Check if filesystem should be skipped
fn should_skip_filesystem(filesystem: &str) -> bool {
	matches!(
		filesystem,
		"devfs" | "tmpfs" | "proc" | "sysfs" | "fdescfs" | "kernfs"
	)
}

/// Extract volume name from mount path
fn extract_volume_name(mount_path: &PathBuf) -> String {
	if let Some(name) = mount_path.file_name() {
		name.to_string_lossy().to_string()
	} else if mount_path.to_string_lossy() == "/" {
		"Macintosh HD".to_string()
	} else {
		mount_path.to_string_lossy().to_string()
	}
}

/// Detect disk type (SSD vs HDD) using diskutil
fn detect_disk_type(mount_point: &PathBuf) -> VolumeResult<DiskType> {
	// Try to detect SSD vs HDD using diskutil
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

/// Detect filesystem type using diskutil
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

/// Get volume space information using df
pub fn get_volume_space_info(mount_point: &PathBuf) -> VolumeResult<(u64, u64)> {
	let output = Command::new("df")
		.args(["-k", mount_point.to_str().unwrap_or("/")])
		.output()
		.map_err(|e| VolumeError::platform(format!("Failed to run df: {}", e)))?;

	if !output.status.success() {
		return Ok((0, 0)); // Return zeros if df fails
	}

	let df_stdout = String::from_utf8_lossy(&output.stdout);
	for line in df_stdout.lines().skip(1) {
		// Skip header
		let fields: Vec<&str> = line.split_whitespace().collect();
		if fields.len() >= 4 {
			// df -k returns sizes in 1K blocks
			let total_kb = fields[1].parse::<u64>().unwrap_or(0);
			let available_kb = fields[3].parse::<u64>().unwrap_or(0);
			return Ok((total_kb * 1024, available_kb * 1024));
		}
	}

	Ok((0, 0))
}
