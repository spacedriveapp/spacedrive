//! ReFS filesystem-specific detection and optimization
//!
//! This module handles ReFS volume detection and provides ReFS-specific
//! optimizations like block cloning operations.
//!
//! Block cloning support is detected via native Win32 `FSCTL_GET_REFS_VOLUME_DATA`
//! IOCTL call. ReFS v2.0+ (Windows 10 1703 / Server 2016) supports block cloning.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::task;
use tracing::{debug, warn};

/// Cached IOCTL results keyed by volume GUID to avoid repeated syscalls.
#[cfg(windows)]
static REFS_BLOCK_CLONE_CACHE: Mutex<Option<HashMap<String, RefsIoctlResult>>> = Mutex::new(None);

/// Result of a ReFS IOCTL version query.
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
struct RefsIoctlResult {
	supports_block_cloning: bool,
	major_version: u32,
	minor_version: u32,
}

/// Query ReFS version and block cloning capability via `FSCTL_GET_REFS_VOLUME_DATA`.
///
/// This is a synchronous function meant to be called inside `spawn_blocking`.
/// Results are cached per volume GUID.
#[cfg(windows)]
fn check_refs_version_sync(path: &Path) -> Option<RefsIoctlResult> {
	use std::mem::size_of;
	use std::os::windows::ffi::OsStrExt;
	use std::ptr::{null, null_mut};
	use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE};
	use windows_sys::Win32::Storage::FileSystem::{
		CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
	};
	use windows_sys::Win32::System::Ioctl::{FSCTL_GET_REFS_VOLUME_DATA, REFS_VOLUME_DATA_BUFFER};
	use windows_sys::Win32::System::IO::DeviceIoControl;

	let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

	let handle = unsafe {
		CreateFileW(
			wide_path.as_ptr(),
			GENERIC_READ,
			FILE_SHARE_READ | FILE_SHARE_WRITE,
			null(),
			OPEN_EXISTING,
			FILE_FLAG_BACKUP_SEMANTICS,
			0,
		)
	};

	if handle == INVALID_HANDLE_VALUE {
		warn!("Failed to open handle for ReFS IOCTL: {}", path.display());
		return None;
	}

	let mut buffer: REFS_VOLUME_DATA_BUFFER = unsafe { std::mem::zeroed() };
	let mut bytes_returned = 0u32;

	let ok = unsafe {
		DeviceIoControl(
			handle,
			FSCTL_GET_REFS_VOLUME_DATA,
			null(),
			0,
			&mut buffer as *mut _ as *mut _,
			size_of::<REFS_VOLUME_DATA_BUFFER>() as u32,
			&mut bytes_returned,
			null_mut(),
		)
	};

	unsafe { CloseHandle(handle) };

	if ok == 0 {
		warn!("FSCTL_GET_REFS_VOLUME_DATA failed for: {}", path.display());
		return None;
	}

	let result = RefsIoctlResult {
		supports_block_cloning: buffer.MajorVersion >= 2,
		major_version: buffer.MajorVersion,
		minor_version: buffer.MinorVersion,
	};

	debug!(
		"ReFS v{}.{} at {}: block cloning = {}",
		result.major_version,
		result.minor_version,
		path.display(),
		result.supports_block_cloning
	);

	Some(result)
}

/// Query ReFS version with caching by volume GUID.
/// Falls back to uncached query if GUID resolution fails.
#[cfg(windows)]
fn check_refs_version_cached(mount_point: &Path) -> Option<RefsIoctlResult> {
	let guid = super::volume_guid(mount_point);

	// Check cache by GUID
	if let Some(ref guid_key) = guid {
		if let Ok(guard) = REFS_BLOCK_CLONE_CACHE.lock() {
			if let Some(cache) = guard.as_ref() {
				if let Some(result) = cache.get(guid_key) {
					return Some(*result);
				}
			}
		}
	}

	let result = check_refs_version_sync(mount_point)?;

	// Store in cache by GUID
	if let Some(guid_key) = guid {
		if let Ok(mut guard) = REFS_BLOCK_CLONE_CACHE.lock() {
			let cache = guard.get_or_insert_with(HashMap::new);
			cache.insert(guid_key, result);
		}
	}

	Some(result)
}

/// ReFS filesystem handler
pub struct RefsHandler;

impl RefsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same ReFS volume.
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		if let (Ok(vol1), Ok(vol2)) = (
			self.get_volume_info(path1).await,
			self.get_volume_info(path2).await,
		) {
			return vol1.volume_guid == vol2.volume_guid;
		}

		false
	}

	/// Get ReFS volume information for a path.
	async fn get_volume_info(&self, path: &Path) -> VolumeResult<RefsVolumeInfo> {
		let path = path.to_path_buf();

		task::spawn_blocking(move || {
			let disks = sysinfo::Disks::new_with_refreshed_list();
			let mut best_match: Option<(&sysinfo::Disk, usize)> = None;

			for disk in &disks {
				let mount_point = disk.mount_point();
				if path.starts_with(mount_point) {
					let len = mount_point.as_os_str().len();
					if best_match.map_or(true, |(_, l)| len > l) {
						best_match = Some((disk, len));
					}
				}
			}

			if let Some((disk, _)) = best_match {
				let mount_point = disk.mount_point();
				let mount_str = mount_point.to_string_lossy();
				let fs_name = disk.file_system().to_string_lossy().to_string();

				// Use stable volume GUID, fall back to mount path
				let volume_guid = super::volume_guid(mount_point).unwrap_or_else(|| {
					warn!(
						"Could not resolve volume GUID for {}, using mount path",
						mount_str
					);
					mount_str.to_string()
				});

				// Query ReFS version and block cloning via IOCTL
				let ioctl = if fs_name == "ReFS" {
					check_refs_version_cached(mount_point)
				} else {
					None
				};

				Ok(RefsVolumeInfo {
					volume_guid,
					file_system: fs_name,
					drive_letter: mount_str.chars().next(),
					label: Some(disk.name().to_string_lossy().to_string()),
					size_bytes: disk.total_space(),
					available_bytes: disk.available_space(),
					disk_number: None,
					partition_number: None,
					media_type: Some(if disk.is_removable() {
						"Removable".to_string()
					} else {
						"Fixed".to_string()
					}),
					supports_block_cloning: ioctl.map_or(false, |r| r.supports_block_cloning),
					refs_major_version: ioctl.map(|r| r.major_version),
					refs_minor_version: ioctl.map(|r| r.minor_version),
				})
			} else {
				Err(crate::volume::error::VolumeError::NotFound(format!(
					"No volume found for path {}",
					path.display()
				)))
			}
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}

	/// Get all ReFS volumes on the system.
	pub async fn get_all_refs_volumes(&self) -> VolumeResult<Vec<RefsVolumeInfo>> {
		task::spawn_blocking(|| {
			let disks = sysinfo::Disks::new_with_refreshed_list();
			let mut refs_volumes = Vec::new();

			for disk in &disks {
				let fs_name = disk.file_system().to_string_lossy().to_string();
				if fs_name == "ReFS" {
					let mount_point = disk.mount_point();
					let mount_str = mount_point.to_string_lossy();

					let volume_guid =
						super::volume_guid(mount_point).unwrap_or_else(|| mount_str.to_string());

					let ioctl = check_refs_version_cached(mount_point);

					refs_volumes.push(RefsVolumeInfo {
						volume_guid,
						file_system: fs_name,
						drive_letter: mount_str.chars().next(),
						label: Some(disk.name().to_string_lossy().to_string()),
						size_bytes: disk.total_space(),
						available_bytes: disk.available_space(),
						disk_number: None,
						partition_number: None,
						media_type: Some(if disk.is_removable() {
							"Removable".to_string()
						} else {
							"Fixed".to_string()
						}),
						supports_block_cloning: ioctl.map_or(false, |r| r.supports_block_cloning),
						refs_major_version: ioctl.map(|r| r.major_version),
						refs_minor_version: ioctl.map(|r| r.minor_version),
					});
				}
			}

			Ok(refs_volumes)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}
}

#[async_trait]
impl super::FilesystemHandler for RefsHandler {
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()> {
		if let Ok(info) = self.get_volume_info(&volume.mount_point).await {
			volume.supports_block_cloning = info.supports_block_cloning;
			if let (Some(major), Some(minor)) = (info.refs_major_version, info.refs_minor_version) {
				debug!(
					"ReFS v{}.{} at {}: block cloning = {}",
					major,
					minor,
					volume.mount_point.display(),
					info.supports_block_cloning
				);
			}
		}
		Ok(())
	}

	async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		self.same_physical_storage(path1, path2).await
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		Box::new(crate::ops::files::copy::strategy::FastCopyStrategy)
	}

	fn contains_path(&self, volume: &Volume, path: &std::path::Path) -> bool {
		let normalized_path = crate::common::utils::strip_windows_extended_prefix(path.to_path_buf());

		if normalized_path.starts_with(&volume.mount_point) {
			return true;
		}

		volume
			.mount_points
			.iter()
			.any(|mp| normalized_path.starts_with(mp))
	}
}

/// ReFS volume information
#[derive(Debug, Clone)]
pub struct RefsVolumeInfo {
	pub volume_guid: String,
	pub file_system: String,
	pub drive_letter: Option<char>,
	pub label: Option<String>,
	pub size_bytes: u64,
	pub available_bytes: u64,
	pub disk_number: Option<u32>,
	pub partition_number: Option<u32>,
	pub media_type: Option<String>,
	pub supports_block_cloning: bool,
	pub refs_major_version: Option<u32>,
	pub refs_minor_version: Option<u32>,
}

/// Enhance volume with ReFS-specific information.
pub async fn enhance_volume_from_windows(volume: &mut Volume) -> VolumeResult<()> {
	use super::FilesystemHandler;
	RefsHandler::new().enhance_volume(volume).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_refs_volume_info_creation() {
		let info = RefsVolumeInfo {
			volume_guid: "\\\\?\\Volume{abc-123}\\".to_string(),
			file_system: "ReFS".to_string(),
			drive_letter: Some('E'),
			label: Some("DevDrive".to_string()),
			size_bytes: 100_000_000_000,
			available_bytes: 50_000_000_000,
			disk_number: None,
			partition_number: None,
			media_type: Some("Fixed".to_string()),
			supports_block_cloning: true,
			refs_major_version: Some(3),
			refs_minor_version: Some(7),
		};

		assert_eq!(info.file_system, "ReFS");
		assert!(info.supports_block_cloning);
		assert_eq!(info.drive_letter, Some('E'));
		assert_eq!(info.refs_major_version, Some(3));
	}

	#[test]
	fn test_refs_handler_creation() {
		let _handler = RefsHandler::new();
	}
}
