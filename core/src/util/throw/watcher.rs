use std::path::PathBuf;
use tokio::task;
use tracing::{debug, error, warn};

use super::error::VolumeError;
use super::types::{DiskType, FileSystem, MountType, Volume};

// Re-export platform-specific get_volumes function
#[cfg(target_os = "linux")]
pub use self::linux::get_volumes;
#[cfg(target_os = "macos")]
pub use self::macos::get_volumes;
#[cfg(target_os = "windows")]
pub use self::windows::get_volumes;

/// Common utilities for volume detection across platforms
mod common {
	use super::*;

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
mod macos {
	use super::*;
	use std::process::Command;
	use sysinfo::{DiskExt, System, SystemExt};

	pub async fn get_volumes() -> Vec<Volume> {
		task::spawn_blocking(|| {
			let mut volumes = Vec::new();
			let mut sys = System::new_all();
			sys.refresh_disks_list();

			for disk in sys.disks() {
				// Skip virtual filesystems
				if common::is_virtual_filesystem(
					disk.file_system().to_string_lossy().to_string().as_str(),
				) {
					continue;
				}

				let mut volume = Volume::new(
					disk.name().to_string_lossy().to_string(),
					if disk.is_removable() {
						MountType::External
					} else {
						MountType::System
					},
					disk.mount_point().to_path_buf(),
					detect_disk_type(disk.name().to_string_lossy().as_ref()),
					FileSystem::from_string(&disk.file_system().to_string_lossy()),
					disk.total_space(),
					disk.available_space(),
				);

				volume.read_only = is_volume_readonly(disk.mount_point());
				volumes.push(volume);
			}

			volumes
		})
		.await
		.unwrap_or_default()
	}

	fn detect_disk_type(device_name: &str) -> DiskType {
		let output = Command::new("diskutil")
			.args(["info", device_name])
			.output();

		match output {
			Ok(output) => {
				let info = String::from_utf8_lossy(&output.stdout);
				if info.contains("Solid State") {
					DiskType::SSD
				} else if info.contains("Rotational") {
					DiskType::HDD
				} else {
					DiskType::Unknown
				}
			}
			Err(_) => DiskType::Unknown,
		}
	}

	fn is_volume_readonly(mount_point: &std::path::Path) -> bool {
		let output = Command::new("mount")
			.output()
			.ok()
			.map(|o| String::from_utf8_lossy(&o.stdout).to_string());

		match output {
			Some(mount_output) => mount_output
				.lines()
				.find(|line| line.contains(&mount_point.to_string_lossy()))
				.map(|line| line.contains("read-only"))
				.unwrap_or(false),
			None => false,
		}
	}
}

#[cfg(target_os = "linux")]
mod linux {
	use super::*;
	use std::{fs, process::Command};
	use sysinfo::{DiskExt, System, SystemExt};

	pub async fn get_volumes() -> Vec<Volume> {
		task::spawn_blocking(|| {
			let mut volumes = Vec::new();
			let mut sys = System::new_all();
			sys.refresh_disks_list();

			// Read /proc/mounts for additional mount information
			let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
			let mount_points: Vec<_> = mounts
				.lines()
				.filter(|line| !line.starts_with("none"))
				.collect();

			for disk in sys.disks() {
				// Skip virtual filesystems
				if common::is_virtual_filesystem(
					disk.file_system().to_string_lossy().to_string().as_str(),
				) {
					continue;
				}

				let mount_point = disk.mount_point().to_path_buf();
				let mount_info = mount_points
					.iter()
					.find(|&line| line.contains(&mount_point.to_string_lossy()));

				let is_network = mount_info
					.map(|info| info.starts_with("//") || info.starts_with("nfs"))
					.unwrap_or(false);

				let mount_type = if is_network {
					MountType::Network
				} else if disk.is_removable() {
					MountType::External
				} else {
					MountType::System
				};

				let mut volume = Volume::new(
					disk.name().to_string_lossy().to_string(),
					mount_type,
					mount_point.clone(),
					detect_disk_type(&disk.name().to_string_lossy()),
					FileSystem::from_string(&disk.file_system().to_string_lossy()),
					disk.total_space(),
					disk.available_space(),
				);

				volume.read_only = mount_info
					.map(|info| info.contains("ro,") || info.contains(",ro"))
					.unwrap_or(false);

				volumes.push(volume);
			}

			volumes
		})
		.await
		.unwrap_or_default()
	}

	fn detect_disk_type(device_name: &str) -> DiskType {
		// Try reading rotational flag from sys
		if let Ok(rotational) =
			fs::read_to_string(format!("/sys/block/{}/queue/rotational", device_name))
		{
			match rotational.trim() {
				"0" => return DiskType::SSD,
				"1" => return DiskType::HDD,
				_ => {}
			}
		}

		// Fallback to lsblk
		let output = Command::new("lsblk")
			.args(["-d", "-o", "name,rota", device_name])
			.output();

		match output {
			Ok(output) => {
				let info = String::from_utf8_lossy(&output.stdout);
				if info.contains(" 0") {
					DiskType::SSD
				} else if info.contains(" 1") {
					DiskType::HDD
				} else {
					DiskType::Unknown
				}
			}
			Err(_) => DiskType::Unknown,
		}
	}
}

#[cfg(target_os = "windows")]
mod windows {
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

// Re-export the platform-specific get_volumes function
pub use self::get_volumes_impl::get_volumes;

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_get_volumes() {
		let volumes = get_volumes().await;
		assert!(!volumes.is_empty(), "Should detect at least one volume");

		for volume in volumes {
			println!("Detected volume: {:?}", volume);
			assert!(
				!volume.mount_point.as_os_str().is_empty(),
				"Mount point should not be empty"
			);
			assert!(!volume.name.is_empty(), "Volume name should not be empty");
			assert!(
				volume.total_bytes_capacity > 0,
				"Volume should have non-zero capacity"
			);
		}
	}
}
