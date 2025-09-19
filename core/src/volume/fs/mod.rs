//! Filesystem-specific volume detection and optimization

pub mod apfs;
pub mod generic;

#[cfg(target_os = "linux")]
pub mod btrfs;

#[cfg(target_os = "linux")]
pub mod zfs;

#[cfg(target_os = "windows")]
pub mod refs;

#[cfg(target_os = "windows")]
pub mod ntfs;

use crate::volume::{
	error::VolumeResult,
	types::{FileSystem, Volume},
};
use std::path::Path;

/// Trait for filesystem-specific volume enhancement
#[async_trait::async_trait]
pub trait FilesystemHandler: Send + Sync {
	/// Enhance a volume with filesystem-specific capabilities
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()>;

	/// Check if two paths are on the same physical storage for this filesystem
	async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool;

	/// Get the optimal copy strategy for this filesystem
	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy>;

	/// Check if a path is contained within a volume (filesystem-specific logic)
	/// This allows each filesystem to implement custom path resolution logic
	fn contains_path(&self, volume: &Volume, path: &Path) -> bool;
}

/// Get the appropriate filesystem handler for a given filesystem type
pub fn get_filesystem_handler(filesystem: &FileSystem) -> Box<dyn FilesystemHandler> {
	match filesystem {
		FileSystem::APFS => Box::new(apfs::ApfsHandler::new()),

		#[cfg(target_os = "linux")]
		FileSystem::Btrfs => Box::new(btrfs::BtrfsHandler::new()),

		#[cfg(target_os = "linux")]
		FileSystem::ZFS => Box::new(zfs::ZfsHandler::new()),

		#[cfg(target_os = "windows")]
		FileSystem::ReFS => Box::new(refs::RefsHandler::new()),

		#[cfg(target_os = "windows")]
		FileSystem::NTFS => Box::new(ntfs::NtfsHandler::new()),

		_ => Box::new(generic::GenericFilesystemHandler),
	}
}

/// Check if two paths are on the same physical storage using filesystem-specific logic
pub async fn same_physical_storage(path1: &Path, path2: &Path, filesystem: &FileSystem) -> bool {
	let handler = get_filesystem_handler(filesystem);
	handler.same_physical_storage(path1, path2).await
}

/// Get the optimal copy strategy for a filesystem
pub fn get_copy_strategy(
	filesystem: &FileSystem,
) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
	let handler = get_filesystem_handler(filesystem);
	handler.get_copy_strategy()
}

/// Check if a path is contained within a volume using filesystem-specific logic
pub fn contains_path(volume: &Volume, path: &Path) -> bool {
	let handler = get_filesystem_handler(&volume.file_system);
	handler.contains_path(volume, path)
}
