//! NTFS filesystem-specific detection and optimization
//!
//! This module handles NTFS volume detection and provides NTFS-specific
//! optimizations like hardlink and junction point handling.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::task;
use tracing::{debug, warn};

/// NTFS filesystem handler
pub struct NtfsHandler;

impl NtfsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same NTFS volume
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		// Check if both paths are on the same NTFS volume
		if let (Ok(vol1), Ok(vol2)) = (
			self.get_volume_info(path1).await,
			self.get_volume_info(path2).await,
		) {
			// Same volume GUID = same physical storage
			return vol1.volume_guid == vol2.volume_guid;
		}

		false
	}

	/// Get NTFS volume information for a path
	async fn get_volume_info(&self, path: &Path) -> VolumeResult<NtfsVolumeInfo> {
		let path = path.to_path_buf();

		task::spawn_blocking(move || {
			// Use PowerShell to get volume information
			let script = format!(
				r#"
				$volume = Get-Volume -FilePath '{}'
				$partition = Get-Partition -DriveLetter $volume.DriveLetter
				$disk = Get-Disk -Number $partition.DiskNumber

				[PSCustomObject]@{{
					VolumeGuid = $volume.UniqueId
					FileSystem = $volume.FileSystem
					DriveLetter = $volume.DriveLetter
					Label = $volume.FileSystemLabel
					Size = $volume.Size
					SizeRemaining = $volume.SizeRemaining
					DiskNumber = $partition.DiskNumber
					PartitionNumber = $partition.PartitionNumber
					MediaType = $disk.MediaType
				}} | ConvertTo-Json
				"#,
				path.display()
			);

			let output = std::process::Command::new("powershell")
				.args(["-Command", &script])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run PowerShell: {}",
						e
					))
				})?;

			if !output.status.success() {
				return Err(crate::volume::error::VolumeError::platform(
					"PowerShell command failed".to_string(),
				));
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			parse_volume_info(&output_text)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}

	/// Check if NTFS hardlinks are supported (they always are on NTFS)
	pub async fn supports_hardlinks(&self, path: &Path) -> bool {
		// NTFS always supports hardlinks
		if let Ok(vol_info) = self.get_volume_info(path).await {
			return vol_info.file_system == "NTFS";
		}
		false
	}

	/// Check if NTFS junction points are supported
	pub async fn supports_junctions(&self, path: &Path) -> bool {
		// NTFS supports junction points (directory symbolic links)
		if let Ok(vol_info) = self.get_volume_info(path).await {
			return vol_info.file_system == "NTFS";
		}
		false
	}

	/// Resolve junction points and symbolic links
	pub async fn resolve_ntfs_path(&self, path: &Path) -> PathBuf {
		let path = path.to_path_buf();

		let result = task::spawn_blocking(move || {
			// Use PowerShell to resolve the path
			let script = format!(
				r#"
				try {{
					$resolvedPath = Resolve-Path -Path '{}' -ErrorAction Stop
					Write-Output $resolvedPath.Path
				}} catch {{
					Write-Output '{}'
				}}
				"#,
				path.display(),
				path.display()
			);

			let output = std::process::Command::new("powershell")
				.args(["-Command", &script])
				.output();

			match output {
				Ok(output) if output.status.success() => {
					let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
					if !resolved.is_empty() {
						PathBuf::from(resolved)
					} else {
						path
					}
				}
				_ => path,
			}
		})
		.await;

		result.unwrap_or(path)
	}

	/// Get NTFS file system features
	pub async fn get_ntfs_features(&self, path: &Path) -> VolumeResult<NtfsFeatures> {
		let path = path.to_path_buf();

		task::spawn_blocking(move || {
			// Use fsutil to get NTFS features
			let script = format!(
				r#"
				$driveLetter = Split-Path -Path '{}' -Qualifier
				$features = @{{}}

				# Check for compression support
				try {{
					$compressionInfo = fsutil behavior query DisableCompression 2>$null
					$features.SupportsCompression = $true
				}} catch {{
					$features.SupportsCompression = $false
				}}

				# Check for encryption support
				try {{
					$encryptionInfo = fsutil behavior query DisableEncryption 2>$null
					$features.SupportsEncryption = $true
				}} catch {{
					$features.SupportsEncryption = $false
				}}

				# NTFS always supports these
				$features.SupportsHardlinks = $true
				$features.SupportsJunctions = $true
				$features.SupportsSymlinks = $true
				$features.SupportsStreams = $true

				$features | ConvertTo-Json
				"#,
				path.display()
			);

			let output = std::process::Command::new("powershell")
				.args(["-Command", &script])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run PowerShell: {}",
						e
					))
				})?;

			if !output.status.success() {
				// Return default NTFS features
				return Ok(NtfsFeatures {
					supports_hardlinks: true,
					supports_junctions: true,
					supports_symlinks: true,
					supports_streams: true,
					supports_compression: true,
					supports_encryption: true,
				});
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			parse_ntfs_features(&output_text)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}
}

#[async_trait]
impl super::FilesystemHandler for NtfsHandler {
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()> {
		// Add NTFS-specific information like feature support
		if let Some(mount_point) = volume.mount_point.to_str() {
			if let Ok(features) = self.get_ntfs_features(Path::new(mount_point)).await {
				debug!("Enhanced NTFS volume with features: {:?}", features);
				// Could store NTFS features in volume metadata
			}
		}
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

/// NTFS volume information
#[derive(Debug, Clone)]
pub struct NtfsVolumeInfo {
	pub volume_guid: String,
	pub file_system: String,
	pub drive_letter: Option<char>,
	pub label: Option<String>,
	pub size_bytes: u64,
	pub available_bytes: u64,
	pub disk_number: Option<u32>,
	pub partition_number: Option<u32>,
	pub media_type: Option<String>,
}

/// NTFS filesystem features
#[derive(Debug, Clone)]
pub struct NtfsFeatures {
	pub supports_hardlinks: bool,
	pub supports_junctions: bool,
	pub supports_symlinks: bool,
	pub supports_streams: bool,
	pub supports_compression: bool,
	pub supports_encryption: bool,
}

/// Parse PowerShell volume info JSON output
fn parse_volume_info(json_output: &str) -> VolumeResult<NtfsVolumeInfo> {
	// Simple JSON parsing - in production, you'd use serde_json
	let json_output = json_output.trim();

	let volume_guid = extract_json_string(json_output, "VolumeGuid").unwrap_or_default();
	let file_system = extract_json_string(json_output, "FileSystem").unwrap_or_default();
	let drive_letter_str = extract_json_string(json_output, "DriveLetter");
	let label = extract_json_string(json_output, "Label");
	let size_bytes = extract_json_number(json_output, "Size").unwrap_or(0);
	let available_bytes = extract_json_number(json_output, "SizeRemaining").unwrap_or(0);
	let disk_number = extract_json_number(json_output, "DiskNumber").map(|n| n as u32);
	let partition_number = extract_json_number(json_output, "PartitionNumber").map(|n| n as u32);
	let media_type = extract_json_string(json_output, "MediaType");

	let drive_letter = drive_letter_str.and_then(|s| s.chars().next());

	Ok(NtfsVolumeInfo {
		volume_guid,
		file_system,
		drive_letter,
		label,
		size_bytes,
		available_bytes,
		disk_number,
		partition_number,
		media_type,
	})
}

/// Parse NTFS features JSON output
fn parse_ntfs_features(json_output: &str) -> VolumeResult<NtfsFeatures> {
	// Simple parsing - in production, use proper JSON parser
	let json_output = json_output.trim();

	let supports_compression =
		extract_json_bool(json_output, "SupportsCompression").unwrap_or(true);
	let supports_encryption = extract_json_bool(json_output, "SupportsEncryption").unwrap_or(true);

	Ok(NtfsFeatures {
		supports_hardlinks: true, // NTFS always supports these
		supports_junctions: true,
		supports_symlinks: true,
		supports_streams: true,
		supports_compression,
		supports_encryption,
	})
}

/// Extract string value from JSON (simple implementation)
fn extract_json_string(json: &str, key: &str) -> Option<String> {
	let pattern = format!("\"{}\":", key);
	if let Some(start) = json.find(&pattern) {
		let start = start + pattern.len();
		if let Some(value_start) = json[start..].find('"') {
			let value_start = start + value_start + 1;
			if let Some(value_end) = json[value_start..].find('"') {
				let value = &json[value_start..value_start + value_end];
				if value != "null" && !value.is_empty() {
					return Some(value.to_string());
				}
			}
		}
	}
	None
}

/// Extract number value from JSON (simple implementation)
fn extract_json_number(json: &str, key: &str) -> Option<u64> {
	let pattern = format!("\"{}\":", key);
	if let Some(start) = json.find(&pattern) {
		let start = start + pattern.len();
		let remaining = json[start..].trim_start();
		if let Some(end) = remaining.find(|c: char| !c.is_ascii_digit()) {
			let number_str = &remaining[..end];
			return number_str.parse().ok();
		}
	}
	None
}

/// Extract boolean value from JSON (simple implementation)
fn extract_json_bool(json: &str, key: &str) -> Option<bool> {
	let pattern = format!("\"{}\":", key);
	if let Some(start) = json.find(&pattern) {
		let start = start + pattern.len();
		let remaining = json[start..].trim_start();
		if remaining.starts_with("true") {
			return Some(true);
		} else if remaining.starts_with("false") {
			return Some(false);
		}
	}
	None
}

/// Enhance volume with NTFS-specific information from Windows
pub async fn enhance_volume_from_windows(volume: &mut Volume) -> VolumeResult<()> {
	use self::FilesystemHandler;

	let handler = NtfsHandler::new();
	handler.enhance_volume(volume).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_json_string() {
		let json =
			r#"{"VolumeGuid": "12345678-1234-1234-1234-123456789abc", "FileSystem": "NTFS"}"#;
		assert_eq!(
			extract_json_string(json, "VolumeGuid"),
			Some("12345678-1234-1234-1234-123456789abc".to_string())
		);
		assert_eq!(
			extract_json_string(json, "FileSystem"),
			Some("NTFS".to_string())
		);
		assert_eq!(extract_json_string(json, "NonExistent"), None);
	}

	#[test]
	fn test_extract_json_bool() {
		let json = r#"{"SupportsCompression": true, "SupportsEncryption": false}"#;
		assert_eq!(extract_json_bool(json, "SupportsCompression"), Some(true));
		assert_eq!(extract_json_bool(json, "SupportsEncryption"), Some(false));
		assert_eq!(extract_json_bool(json, "NonExistent"), None);
	}
}
