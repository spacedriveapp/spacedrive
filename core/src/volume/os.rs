use super::error::VolumeError;
use super::types::{DiskType, FileSystem, MountType, Volume};
use tokio::task;

// Re-export platform-specific get_volumes function
#[cfg(target_os = "linux")]
pub use self::linux::get_volumes;
#[cfg(target_os = "macos")]
pub use self::macos::get_volumes;
#[cfg(any(target_os = "ios", target_os = "android"))]
pub use self::mobile::get_volumes;
#[cfg(target_os = "windows")]
pub use self::windows::get_volumes;

// Re-export platform-specific unmount_volume function
#[cfg(target_os = "linux")]
pub use self::linux::unmount_volume;
#[cfg(target_os = "macos")]
pub use self::macos::unmount_volume;
#[cfg(any(target_os = "ios", target_os = "android"))]
pub use self::mobile::unmount_volume;
#[cfg(target_os = "windows")]
pub use self::windows::unmount_volume;

/// Common utilities for volume detection across platforms
mod common {
	pub fn parse_size(size_str: &str) -> u64 {
		size_str
			.chars()
			.filter(|c| c.is_ascii_digit())
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
	use std::{path::PathBuf, process::Command};
	use sysinfo::{DiskExt, System, SystemExt};

	pub async fn get_volumes() -> Result<Vec<Volume>, VolumeError> {
		let disk_info: Vec<(String, bool, PathBuf, Vec<u8>, u64, u64)> =
			tokio::task::spawn_blocking(|| {
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
					.collect()
			})
			.await
			.map_err(|e| VolumeError::Platform(format!("Task join error: {}", e)))?;

		let mut volumes = Vec::new();
		for (name, is_removable, mount_point, file_system, total_space, available_space) in
			disk_info
		{
			if !mount_point.exists() {
				continue;
			}

			let read_only = is_volume_readonly(&mount_point)?;
			let disk_type = detect_disk_type(&name)?;

			volumes.push(Volume::new(
				name,
				if is_removable {
					MountType::External
				} else {
					MountType::System
				},
				mount_point.clone(),
				vec![mount_point],
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
		let path = format!(
			"/sys/block/{}/queue/rotational",
			device_name.trim_start_matches("/dev/")
		);
		match std::fs::read_to_string(path) {
			Ok(contents) => match contents.trim() {
				"0" => Ok(DiskType::SSD),
				"1" => Ok(DiskType::HDD),
				_ => Ok(DiskType::Unknown),
			},
			Err(_) => Ok(DiskType::Unknown),
		}
	}

	fn is_volume_readonly(mount_point: &std::path::Path) -> Result<bool, VolumeError> {
		let output = Command::new("findmnt")
			.args([
				"--noheadings",
				"--output",
				"OPTIONS",
				mount_point.to_str().unwrap(),
			])
			.output()
			.map_err(|e| VolumeError::Platform(format!("Failed to run findmnt: {}", e)))?;

		let options = String::from_utf8_lossy(&output.stdout);
		Ok(options.contains("ro,") || options.contains(",ro") || options.contains("ro "))
	}

	pub async fn unmount_volume(path: &std::path::Path) -> Result<(), VolumeError> {
		// Try regular unmount first
		let result = tokio::process::Command::new("umount")
			.arg(path)
			.output()
			.await;

		match result {
			Ok(output) if output.status.success() => Ok(()),
			_ => {
				// If regular unmount fails, try lazy unmount
				let lazy_result = tokio::process::Command::new("umount")
					.args(["-l", path.to_str().unwrap()])
					.output()
					.await
					.map_err(|e| VolumeError::Platform(format!("Lazy unmount failed: {}", e)))?;

				if lazy_result.status.success() {
					Ok(())
				} else {
					Err(VolumeError::Platform(format!(
						"Failed to unmount volume: {}",
						String::from_utf8_lossy(&lazy_result.stderr)
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
	use std::path::PathBuf;

	use ::windows::core::PCWSTR;
	use ::windows::Win32::Storage::FileSystem::{
		GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
	};
	use ::windows::Win32::System::WindowsProgramming::{
		DRIVE_FIXED, DRIVE_REMOTE, DRIVE_REMOVABLE,
	};
	use std::os::windows::ffi::OsStrExt;

	pub async fn get_volumes() -> Vec<Volume> {
		task::spawn_blocking(|| {
			let mut volumes = Vec::new();

			// Get available drives
			let drives = unsafe { ::windows::Win32::Storage::FileSystem::GetLogicalDrives() };

			for i in 0..26 {
				if (drives & (1 << i)) != 0 {
					let drive_letter = (b'A' + i as u8) as char;
					let path = format!("{}:\\", drive_letter);
					let wide_path: Vec<u16> = OsString::from(&path)
						.encode_wide()
						.chain(std::iter::once(0))
						.collect();

					let drive_type = unsafe { GetDriveTypeW(PCWSTR(wide_path.as_ptr())) };

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

	fn detect_disk_type(path: &str) -> DiskType {
		// We would need to use DeviceIoControl to get this information
		// For brevity, returning Unknown, but you could implement the full detection
		// using IOCTL_STORAGE_QUERY_PROPERTY
		DiskType::Unknown
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
				PCWSTR(wide_path.as_ptr()),
				Some(name_buf.as_mut_slice()),
				Some(&mut serial_number),
				Some(&mut max_component_length),
				Some(&mut flags),
				Some(&mut fs_name_buf),
			);

			if let Ok(_) = success {
				let mut total_bytes = 0;
				let mut free_bytes = 0;
				let mut available_bytes = 0;

				if let Ok(_) = GetDiskFreeSpaceExW(
					PCWSTR(wide_path.as_ptr()),
					Some(&mut available_bytes),
					Some(&mut total_bytes),
					Some(&mut free_bytes),
				) {
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
						vec![PathBuf::from(path)],
						detect_disk_type(path),
						FileSystem::from_string(&fs_name),
						total_bytes,
						available_bytes,
						false,
					))
				} else {
					None
				}
			} else {
				None
			}
		}
	}

	pub async fn unmount_volume(path: &std::path::Path) -> Result<(), VolumeError> {
		use ::windows::core::PWSTR;
		use ::windows::Win32::Storage::FileSystem::{
			DeleteVolumeMountPointW, GetVolumeNameForVolumeMountPointW,
		};
		use std::ffi::OsStr;
		use std::os::windows::ffi::OsStrExt;

		// Convert path to wide string for Windows API
		let mut wide_path: Vec<u16> = OsStr::new(path)
			.encode_wide()
			.chain(std::iter::once(0))
			.collect();

		let wide_path_ptr = PWSTR(wide_path.as_mut_ptr());

		unsafe {
			// Buffer for volume name
			let mut volume_name = [0u16; 50];

			// Get the volume name for the mount point
			let result =
				GetVolumeNameForVolumeMountPointW(wide_path_ptr, volume_name.as_mut_slice());

			if result.is_err() {
				return Err(VolumeError::Platform(
					"Failed to get volume name".to_string(),
				));
			}

			// Delete the mount point
			let result = DeleteVolumeMountPointW(wide_path_ptr);

			if let Ok(_) = result {
				Ok(())
			} else {
				Err(VolumeError::Platform(
					"Failed to unmount volume".to_string(),
				))
			}
		}
	}
}

#[cfg(any(target_os = "ios", target_os = "android"))]
pub mod mobile {
	use super::*;

	pub async fn get_volumes() -> Result<Vec<Volume>, VolumeError> {
		// Mobile platforms don't have mountable volumes
		Ok(Vec::new())
	}

	pub async fn unmount_volume(_path: &std::path::Path) -> Result<(), VolumeError> {
		Err(VolumeError::Platform(
			"Volumes not supported on mobile platforms".to_string(),
		))
	}
}
