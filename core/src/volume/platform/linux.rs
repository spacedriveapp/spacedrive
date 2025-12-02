//! Linux-specific volume detection helpers

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

/// Mount information from /proc/mounts or df output
#[derive(Debug, Clone)]
pub struct MountInfo {
	pub device: String,
	pub mount_point: String,
	pub filesystem_type: String,
	pub total_bytes: u64,
	pub available_bytes: u64,
}

/// Detect Linux volumes using df command
pub async fn detect_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	let config = config.clone(); // Clone to move into async block
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

		for line in df_text.lines().skip(1) {
			// Skip header
			if let Some(volume) = parse_df_line(line, device_id, &config)? {
				volumes.push(volume);
			}
		}

		Ok(volumes)
	})
	.await
	.map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
}

/// Parse a single df output line into a Volume
fn parse_df_line(
	line: &str,
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Option<Volume>> {
	let parts: Vec<&str> = line.split_whitespace().collect();
	if parts.len() < 7 {
		return Ok(None);
	}

	let filesystem_device = parts[0];
	let filesystem_type = parts[1];
	let size_str = parts[2];
	let _used_str = parts[3];
	let available_str = parts[4];
	let mount_point = parts[6];

	// Skip system filesystems unless requested
	if !config.include_system && utils::is_system_filesystem(filesystem_device) {
		return Ok(None);
	}

	// Skip virtual filesystems unless requested
	if !config.include_virtual && utils::is_virtual_filesystem(filesystem_type) {
		return Ok(None);
	}

	let mount_path = PathBuf::from(mount_point);

	let total_bytes = utils::parse_size_string(size_str)?;
	let available_bytes = utils::parse_size_string(available_str)?;

	let name = if mount_point == "/" {
		"Root".to_string()
	} else {
		mount_path
			.file_name()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string()
	};

	let mount_type = determine_mount_type(mount_point, filesystem_device);
	let disk_type = detect_disk_type_linux(filesystem_device)?;
	let file_system = utils::parse_filesystem_type(filesystem_type);
	let volume_type = classify_volume(&mount_path, &file_system, &name);
	let fingerprint = VolumeFingerprint::new(&name, total_bytes, &file_system.to_string());

	let mut volume = Volume::new(
		device_id,
		fingerprint,
		name.clone(),
		mount_path,
	);

	volume.mount_type = mount_type;
	volume.volume_type = volume_type;
	volume.disk_type = disk_type;
	volume.file_system = file_system;
	volume.total_capacity = total_bytes;
	volume.available_space = available_bytes;
	volume.is_read_only = false;
	volume.hardware_id = Some(filesystem_device.to_string());

	Ok(Some(volume))
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

/// Detect disk type (SSD vs HDD) using Linux /sys filesystem
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

/// Determine mount type based on mount point and device
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

/// Parse /proc/mounts for detailed mount information
pub async fn parse_proc_mounts() -> VolumeResult<Vec<MountInfo>> {
	task::spawn_blocking(|| {
		let contents = std::fs::read_to_string("/proc/mounts")
			.map_err(|e| VolumeError::platform(format!("Failed to read /proc/mounts: {}", e)))?;

		let mut mounts = Vec::new();

		for line in contents.lines() {
			let parts: Vec<&str> = line.split_whitespace().collect();
			if parts.len() >= 3 {
				let device = parts[0].to_string();
				let mount_point = parts[1].to_string();
				let filesystem_type = parts[2].to_string();

				// Skip virtual filesystems
				if utils::is_virtual_filesystem(&filesystem_type) {
					continue;
				}

				// Get size information using statvfs
				let (total_bytes, available_bytes) = get_filesystem_space(&mount_point)?;

				mounts.push(MountInfo {
					device,
					mount_point,
					filesystem_type,
					total_bytes,
					available_bytes,
				});
			}
		}

		Ok(mounts)
	})
	.await
	.map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
}

/// Get filesystem space using statvfs
fn get_filesystem_space(mount_point: &str) -> VolumeResult<(u64, u64)> {
	use std::ffi::CString;
	use std::mem;

	let path = CString::new(mount_point)
		.map_err(|e| VolumeError::platform(format!("Invalid path: {}", e)))?;

	unsafe {
		let mut statvfs: libc::statvfs = mem::zeroed();
		if libc::statvfs(path.as_ptr(), &mut statvfs) == 0 {
			let total_bytes = statvfs.f_blocks * statvfs.f_frsize;
			let available_bytes = statvfs.f_bavail * statvfs.f_frsize;
			Ok((total_bytes, available_bytes))
		} else {
			Ok((0, 0))
		}
	}
}

/// Create a Volume from MountInfo
pub fn create_volume_from_mount(mount: MountInfo, device_id: Uuid) -> VolumeResult<Volume> {
	let mount_path = PathBuf::from(&mount.mount_point);
	let file_system = utils::parse_filesystem_type(&mount.filesystem_type);

	let name = if mount.mount_point == "/" {
		"Root".to_string()
	} else {
		mount_path
			.file_name()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string()
	};

	let mount_type = determine_mount_type(&mount.mount_point, &mount.device);
	let disk_type = detect_disk_type_linux(&mount.device)?;
	let volume_type = classify_volume(&mount_path, &file_system, &name);
	let fingerprint = VolumeFingerprint::new(&name, mount.total_bytes, &file_system.to_string());

	let mut volume = Volume::new(
		device_id,
		fingerprint,
		name.clone(),
		mount_path,
	);

	volume.mount_type = mount_type;
	volume.volume_type = volume_type;
	volume.disk_type = disk_type;
	volume.file_system = file_system;
	volume.total_capacity = mount.total_bytes;
	volume.available_space = mount.available_bytes;
	volume.is_read_only = false;
	volume.hardware_id = Some(mount.device);

	Ok(volume)
}
