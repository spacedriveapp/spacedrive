//! NTFS filesystem-specific detection and optimization
//!
//! This module handles NTFS volume detection and provides NTFS-specific
//! optimizations like hardlink and junction point handling.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::path::Path;

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
			if let (Some(g1), Some(g2)) = (super::volume_guid(path1), super::volume_guid(path2)) {
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
