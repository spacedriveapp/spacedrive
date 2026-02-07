//! Platform-specific volume detection orchestrator

use crate::volume::{
	error::{VolumeError, VolumeResult},
	fs,
	types::{Volume, VolumeDetectionConfig},
};
use std::collections::HashMap;
use tokio::task;
use tracing::{debug, instrument, warn};
use uuid::Uuid;

/// Detect all volumes on the system using platform-specific methods
#[instrument(skip(config))]
pub async fn detect_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	debug!("Starting volume detection for device {}", device_id);

	let mut volumes = Vec::new();

	// Platform-specific detection
	#[cfg(target_os = "macos")]
	{
		volumes.extend(detect_macos_volumes(device_id, config).await?);
	}

	#[cfg(target_os = "linux")]
	{
		volumes.extend(detect_linux_volumes(device_id, config).await?);
	}

	#[cfg(target_os = "windows")]
	{
		volumes.extend(detect_windows_volumes(device_id, config).await?);
	}

	#[cfg(target_os = "ios")]
	{
		volumes.extend(detect_ios_volumes(device_id, config).await?);
	}

	#[cfg(target_os = "android")]
	{
		volumes.extend(detect_android_volumes(device_id, config).await?);
	}

	// Enhance volumes with filesystem-specific capabilities
	enhance_volumes_with_fs_capabilities(&mut volumes).await?;

	debug!(
		"Detected {} volumes for device {}",
		volumes.len(),
		device_id
	);
	Ok(volumes)
}

/// Enhance detected volumes with filesystem-specific capabilities
async fn enhance_volumes_with_fs_capabilities(volumes: &mut Vec<Volume>) -> VolumeResult<()> {
	for volume in volumes.iter_mut() {
		match &volume.file_system {
			crate::volume::types::FileSystem::APFS => {
				// APFS volumes already have container info from detection
				// But we could add additional APFS-specific metadata here
			}
			#[cfg(target_os = "linux")]
			crate::volume::types::FileSystem::Btrfs => {
				// Add btrfs subvolume and reflink capability detection
				fs::btrfs::enhance_volume_from_mount(volume).await?;
			}
			#[cfg(target_os = "linux")]
			crate::volume::types::FileSystem::ZFS => {
				// Add ZFS pool and clone capability detection
				fs::zfs::enhance_volume_from_mount(volume).await?;
			}
			#[cfg(target_os = "windows")]
			crate::volume::types::FileSystem::ReFS => {
				// Add ReFS block cloning capability detection
				fs::refs::enhance_volume_from_windows(volume).await?;
			}
			#[cfg(target_os = "windows")]
			crate::volume::types::FileSystem::NTFS => {
				// Add NTFS feature detection
				fs::ntfs::enhance_volume_from_windows(volume).await?;
			}
			_ => {
				// No special handling for other filesystems
			}
		}
	}
	Ok(())
}

#[cfg(target_os = "macos")]
async fn detect_macos_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	use crate::volume::fs::apfs;
	use std::process::Command;

	debug!("MACOS_DETECT: Starting macOS volume detection");
	let mut volumes = Vec::new();

	// Detect APFS containers using filesystem-specific module
	let containers = apfs::detect_containers().await?;
	debug!(
		"MACOS_DETECT: Detected {} APFS containers",
		containers.len()
	);

	// Convert APFS containers to volumes
	for container in containers {
		let converted = apfs::containers_to_volumes(container, device_id, config)?;
		debug!(
			"MACOS_DETECT: Container converted to {} volumes",
			converted.len()
		);
		volumes.extend(converted);
	}

	// Detect non-APFS volumes using traditional methods
	let generic_volumes = detect_generic_volumes_macos(device_id, config).await?;
	debug!(
		"MACOS_DETECT: Detected {} generic (non-APFS) volumes",
		generic_volumes.len()
	);
	volumes.extend(generic_volumes);

	debug!("MACOS_DETECT: Total volumes detected: {}", volumes.len());
	Ok(volumes)
}

#[cfg(target_os = "linux")]
async fn detect_linux_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	use crate::volume::platform::linux;

	debug!("Starting Linux volume detection");
	let mut volumes = linux::detect_volumes(device_id, config).await?;

	// Enhance with filesystem-specific capabilities
	for volume in &mut volumes {
		match &volume.file_system {
			crate::volume::types::FileSystem::Btrfs => {
				fs::btrfs::enhance_volume_from_mount(volume).await?;
			}
			crate::volume::types::FileSystem::ZFS => {
				fs::zfs::enhance_volume_from_mount(volume).await?;
			}
			_ => {}
		}
	}

	Ok(volumes)
}

#[cfg(target_os = "windows")]
async fn detect_windows_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	use crate::volume::platform::windows;

	debug!("Starting Windows volume detection");
	let mut volumes = windows::detect_volumes(device_id, config).await?;

	// Enhance with filesystem-specific capabilities
	for volume in &mut volumes {
		match &volume.file_system {
			crate::volume::types::FileSystem::ReFS => {
				fs::refs::enhance_volume_from_windows(volume).await?;
			}
			crate::volume::types::FileSystem::NTFS => {
				fs::ntfs::enhance_volume_from_windows(volume).await?;
			}
			_ => {}
		}
	}

	Ok(volumes)
}

#[cfg(target_os = "macos")]
async fn detect_generic_volumes_macos(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	use crate::volume::platform::macos;
	macos::detect_non_apfs_volumes(device_id, config).await
}

#[cfg(target_os = "ios")]
async fn detect_ios_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	use crate::volume::platform::ios;

	debug!("Starting iOS volume detection");
	ios::detect_volumes(device_id, config).await
}

#[cfg(target_os = "android")]
async fn detect_android_volumes(
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	use crate::volume::platform::android;

	debug!("Starting Android volume detection");
	android::detect_volumes(device_id, config).await
}
