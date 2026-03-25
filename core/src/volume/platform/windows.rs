//! Windows-specific volume detection using native sysinfo APIs
//!
//! Uses the `sysinfo` crate for volume enumeration instead of spawning
//! PowerShell processes, which is significantly faster and more reliable.

use crate::volume::{
	classification::{get_classifier, VolumeDetectionInfo},
	error::{VolumeError, VolumeResult},
	types::{DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig, VolumeFingerprint},
	utils,
};
use std::path::PathBuf;
use tokio::task;
use tracing::{debug, warn};
use uuid::Uuid;

/// Detect Windows volumes using sysinfo (native Win32 APIs)
pub async fn detect_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	let config = config.clone();
	task::spawn_blocking(move || {
		let disks = sysinfo::Disks::new_with_refreshed_list();
		debug!("sysinfo detected {} disks", disks.list().len());

		let mut volumes = Vec::new();
		for disk in disks.list() {
			let mount_point = disk.mount_point().to_path_buf();
			let total_space = disk.total_space();
			let available_space = disk.available_space();
			let fs_name = disk.file_system().to_string_lossy().to_string();
			let label = disk.name().to_string_lossy().to_string();
			let is_removable = disk.is_removable();

			// Skip volumes with no mount point or zero capacity
			if mount_point.as_os_str().is_empty() {
				debug!("Skipping disk with empty mount point: label={:?}", label);
				continue;
			}

			if total_space == 0 {
				debug!("Skipping disk with zero capacity: {:?}", mount_point);
				continue;
			}

			let name = if label.is_empty() {
				format!(
					"Local Disk ({})",
					mount_point.to_string_lossy().trim_end_matches('\\')
				)
			} else {
				label
			};

			let file_system = utils::parse_filesystem_type(&fs_name);
			let mount_type = determine_mount_type_windows(&mount_point);
			let disk_type = match disk.kind() {
				sysinfo::DiskKind::SSD => DiskType::SSD,
				sysinfo::DiskKind::HDD => DiskType::HDD,
				_ => DiskType::Unknown,
			};

			let volume_type =
				classify_volume(&mount_point, &file_system, &name, is_removable, total_space);

			// Generate stable fingerprint based on volume type
			let fingerprint = match volume_type {
				crate::volume::types::VolumeType::External => {
					if let Some(spacedrive_id) =
						utils::read_or_create_dotfile_sync(&mount_point, device_id, None)
					{
						VolumeFingerprint::from_external_volume(spacedrive_id, device_id)
					} else {
						VolumeFingerprint::from_primary_volume(&mount_point, device_id)
					}
				}
				crate::volume::types::VolumeType::Network => {
					let path_lossy = mount_point.to_string_lossy();
					VolumeFingerprint::from_network_volume(&path_lossy, &path_lossy)
				}
				_ => VolumeFingerprint::from_primary_volume(&mount_point, device_id),
			};

			let mut volume = Volume::new(device_id, fingerprint, name.clone(), mount_point);

			volume.mount_type = mount_type;
			volume.volume_type = volume_type;
			volume.disk_type = disk_type;
			volume.file_system = file_system;
			volume.total_capacity = total_space;
			volume.available_space = available_space;
			volume.is_read_only = false;

			if should_include_volume(&volume, &config) {
				debug!(
					"Detected volume: {} ({}) - {} bytes",
					volume.name,
					volume.mount_point.display(),
					total_space
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
	is_removable: bool,
	total_bytes_capacity: u64,
) -> crate::volume::types::VolumeType {
	let classifier = get_classifier();
	let detection_info = VolumeDetectionInfo {
		mount_point: mount_point.clone(),
		file_system: file_system.clone(),
		total_bytes_capacity,
		is_removable: Some(is_removable),
		is_network_drive: None,
		device_model: None,
	};

	classifier.classify(&detection_info)
}

/// Determine mount type for Windows drives by checking if the volume
/// hosts the Windows installation (contains `\Windows\System32`).
fn determine_mount_type_windows(mount_point: &std::path::Path) -> MountType {
	if mount_point.join("Windows").join("System32").is_dir() {
		MountType::System
	} else {
		MountType::External
	}
}

/// Check if volume should be included based on config
pub fn should_include_volume(volume: &Volume, config: &VolumeDetectionConfig) -> bool {
	if !config.include_system && matches!(volume.mount_type, MountType::System) {
		return false;
	}

	if !config.include_virtual && volume.total_bytes_capacity() == 0 {
		return false;
	}

	true
}
