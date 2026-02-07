//! Android-specific volume detection using Linux APIs
//!
//! Android apps are sandboxed and can only access their own storage by default.
//! This module detects two types of storage:
//!
//! 1. **App's data directory** (`/data/data/com.spacedrive.app`) - private app storage
//! 2. **External storage** (`/storage/emulated/0`) - user-accessible storage via SAF
//!
//! The external storage path is essential for location creation, as Android's folder
//! picker (Storage Access Framework) returns paths under `/storage/emulated/0/...`.

use crate::volume::{
	error::VolumeResult,
	types::{
		DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig, VolumeFingerprint,
		VolumeType,
	},
};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Storage information retrieved from Android filesystem
struct AndroidVolumeInfo {
	total_capacity: u64,
	available_capacity: u64,
	mount_point: PathBuf,
}

/// Query Android device storage using statvfs
///
/// Uses the data directory path to query filesystem statistics.
/// This works because Android is Linux-based and exposes statvfs.
fn query_device_storage(data_dir: &std::path::Path) -> Result<AndroidVolumeInfo, String> {
	use std::mem::MaybeUninit;

	let path_str = data_dir
		.to_str()
		.ok_or_else(|| "Invalid data directory path".to_string())?;

	let c_path = CString::new(path_str).map_err(|e| format!("Failed to create CString: {}", e))?;

	let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

	let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };

	if result != 0 {
		let errno = std::io::Error::last_os_error();
		return Err(format!("statvfs failed: {}", errno));
	}

	let stat = unsafe { stat.assume_init() };

	// Calculate capacities
	// Total = blocks * block_size
	// Available = available_blocks * block_size (for unprivileged users)
	let block_size = stat.f_frsize as u64; // Fragment size (actual block size)
	let total_capacity = stat.f_blocks as u64 * block_size;
	let available_capacity = stat.f_bavail as u64 * block_size; // Available to non-root

	Ok(AndroidVolumeInfo {
		total_capacity,
		available_capacity,
		mount_point: data_dir.to_path_buf(),
	})
}

/// Get Android device model name
///
/// Reads from /system/build.prop or uses android.os.Build.MODEL equivalent.
/// Falls back to "Android Device" if unavailable.
fn get_device_name() -> String {
	// Try reading device model from system properties
	// Format: ro.product.model=Pixel 8a
	if let Ok(content) = std::fs::read_to_string("/system/build.prop") {
		for line in content.lines() {
			if line.starts_with("ro.product.model=") {
				if let Some(model) = line.strip_prefix("ro.product.model=") {
					let model = model.trim();
					if !model.is_empty() {
						return model.to_string();
					}
				}
			}
		}
	}

	// Fallback: try /proc/sys/kernel/hostname or just use generic name
	if let Ok(hostname) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
		let hostname = hostname.trim();
		if !hostname.is_empty() && hostname != "localhost" {
			return hostname.to_string();
		}
	}

	"Android Device".to_string()
}

/// Create a Volume struct from storage info
fn create_volume(
	storage_info: &AndroidVolumeInfo,
	device_id: Uuid,
	name: String,
	display_name: String,
	volume_type: VolumeType,
) -> Volume {
	let fingerprint = VolumeFingerprint::from_primary_volume(&storage_info.mount_point, device_id);
	let volume_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, fingerprint.0.as_bytes());
	let now = chrono::Utc::now();

	Volume {
		id: volume_id,
		fingerprint,
		device_id,
		name,
		library_id: None,
		is_tracked: false,
		mount_point: storage_info.mount_point.clone(),
		mount_points: vec![storage_info.mount_point.clone()],
		volume_type,
		mount_type: MountType::System,
		disk_type: DiskType::SSD,      // All Android devices use flash storage
		file_system: FileSystem::Ext4, // Android typically uses ext4 or f2fs
		total_capacity: storage_info.total_capacity,
		available_space: storage_info.available_capacity,
		is_read_only: false,
		is_mounted: true,
		hardware_id: None,
		backend: None,
		cloud_identifier: None,
		cloud_config: None,
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
		display_name: Some(display_name),
		is_favorite: false,
		color: None,
		icon: None,
		error_message: None,
	}
}

/// Check if a storage device is removable by examining /sys/block/{device}/removable
fn is_removable_storage(mount_point: &Path) -> bool {
	// Try to determine the block device from the mount point
	// On Android, we can check /proc/mounts to find the device
	if let Ok(mounts) = std::fs::read_to_string("/proc/mounts") {
		let mount_str = mount_point.to_string_lossy();
		for line in mounts.lines() {
			let parts: Vec<&str> = line.split_whitespace().collect();
			if parts.len() >= 2 && parts[1] == mount_str {
				let device = parts[0];
				// Extract device name (e.g., /dev/block/vold/public:179,65 -> check sysfs)
				if device.contains("/dev/block/") {
					// For vold-managed devices, check if they're under /mnt/media_rw (typically removable)
					if mount_point.starts_with("/mnt/media_rw")
						|| mount_point.starts_with("/mnt/usb")
					{
						return true;
					}
					// Try to extract the block device name for sysfs check
					if let Some(dev_name) = device.split('/').last() {
						let sysfs_path = format!("/sys/block/{}/removable", dev_name);
						if let Ok(removable) = std::fs::read_to_string(&sysfs_path) {
							return removable.trim() == "1";
						}
					}
				}
			}
		}
	}
	// Default to checking path patterns for removable storage
	let path_str = mount_point.to_string_lossy();
	// SD cards and USB drives are typically not under /storage/emulated
	!path_str.contains("/emulated/") && !path_str.contains("/self/")
}

/// Detect external volumes (SD cards, USB drives) on Android
fn detect_external_volumes(device_id: Uuid, device_name: &str) -> Vec<Volume> {
	let mut volumes = Vec::new();
	let search_paths = ["/storage", "/mnt/media_rw", "/mnt/usb"];

	for base_path in &search_paths {
		let base = Path::new(base_path);
		if !base.exists() {
			continue;
		}

		let entries = match std::fs::read_dir(base) {
			Ok(e) => e,
			Err(e) => {
				debug!("ANDROID_DETECT: Cannot read {}: {}", base_path, e);
				continue;
			}
		};

		for entry in entries.flatten() {
			let path = entry.path();
			let name = match entry.file_name().into_string() {
				Ok(n) => n,
				Err(_) => continue,
			};

			// Skip emulated and self (already handled as primary storage)
			if name == "emulated" || name == "self" {
				continue;
			}

			// Must be a directory and accessible
			if !path.is_dir() {
				continue;
			}

			// Try to query storage info
			match query_device_storage(&path) {
				Ok(storage_info) => {
					let is_removable = is_removable_storage(&path);
					let display_name = if is_removable {
						format!("SD Card ({})", name)
					} else {
						format!("External Storage ({})", name)
					};

					info!(
						"ANDROID_DETECT: Found external volume at {} - removable: {}, total: {} bytes",
						path.display(),
						is_removable,
						storage_info.total_capacity
					);

					let mut volume = create_volume(
						&AndroidVolumeInfo {
							total_capacity: storage_info.total_capacity,
							available_capacity: storage_info.available_capacity,
							mount_point: path.clone(),
						},
						device_id,
						name.clone(),
						display_name,
						VolumeType::External, // Both removable and non-removable external storage
					);

					// Set additional metadata for removable volumes
					if is_removable {
						volume.disk_type = DiskType::SSD; // SD cards are flash-based
						volume.mount_type = MountType::External;
					}

					volumes.push(volume);
				}
				Err(e) => {
					debug!(
						"ANDROID_DETECT: Cannot query storage at {}: {}",
						path.display(),
						e
					);
				}
			}
		}
	}

	volumes
}

/// Detect Android device storage volumes
///
/// Returns volumes representing accessible storage on Android:
/// 1. App's data directory (internal app storage)
/// 2. External storage (/storage/emulated/0) - user-accessible storage via SAF
/// 3. External volumes (SD cards, USB drives) if present
pub async fn detect_volumes(
	device_id: Uuid,
	_config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	debug!("ANDROID_DETECT: Starting Android volume detection");

	let mut volumes = Vec::new();
	let device_name = get_device_name();
	debug!("ANDROID_DETECT: Device name: {}", device_name);

	// 1. App's data directory (internal app storage)
	let data_dir = std::env::var("SPACEDRIVE_DATA_DIR")
		.map(PathBuf::from)
		.unwrap_or_else(|_| PathBuf::from("/data/data/com.spacedrive.app"));

	if let Ok(storage_info) = query_device_storage(&data_dir) {
		debug!(
			"ANDROID_DETECT: App storage query succeeded - total: {} bytes, available: {} bytes",
			storage_info.total_capacity, storage_info.available_capacity
		);
		volumes.push(create_volume(
			&storage_info,
			device_id,
			"App Storage".to_string(),
			"App Storage".to_string(),
			VolumeType::Primary,
		));
	} else {
		debug!("ANDROID_DETECT: Failed to query app data directory, continuing...");
	}

	// 2. External storage (user-accessible storage via SAF/folder picker)
	// This is where paths like /storage/emulated/0/Pictures/... live
	let external_storage = PathBuf::from("/storage/emulated/0");
	if external_storage.exists() {
		match query_device_storage(&external_storage) {
			Ok(storage_info) => {
				debug!(
					"ANDROID_DETECT: External storage query succeeded - total: {} bytes, available: {} bytes",
					storage_info.total_capacity, storage_info.available_capacity
				);
				volumes.push(create_volume(
					&storage_info,
					device_id,
					device_name.clone(),
					"Internal Storage".to_string(),
					VolumeType::Primary,
				));
			}
			Err(e) => {
				warn!("ANDROID_DETECT: Failed to query external storage: {}", e);
			}
		}
	} else {
		debug!("ANDROID_DETECT: External storage path does not exist");
	}

	// 3. Detect external volumes (SD cards, USB drives)
	let external_volumes = detect_external_volumes(device_id, &device_name);
	if !external_volumes.is_empty() {
		info!(
			"ANDROID_DETECT: Found {} external volume(s)",
			external_volumes.len()
		);
		volumes.extend(external_volumes);
	}

	if volumes.is_empty() {
		warn!("ANDROID_DETECT: No volumes detected on Android device");
	} else {
		debug!(
			"ANDROID_DETECT: Successfully detected {} Android volume(s)",
			volumes.len()
		);
	}

	Ok(volumes)
}
