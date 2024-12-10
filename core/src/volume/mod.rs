//! Volume management for Spacedrive
//!
//! This module provides functionality for detecting, monitoring, and managing storage volumes
//! across different platforms.
//! Volumes use a fingerprint to identify them as they sometimes are not persisted in the database
//!
pub(crate) mod actor;
// pub(crate) mod cloud;
mod error;
mod os;
mod speed;
mod state;
mod types;
mod volumes;
mod watcher;
use crate::library::LibraryManagerEvent;
use crate::util::mpscrr;

pub use {
	actor::VolumeManagerActor,
	error::VolumeError,
	state::VolumeManagerState,
	types::{
		DiskType, FileSystem, MountType, Volume, VolumeEvent, VolumeFingerprint, VolumeOptions,
	},
	volumes::Volumes,
};

#[derive(Clone)]
pub struct VolumeManagerContext {
	// Used for device identification
	pub device_id: Vec<u8>,
	pub library_event_tx: mpscrr::Receiver<LibraryManagerEvent, ()>,
}

// Extension trait for Volume operations that don't require actor communication
pub trait VolumeExt {
	/// Checks if volume is mounted and accessible
	async fn is_available(&self) -> bool;

	/// Checks if volume has enough free space
	fn has_space(&self, required_bytes: u64) -> bool;
}

impl VolumeExt for Volume {
	async fn is_available(&self) -> bool {
		self.is_mounted && tokio::fs::metadata(&self.mount_point).await.is_ok()
	}

	fn has_space(&self, required_bytes: u64) -> bool {
		self.total_bytes_available >= required_bytes
	}
}

// Re-export platform-specific volume detection
#[cfg(target_os = "linux")]
pub use os::linux::get_volumes;
#[cfg(target_os = "macos")]
pub use os::macos::get_volumes;
#[cfg(target_os = "windows")]
pub use os::windows::get_volumes;

// Internal utilities
pub(crate) mod util {
	use super::*;
	use std::path::Path;

	pub(crate) fn is_path_on_volume(path: &Path, volume: &Volume) -> bool {
		path.starts_with(&volume.mount_point)
	}

	pub(crate) fn calculate_path_on_volume(
		path: &Path,
		volume: &Volume,
	) -> Option<std::path::PathBuf> {
		if is_path_on_volume(path, volume) {
			path.strip_prefix(&volume.mount_point)
				.ok()
				.map(|p| p.to_path_buf())
		} else {
			None
		}
	}
}
