//! Windows-specific volume detection helpers

use crate::volume::{
	classification::{get_classifier, VolumeDetectionInfo},
	error::{VolumeError, VolumeResult},
	types::{DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig, VolumeFingerprint},
	utils,
};
use std::path::PathBuf;
use std::process::Command;
use tokio::task;
use tracing::warn;
use uuid::Uuid;

/// Windows volume information from PowerShell/WMI
#[derive(Debug, Clone)]
pub struct WindowsVolumeInfo {
	pub drive_letter: Option<String>,
	pub label: Option<String>,
	pub size: u64,
	pub size_remaining: u64,
	pub filesystem: String,
	pub volume_guid: Option<String>,
}

/// Detect Windows volumes using PowerShell
pub async fn detect_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	let config = config.clone(); // Clone to move into async block
	task::spawn_blocking(move || {
		// Use PowerShell to get volume information
		let output = Command::new("powershell")
			.args([
				"-Command",
				"Get-Volume | Select-Object DriveLetter,FileSystemLabel,Size,SizeRemaining,FileSystem | ConvertTo-Json"
			])
			.output()
			.map_err(|e| VolumeError::platform(format!("Failed to run PowerShell: {}", e)))?;

		if !output.status.success() {
			warn!("PowerShell Get-Volume command failed, trying fallback method");
			return detect_volumes_fallback(device_id, &config);
		}

		let json_output = String::from_utf8_lossy(&output.stdout);
		parse_powershell_volumes(&json_output, device_id, &config)
	})
	.await
	.map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
}

/// Parse PowerShell JSON output into volumes
fn parse_powershell_volumes(
	json_output: &str,
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	// For now, return empty until we implement full JSON parsing
	// This would require adding serde_json dependency
	warn!("PowerShell JSON parsing not fully implemented yet");
	Ok(Vec::new())
}

/// Fallback method using wmic or fsutil
fn detect_volumes_fallback(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	let mut volumes = Vec::new();

	// Try using wmic as fallback
	let output = Command::new("wmic")
		.args([
			"logicaldisk",
			"get",
			"size,freespace,caption,filesystem,volumename",
			"/format:csv",
		])
		.output();

	match output {
		Ok(output) if output.status.success() => {
			let csv_output = String::from_utf8_lossy(&output.stdout);
			volumes.extend(parse_wmic_output(&csv_output, device_id, config)?);
		}
		_ => {
			warn!("Both PowerShell and wmic methods failed for Windows volume detection");
		}
	}

	Ok(volumes)
}

/// Parse wmic CSV output
fn parse_wmic_output(
	csv_output: &str,
	device_id: Uuid,
	_config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	let mut volumes = Vec::new();

	for line in csv_output.lines().skip(1) {
		// Skip header
		let fields: Vec<&str> = line.split(',').collect();
		if fields.len() >= 6 {
			let caption = fields[1].trim();
			let filesystem = fields[2].trim();
			let freespace_str = fields[3].trim();
			let size_str = fields[5].trim();
			let volume_name = fields[6].trim();

			// Skip if essential fields are empty
			if caption.is_empty() || size_str.is_empty() {
				continue;
			}

			let total_bytes = size_str.parse::<u64>().unwrap_or(0);
			let available_bytes = freespace_str.parse::<u64>().unwrap_or(0);

			if total_bytes == 0 {
				continue;
			}

			let mount_path = PathBuf::from(caption);
			let name = if volume_name.is_empty() {
				format!("Local Disk ({})", caption)
			} else {
				volume_name.to_string()
			};

			let file_system = utils::parse_filesystem_type(filesystem);
			let mount_type = determine_mount_type_windows(caption);
			let disk_type = DiskType::Unknown; // Would need additional WMI queries

			let volume_type = classify_volume(&mount_path, &file_system, &name);
			let fingerprint = VolumeFingerprint::new(&name, total_bytes, &file_system.to_string());

			let mut volume = Volume::new(device_id, fingerprint, name.clone(), mount_path);

			volume.mount_type = mount_type;
			volume.volume_type = volume_type;
			volume.disk_type = disk_type;
			volume.file_system = file_system;
			volume.total_capacity = total_bytes;
			volume.available_space = available_bytes;
			volume.is_read_only = false;
			volume.hardware_id = Some(caption.to_string());

			volumes.push(volume);
		}
	}

	Ok(volumes)
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

/// Determine mount type for Windows drives
fn determine_mount_type_windows(drive_letter: &str) -> MountType {
	match drive_letter.to_uppercase().as_str() {
		"C:\\" | "D:\\" => MountType::System, // Common system drives
		_ => MountType::External,             // Assume external for others
	}
}

/// Get Windows volume info using PowerShell (stub for now)
pub async fn get_windows_volume_info() -> VolumeResult<Vec<WindowsVolumeInfo>> {
	// This would be implemented with proper PowerShell parsing
	// or Windows API calls
	Ok(Vec::new())
}

/// Create volume from Windows info (stub for now)
pub fn create_volume_from_windows_info(
	info: WindowsVolumeInfo,
	device_id: Uuid,
) -> VolumeResult<Volume> {
	let mount_path = if let Some(drive_letter) = &info.drive_letter {
		PathBuf::from(format!("{}:\\", drive_letter))
	} else {
		PathBuf::from("C:\\") // Default fallback
	};

	let name = info.label.unwrap_or_else(|| {
		if let Some(drive) = &info.drive_letter {
			format!("Local Disk ({}:)", drive)
		} else {
			"Unknown Drive".to_string()
		}
	});

	let file_system = utils::parse_filesystem_type(&info.filesystem);
	let mount_type = if let Some(drive) = &info.drive_letter {
		determine_mount_type_windows(&format!("{}:\\", drive))
	} else {
		MountType::System
	};
	let volume_type = classify_volume(&mount_path, &file_system, &name);
	let fingerprint = VolumeFingerprint::new(&name, info.size, &file_system.to_string());

	let mut volume = Volume::new(device_id, fingerprint, name.clone(), mount_path);

	volume.mount_type = mount_type;
	volume.volume_type = volume_type;
	volume.disk_type = DiskType::Unknown;
	volume.file_system = file_system;
	volume.total_capacity = info.size;
	volume.available_space = info.size_remaining;
	volume.is_read_only = false;
	volume.hardware_id = info.volume_guid;

	Ok(volume)
}

/// Check if volume should be included based on config
pub fn should_include_volume(volume: &Volume, config: &VolumeDetectionConfig) -> bool {
	// Apply filtering based on config
	if !config.include_system && matches!(volume.mount_type, MountType::System) {
		return false;
	}

    // FIX: Use parentheses to call the method
	if !config.include_virtual && volume.total_bytes_capacity() == 0 {
		return false;
	}

	true
}