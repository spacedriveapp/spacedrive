//! Volume management for Spacedrive Core v2
//!
//! This module provides functionality for detecting, monitoring, and managing storage volumes
//! across different platforms. It's designed to integrate with the copy system for optimal
//! file operation routing.

mod error;
mod manager;
mod os_detection;
mod speed;
pub mod types;

pub use error::VolumeError;
pub use manager::VolumeManager;
pub use types::{
	DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig, VolumeEvent, VolumeFingerprint,
	VolumeInfo,
};

// Re-export platform-specific detection
pub use os_detection::detect_volumes;

/// Extension trait for Volume operations
pub trait VolumeExt {
	/// Checks if volume is mounted and accessible
	async fn is_available(&self) -> bool;

	/// Checks if volume has enough free space
	fn has_space(&self, required_bytes: u64) -> bool;

	/// Check if path is on this volume
	fn contains_path(&self, path: &std::path::Path) -> bool;
}

impl VolumeExt for Volume {
	async fn is_available(&self) -> bool {
		self.is_mounted && tokio::fs::metadata(&self.mount_point).await.is_ok()
	}

	fn has_space(&self, required_bytes: u64) -> bool {
		self.total_bytes_available >= required_bytes
	}

	fn contains_path(&self, path: &std::path::Path) -> bool {
		// Check primary mount point
		if path.starts_with(&self.mount_point) {
			return true;
		}

		// Check additional mount points (for APFS volumes)
		self.mount_points.iter().any(|mp| path.starts_with(mp))
	}
}

/// Utilities for volume operations
pub mod util {
	use super::*;
	use std::path::Path;

	/// Check if a path is on the specified volume
	pub fn is_path_on_volume(path: &Path, volume: &Volume) -> bool {
		volume.contains_path(&path.to_path_buf())
	}

	/// Calculate relative path from volume mount point
	pub fn relative_path_on_volume(path: &Path, volume: &Volume) -> Option<std::path::PathBuf> {
		// Try primary mount point first
		if let Ok(relative) = path.strip_prefix(&volume.mount_point) {
			return Some(relative.to_path_buf());
		}

		// Try additional mount points
		for mount_point in &volume.mount_points {
			if let Ok(relative) = path.strip_prefix(mount_point) {
				return Some(relative.to_path_buf());
			}
		}

		None
	}

	/// Find the volume that contains the given path
	pub fn find_volume_for_path<'a>(
		path: &Path,
		volumes: impl Iterator<Item = &'a Volume>,
	) -> Option<&'a Volume> {
		volumes
			.filter(|vol| vol.contains_path(&path.to_path_buf()))
			.max_by_key(|vol| vol.mount_point.as_os_str().len()) // Prefer most specific mount
	}
}
