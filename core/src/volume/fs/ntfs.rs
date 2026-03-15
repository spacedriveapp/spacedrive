//! NTFS filesystem-specific detection and optimization
//!
//! This module handles NTFS volume detection and provides NTFS-specific
//! optimizations like hardlink and junction point handling.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::path::Path;

/// Get the volume GUID path (e.g. `\?\Volume{guid}\`) for the volume containing `path`.
///
/// Uses `GetVolumePathNameW` to find the mount point root, then
/// `GetVolumeNameForVolumeMountPointW` to retrieve the stable GUID.
/// Returns `None` if either API call fails.
#[cfg(windows)]
fn volume_guid(path: &Path) -> Option<String> {
	use std::ffi::OsString;
	use std::os::windows::ffi::{OsStrExt, OsStringExt};
	use windows_sys::Win32::Storage::FileSystem::{
		GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
	};

	let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();

	// Step 1: resolve mount point root (e.g. "C:\")
	let mut root_buf = vec![0u16; 261];
	if unsafe { GetVolumePathNameW(wide.as_ptr(), root_buf.as_mut_ptr(), 261) } == 0 {
		return None;
	}

	// Step 2: get stable volume GUID path
	let mut guid_buf = vec![0u16; 50]; // "\?\Volume{GUID}\" is ~49 chars
	if unsafe {
		GetVolumeNameForVolumeMountPointW(root_buf.as_ptr(), guid_buf.as_mut_ptr(), 50)
	} == 0
	{
		return None;
	}

	let len = guid_buf.iter().position(|&c| c == 0).unwrap_or(guid_buf.len());
	Some(OsString::from_wide(&guid_buf[..len]).to_string_lossy().into_owned())
}


/// NTFS filesystem handler
pub struct NtfsHandler;

impl NtfsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same NTFS volume
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		// Use GetVolumeNameForVolumeMountPointW to obtain the stable volume GUID
		// (e.g. \?\Volume{guid}\) and compare — no PowerShell required.
		#[cfg(windows)]
		{
			if let (Some(g1), Some(g2)) = (volume_guid(path1), volume_guid(path2)) {
				return g1 == g2;
			}
		}
		false
	}

}

#[async_trait]
impl super::FilesystemHandler for NtfsHandler {
	async fn enhance_volume(&self, _volume: &mut Volume) -> VolumeResult<()> {
		Ok(())
	}

	async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		self.same_physical_storage(path1, path2).await
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		// Use streaming copy for NTFS (no built-in CoW like APFS/ReFS)
		// Could potentially use hardlinks for same-volume copies
		Box::new(crate::ops::files::copy::strategy::LocalStreamCopyStrategy)
	}

	fn contains_path(&self, volume: &Volume, path: &std::path::Path) -> bool {
		// Check primary mount point
		if path.starts_with(&volume.mount_point) {
			return true;
		}

		// Check additional mount points
		if volume.mount_points.iter().any(|mp| path.starts_with(mp)) {
			return true;
		}

		// TODO: NTFS-specific logic for junction points and mount points
		// Windows can have volumes mounted as folders (mount points) within other volumes
		// NTFS also supports junction points and symbolic links that may need resolution

		false
	}
}

/// Enhance volume with NTFS-specific information from Windows
pub async fn enhance_volume_from_windows(volume: &mut Volume) -> VolumeResult<()> {
	use crate::volume::fs::FilesystemHandler;

	let handler = NtfsHandler::new();
	handler.enhance_volume(volume).await
}

