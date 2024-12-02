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

// Re-export platform-specific unmount_volume function
#[cfg(target_os = "linux")]
pub use self::linux::unmount_volume;
#[cfg(target_os = "macos")]
pub use self::macos::unmount_volume;
#[cfg(target_os = "windows")]
pub use self::windows::unmount_volume;

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
	use std::{path::PathBuf, process::Command};
	use sysinfo::{DiskExt, System, SystemExt};

	pub async fn get_volumes() -> Result<Vec<Volume>, VolumeError> {
		// First collect disk info in blocking context
		let disk_info: Vec<(String, bool, PathBuf, Vec<u8>, u64, u64)> =
			task::spawn_blocking(|| {
				let mut sys = System::new_all();
				sys.refresh_disks_list();

				sys.disks()
					.iter()
					.filter(|disk| {
						!common::is_virtual_filesystem(
							std::str::from_utf8(disk.file_system()).unwrap_or(""),
						)
					})
					.map(|disk| {
						(
							disk.name().to_string_lossy().to_string(),
							disk.is_removable(),
							disk.mount_point().to_path_buf(),
							disk.file_system().to_vec(),
							disk.total_space(),
							disk.available_space(),
						)
					})
					.collect::<Vec<_>>() // Specify that the collection should be a Vec
			})
			.await
			.map_err(|e| VolumeError::Platform(format!("Task join error: {}", e)))?;

		// Then create volumes with the collected info
		let mut volumes = Vec::new();
		for (name, is_removable, mount_point, file_system, total_space, available_space) in
			disk_info
		{
			if !mount_point.exists() {
				continue;
			}
			let read_only = is_volume_readonly(&mount_point)?;
			// Skip adding the `/` mount point if it's both read-only and a system volume
			if mount_point == PathBuf::from("/") && read_only {
				continue;
			}
			let disk_type = detect_disk_type(&name)?;
			let mut mount_points = vec![mount_point.clone()];

			// For macOS APFS system volumes
			if mount_point == PathBuf::from("/") {
				let data_path = PathBuf::from("/System/Volumes/Data");
				if data_path.exists() {
					mount_points.push(data_path);
				}
			}

			volumes.push(Volume::new(
				name,
				if is_removable {
					MountType::External
				} else {
					MountType::System
				},
				mount_point,
				mount_points,
				disk_type,
				FileSystem::from_string(&String::from_utf8_lossy(&file_system)),
				total_space,
				available_space,
				read_only,
			));
		}

		Ok(volumes)
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
	pub async fn unmount_volume(path: &std::path::Path) -> Result<(), VolumeError> {
		use std::process::Command;
		use tokio::process::Command as TokioCommand;

		// First try diskutil
		let result = TokioCommand::new("diskutil")
			.arg("unmount")
			.arg(path)
			.output()
			.await;

		match result {
			Ok(output) => {
				if output.status.success() {
					return Ok(());
				}
				// If diskutil fails, try umount as fallback
				let fallback = Command::new("umount")
					.arg(path)
					.output()
					.map_err(|e| VolumeError::Platform(format!("Unmount failed: {}", e)))?;

				if fallback.status.success() {
					Ok(())
				} else {
					Err(VolumeError::Platform(format!(
						"Failed to unmount volume: {}",
						String::from_utf8_lossy(&fallback.stderr)
					)))
				}
			}
			Err(e) => Err(VolumeError::Platform(format!(
				"Failed to execute unmount command: {}",
				e
			))),
		}
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
	pub async fn unmount_volume(path: &std::path::Path) -> Result<(), VolumeError> {
		use tokio::process::Command;

		// Try umount first
		let result = Command::new("umount")
			.arg(path)
			.output()
			.await
			.map_err(|e| VolumeError::Platform(format!("Unmount failed: {}", e)))?;

		if result.status.success() {
			Ok(())
		} else {
			// If regular unmount fails, try with force option
			let force_result = Command::new("umount")
				.arg("-f") // Force unmount
				.arg(path)
				.output()
				.await
				.map_err(|e| VolumeError::Platform(format!("Force unmount failed: {}", e)))?;

			if force_result.status.success() {
				Ok(())
			} else {
				// If both attempts fail, try udisksctl as a last resort
				let udisks_result = Command::new("udisksctl")
					.arg("unmount")
					.arg("-b")
					.arg(path)
					.output()
					.await
					.map_err(|e| {
						VolumeError::Platform(format!("udisksctl unmount failed: {}", e))
					})?;

				if udisks_result.status.success() {
					Ok(())
				} else {
					Err(VolumeError::Platform(format!(
						"All unmount attempts failed: {}",
						String::from_utf8_lossy(&udisks_result.stderr)
					)))
				}
			}
		}
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
	pub async fn unmount_volume(path: &std::path::Path) -> Result<(), VolumeError> {
		use std::ffi::OsStr;
		use std::os::windows::ffi::OsStrExt;
		use windows::core::PWSTR;
		use windows::Win32::Storage::FileSystem::{
			DeleteVolumeMountPointW, GetVolumeNameForVolumeMountPointW,
		};

		// Convert path to wide string for Windows API
		let wide_path: Vec<u16> = OsStr::new(path)
			.encode_wide()
			.chain(std::iter::once(0))
			.collect();

		unsafe {
			// Buffer for volume name
			let mut volume_name = [0u16; 50];
			let mut volume_name_ptr = PWSTR(volume_name.as_mut_ptr());

			// Get the volume name for the mount point
			let result = GetVolumeNameForVolumeMountPointW(wide_path.as_ptr(), volume_name_ptr);

			if !result.as_bool() {
				return Err(VolumeError::Platform(
					"Failed to get volume name".to_string(),
				));
			}

			// Delete the mount point
			let result = DeleteVolumeMountPointW(wide_path.as_ptr());

			if result.as_bool() {
				Ok(())
			} else {
				Err(VolumeError::Platform(
					"Failed to unmount volume".to_string(),
				))
			}
		}
	}
}
