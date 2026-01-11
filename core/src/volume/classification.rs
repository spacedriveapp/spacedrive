//! Volume classification system for platform-aware volume type detection

use crate::volume::types::{FileSystem, VolumeType};
use std::path::Path;

/// Information needed for volume classification
#[derive(Debug, Clone)]
pub struct VolumeDetectionInfo {
	pub mount_point: std::path::PathBuf,
	pub file_system: FileSystem,
	pub total_bytes_capacity: u64,
	pub is_removable: Option<bool>,
	pub is_network_drive: Option<bool>,
	pub device_model: Option<String>,
}

/// Trait for platform-specific volume classification
pub trait VolumeClassifier {
	fn classify(&self, volume_info: &VolumeDetectionInfo) -> VolumeType;
}

/// macOS volume classifier
pub struct MacOSClassifier;

impl VolumeClassifier for MacOSClassifier {
	fn classify(&self, info: &VolumeDetectionInfo) -> VolumeType {
		let mount_str = info.mount_point.to_string_lossy();

		match mount_str.as_ref() {
			// Primary system drive (legacy pre-Catalina)
			"/" => VolumeType::Primary,

			// Primary data volume (modern macOS Catalina+ with APFS system/data split)
			// This is where all user data, applications, and writable files live
			path if path.starts_with("/System/Volumes/Data") => VolumeType::Primary,

			// System internal volumes (preboot, recovery, VM, etc.)
			path if path.starts_with("/System/Volumes/") => VolumeType::System,

			// macOS autofs system and /home mount
			path if mount_str.contains("auto_home")
				|| mount_str.as_ref() == "/home"
				|| info.file_system == FileSystem::Other("autofs".to_string()) =>
			{
				VolumeType::System
			}

			// External drives
			path if path.starts_with("/Volumes/") => {
				if info.is_removable.unwrap_or(false) {
					VolumeType::External
				} else {
					// Could be user-created APFS volume
					VolumeType::Secondary
				}
			}

			// Network mounts
			path if path.starts_with("/Network/") => VolumeType::Network,

			_ => VolumeType::Unknown,
		}
	}
}

/// Windows volume classifier
pub struct WindowsClassifier;

impl VolumeClassifier for WindowsClassifier {
	fn classify(&self, info: &VolumeDetectionInfo) -> VolumeType {
		let mount_str = info.mount_point.to_string_lossy();

		match mount_str.as_ref() {
			// Primary system drive (usually C:)
			"C:\\" => VolumeType::Primary,

			// Recovery and EFI partitions
			path if path.contains("Recovery")
				|| path.contains("EFI")
				|| (info.file_system == FileSystem::FAT32
					&& info.total_bytes_capacity < 1_000_000_000) =>
			{
				VolumeType::System
			}

			// Other drive letters
			path if path.len() == 3 && path.ends_with(":\\") => {
				if info.is_removable.unwrap_or(false) {
					VolumeType::External
				} else {
					VolumeType::Secondary
				}
			}

			// Network drives
			path if path.starts_with("\\\\") => VolumeType::Network,

			_ => VolumeType::Unknown,
		}
	}
}

/// Linux volume classifier
pub struct LinuxClassifier;

impl VolumeClassifier for LinuxClassifier {
	fn classify(&self, info: &VolumeDetectionInfo) -> VolumeType {
		let mount_str = info.mount_point.to_string_lossy();

		match mount_str.as_ref() {
			// Root filesystem
			"/" => VolumeType::Primary,

			// User data partition
			"/home" => VolumeType::UserData,

			// System/virtual filesystems
			path if path.starts_with("/proc")
				|| path.starts_with("/sys")
				|| path.starts_with("/dev")
				|| path.starts_with("/boot") =>
			{
				VolumeType::System
			}

			// External/removable media
			path if path.starts_with("/media/")
				|| path.starts_with("/mnt/")
				|| info.is_removable.unwrap_or(false) =>
			{
				VolumeType::External
			}

			// Network mounts
			path if info.file_system == FileSystem::Other("nfs".to_string())
				|| info.file_system == FileSystem::Other("cifs".to_string()) =>
			{
				VolumeType::Network
			}

			_ => VolumeType::Secondary,
		}
	}
}

/// Fallback classifier for unknown platforms
pub struct UnknownClassifier;

impl VolumeClassifier for UnknownClassifier {
	fn classify(&self, info: &VolumeDetectionInfo) -> VolumeType {
		// Basic classification based on common patterns
		if info.is_removable.unwrap_or(false) {
			VolumeType::External
		} else if info.is_network_drive.unwrap_or(false) {
			VolumeType::Network
		} else {
			VolumeType::Unknown
		}
	}
}

/// Get the appropriate classifier for the current platform
pub fn get_classifier() -> Box<dyn VolumeClassifier> {
	#[cfg(target_os = "macos")]
	return Box::new(MacOSClassifier);

	#[cfg(target_os = "windows")]
	return Box::new(WindowsClassifier);

	#[cfg(target_os = "linux")]
	return Box::new(LinuxClassifier);

	#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
	return Box::new(UnknownClassifier);
}
// test comment
