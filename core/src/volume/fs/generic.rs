//! Generic filesystem handler for unknown/unsupported filesystems

use super::FilesystemHandler;
use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;

/// Get the volume serial number for the volume containing `path`.
///
/// Uses `GetVolumePathNameW` to resolve the actual volume mount point
/// (handles folder-mounted volumes correctly), then queries `GetVolumeInformationW`.
#[cfg(windows)]
fn volume_serial(path: &std::path::Path) -> Option<u32> {
	use std::os::windows::ffi::OsStrExt;
	use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};

	let wide: Vec<u16> = path
		.as_os_str()
		.encode_wide()
		.chain(std::iter::once(0))
		.collect();

	// Resolve the actual volume mount point (e.g. "C:\" or "C:\mount\othervol\")
	let mut root_buf = vec![0u16; 1024];
	if unsafe { GetVolumePathNameW(wide.as_ptr(), root_buf.as_mut_ptr(), root_buf.len() as u32) }
		== 0
	{
		return None;
	}

	let mut serial: u32 = 0;
	let ok = unsafe {
		GetVolumeInformationW(
			root_buf.as_ptr(),
			std::ptr::null_mut(),
			0,
			&mut serial,
			std::ptr::null_mut(),
			std::ptr::null_mut(),
			std::ptr::null_mut(),
			0,
		)
	};
	if ok != 0 {
		Some(serial)
	} else {
		None
	}
}

/// Generic handler for filesystems without specific optimizations
pub struct GenericFilesystemHandler;

#[async_trait]
impl FilesystemHandler for GenericFilesystemHandler {
	async fn enhance_volume(&self, _volume: &mut Volume) -> VolumeResult<()> {
		// No special enhancements for generic filesystems
		Ok(())
	}

	async fn same_physical_storage(
		&self,
		path1: &std::path::Path,
		path2: &std::path::Path,
	) -> bool {
		// For generic filesystems, we can only check if they resolve to the same device
		// This is a conservative approach that may miss some optimizations
		if let (Ok(meta1), Ok(meta2)) = (path1.metadata(), path2.metadata()) {
			// On Unix systems, compare device IDs
			#[cfg(unix)]
			{
				use std::os::unix::fs::MetadataExt;
				return meta1.dev() == meta2.dev();
			}

			// On Windows, compare volume serial numbers via GetVolumeInformationW
			#[cfg(windows)]
			{
				if let (Some(s1), Some(s2)) = (volume_serial(path1), volume_serial(path2)) {
					return s1 == s2;
				}
				return false;
			}
		}

		false
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		// Use streaming copy as the safe default
		Box::new(crate::ops::files::copy::strategy::LocalStreamCopyStrategy)
	}

	fn contains_path(&self, volume: &Volume, path: &std::path::Path) -> bool {
		// Generic implementation: only check mount points
		// Check primary mount point
		if path.starts_with(&volume.mount_point) {
			return true;
		}

		// Check additional mount points
		volume.mount_points.iter().any(|mp| path.starts_with(mp))
	}
}
