use super::error::VolumeError;
use super::types::{DiskType, FileSystem, MountType, Volume};
use tokio::task;

// Re-export platform-specific get_volumes function
#[cfg(target_os = "linux")]
pub use self::linux::get_volumes;
#[cfg(target_os = "macos")]
pub use self::macos::get_volumes;
#[cfg(target_os = "windows")]
pub use self::windows::get_volumes;

/// Common utilities for volume detection across platforms
mod common {
	pub fn parse_size(size_str: &str) -> u64 {
		size_str
			.chars()
			.filter(|c| c.is_digit(10))
			.collect::<String>()
			.parse()
			.unwrap_or(0)
	}

	pub fn is_virtual_filesystem(fs: &str) -> bool {
		matches!(
			fs.to_lowercase().as_str(),
			"devfs" | "sysfs" | "proc" | "tmpfs" | "ramfs" | "devtmpfs"
		)
	}
}

#[cfg(target_os = "macos")]
pub mod macos {
	use super::*;
	use std::{collections::HashMap, path::PathBuf, process::Command};
	use sysinfo::{DiskExt, System, SystemExt};

	pub async fn get_volumes() -> Result<Vec<Volume>, VolumeError> {
		task::spawn_blocking(|| {
			let mut volumes = Vec::new();
			let mut sys = System::new_all();
			sys.refresh_disks_list();

			let mut temp_volumes: HashMap<String, Volume> = HashMap::new();

			for disk in sys.disks() {
				// Skip virtual filesystems
				if common::is_virtual_filesystem(
					std::str::from_utf8(disk.file_system()).unwrap_or(""),
				) {
					continue;
				}

				let disk_type = detect_disk_type(disk.name().to_string_lossy().as_ref())?;
				let mount_point = disk.mount_point().to_path_buf();

				// For APFS volumes, use the disk name as the key
				let key = disk.name().to_string_lossy().to_string();

				let mount_points = vec![mount_point.clone()];

				if let Some(existing) = temp_volumes.get_mut(&key) {
					// If we already have this volume, add the mount point
					existing.mount_points.push(mount_point);
					continue;
				}

				let volume = Volume::new(
					disk.name().to_string_lossy().to_string(),
					if disk.is_removable() {
						MountType::External
					} else {
						MountType::System
					},
					mount_point.clone(),
					mount_points,
					disk_type,
					FileSystem::from_string(&String::from_utf8_lossy(&disk.file_system())),
					disk.total_space(),
					disk.available_space(),
					is_volume_readonly(&mount_point)?,
				);

				temp_volumes.insert(key, volume);
			}

			// Move volumes from HashMap to Vec
			volumes.extend(temp_volumes.into_values());

			Ok(volumes)
		})
		.await
		.map_err(|e| VolumeError::Platform(format!("Task join error: {}", e)))?
	}

	fn create_volume_from_disk(disk: &sysinfo::Disk) -> Result<Volume, VolumeError> {
		let disk_type = detect_disk_type(disk.name().to_string_lossy().as_ref())?;
		let primary_mount_point = disk.mount_point().to_path_buf();

		if !primary_mount_point.exists() {
			return Err(VolumeError::NoMountPoint);
		}

		// Get all mount points for this volume
		let mut mount_points = Vec::new();
		mount_points.push(primary_mount_point.clone());

		// For macOS APFS system volumes
		if primary_mount_point == PathBuf::from("/") {
			let data_path = PathBuf::from("/System/Volumes/Data");
			if data_path.exists() {
				mount_points.push(data_path);
			}
		}

		let read_only = is_volume_readonly(&primary_mount_point)?;

		Ok(Volume::new(
			disk.name().to_string_lossy().to_string(),
			if disk.is_removable() {
				MountType::External
			} else {
				MountType::System
			},
			primary_mount_point,
			mount_points,
			disk_type,
			FileSystem::from_string(&String::from_utf8_lossy(&disk.file_system())),
			disk.total_space(),
			disk.available_space(),
			read_only,
		))
	}

	fn detect_disk_type(device_name: &str) -> Result<DiskType, VolumeError> {
		let output = Command::new("diskutil")
			.args(["info", device_name])
			.output()
			.map_err(|e| VolumeError::Platform(format!("Failed to run diskutil: {}", e)))?;

		let info = String::from_utf8_lossy(&output.stdout);
		Ok(if info.contains("Solid State") {
			DiskType::SSD
		} else if info.contains("Rotational") {
			DiskType::HDD
		} else {
			DiskType::Unknown
		})
	}

	fn is_volume_readonly(mount_point: &std::path::Path) -> Result<bool, VolumeError> {
		let output = Command::new("mount")
			.output()
			.map_err(|e| VolumeError::Platform(format!("Failed to run mount command: {}", e)))?;

		let mount_output = String::from_utf8_lossy(&output.stdout);
		Ok(mount_output
			.lines()
			.find(|line| line.contains(&*mount_point.to_string_lossy()))
			.map(|line| line.contains("read-only"))
			.unwrap_or(false))
	}
}

#[cfg(target_os = "linux")]
pub mod linux {
	use super::*;
	use std::{fs, process::Command};
	use sysinfo::{DiskExt, System, SystemExt};

	pub async fn get_volumes() -> Result<Vec<Volume>, VolumeError> {
		task::spawn_blocking(|| {
			let mut volumes = Vec::new();
			let mut sys = System::new_all();
			sys.refresh_disks_list();

			// Read /proc/mounts for additional mount information
			let mounts = fs::read_to_string("/proc/mounts").map_err(|e| {
				VolumeError::Platform(format!("Failed to read /proc/mounts: {}", e))
			})?;

			let mount_points: Vec<_> = mounts
				.lines()
				.filter(|line| !line.starts_with("none"))
				.collect();

			for disk in sys.disks() {
				if common::is_virtual_filesystem(
					disk.file_system().to_string_lossy().to_string().as_str(),
				) {
					continue;
				}

				let volume = create_volume_from_disk(disk, &mount_points).map_err(|e| {
					VolumeError::Platform(format!("Failed to create volume: {}", e))
				})?;
				volumes.push(volume);
			}

			Ok(volumes)
		})
		.await
		.map_err(|e| VolumeError::Platform(format!("Task join error: {}", e)))?
	}

	fn create_volume_from_disk(
		disk: &sysinfo::Disk,
		mount_points: &[&str],
	) -> Result<Volume, VolumeError> {
		let mount_point = disk.mount_point().to_path_buf();

		let mount_info = mount_points
			.iter()
			.find(|&line| line.contains(&mount_point.to_string_lossy()))
			.ok_or_else(|| VolumeError::NoMountPoint)?;

		let is_network = mount_info.starts_with("//") || mount_info.starts_with("nfs");
		let disk_type = detect_disk_type(&disk.name().to_string_lossy())
			.map_err(|e| VolumeError::Platform(format!("Failed to detect disk type: {}", e)))?;

		Ok(Volume::new(
			disk.name().to_string_lossy().to_string(),
			if is_network {
				MountType::Network
			} else if disk.is_removable() {
				MountType::External
			} else {
				MountType::System
			},
			mount_point,
			disk_type,
			FileSystem::from_string(&disk.file_system().to_string_lossy()),
			disk.total_space(),
			disk.available_space(),
		))
	}
}
#[cfg(target_os = "windows")]
pub mod windows {
	use super::*;
	use std::ffi::OsString;
	use std::os::windows::ffi::OsStringExt;
	use windows::Win32::Storage::FileSystem::{
		GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW, DRIVE_FIXED, DRIVE_REMOTE,
		DRIVE_REMOVABLE,
	};
	use windows::Win32::System::Ioctl::STORAGE_PROPERTY_QUERY;

	pub async fn get_volumes() -> Vec<Volume> {
		task::spawn_blocking(|| {
			let mut volumes = Vec::new();

			// Get available drives
			let drives = unsafe { windows::Win32::Storage::FileSystem::GetLogicalDrives() };

			for i in 0..26 {
				if (drives & (1 << i)) != 0 {
					let drive_letter = (b'A' + i as u8) as char;
					let path = format!("{}:\\", drive_letter);
					let wide_path: Vec<u16> = OsString::from(&path)
						.encode_wide()
						.chain(std::iter::once(0))
						.collect();

					let drive_type = unsafe { GetDriveTypeW(wide_path.as_ptr()) };

					// Skip CD-ROM drives and other unsupported types
					if drive_type == DRIVE_FIXED
						|| drive_type == DRIVE_REMOVABLE
						|| drive_type == DRIVE_REMOTE
					{
						if let Some(volume) = get_volume_info(&path, drive_type) {
							volumes.push(volume);
						}
					}
				}
			}

			volumes
		})
		.await
		.unwrap_or_default()
	}

	fn get_volume_info(path: &str, drive_type: u32) -> Option<Volume> {
		let wide_path: Vec<u16> = OsString::from(path)
			.encode_wide()
			.chain(std::iter::once(0))
			.collect();

		let mut name_buf = [0u16; 256];
		let mut fs_name_buf = [0u16; 256];
		let mut serial_number = 0;
		let mut max_component_length = 0;
		let mut flags = 0;

		unsafe {
			let success = GetVolumeInformationW(
				wide_path.as_ptr(),
				name_buf.as_mut_ptr(),
				name_buf.len() as u32,
				&mut serial_number,
				&mut max_component_length,
				&mut flags,
				fs_name_buf.as_mut_ptr(),
				fs_name_buf.len() as u32,
			);

			if success.as_bool() {
				let mut total_bytes = 0;
				let mut free_bytes = 0;
				let mut available_bytes = 0;

				if GetDiskFreeSpaceExW(
					wide_path.as_ptr(),
					&mut available_bytes,
					&mut total_bytes,
					&mut free_bytes,
				)
				.as_bool()
				{
					let mount_type = match drive_type {
						DRIVE_FIXED => MountType::System,
						DRIVE_REMOVABLE => MountType::External,
						DRIVE_REMOTE => MountType::Network,
						_ => MountType::System,
					};

					let volume_name = String::from_utf16_lossy(&name_buf)
						.trim_matches(char::from(0))
						.to_string();

					let fs_name = String::from_utf16_lossy(&fs_name_buf)
						.trim_matches(char::from(0))
						.to_string();

					Some(Volume::new(
						if volume_name.is_empty() {
							path.to_string()
						} else {
							volume_name
						},
						mount_type,
						PathBuf::from(path),
						detect_disk_type(path),
						FileSystem::from_string(&fs_name),
						total_bytes as u64,
						available_bytes as u64,
					))
				} else {
					None
				}
			} else {
				None
			}
		}
	}

	fn detect_disk_type(path: &str) -> DiskType {
		// We would need to use DeviceIoControl to get this information
		// For brevity, returning Unknown, but you could implement the full detection
		// using IOCTL_STORAGE_QUERY_PROPERTY
		DiskType::Unknown
	}
}
