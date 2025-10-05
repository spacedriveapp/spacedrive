//! ReFS filesystem-specific detection and optimization
//!
//! This module handles ReFS volume detection and provides ReFS-specific
//! optimizations like block cloning operations.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::task;
use tracing::{debug, warn};

/// ReFS filesystem handler
pub struct RefsHandler;

impl RefsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same ReFS volume and support block cloning
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		// Check if both paths are on the same ReFS volume
		if let (Ok(vol1), Ok(vol2)) = (
			self.get_volume_info(path1).await,
			self.get_volume_info(path2).await,
		) {
			// Same volume GUID = same physical storage
			return vol1.volume_guid == vol2.volume_guid
				&& vol1.supports_block_cloning
				&& vol2.supports_block_cloning;
		}

		false
	}

	/// Get ReFS volume information for a path
	async fn get_volume_info(&self, path: &Path) -> VolumeResult<RefsVolumeInfo> {
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

	/// Check if ReFS block cloning is supported
	async fn supports_block_cloning(&self, path: &Path) -> bool {
		// ReFS supports block cloning starting from Windows Server 2016 / Windows 10
		// Check if the volume supports the feature
		let path = path.to_path_buf();

		let result = task::spawn_blocking(move || {
			let script = format!(
				r#"
				try {{
					$volume = Get-Volume -FilePath '{}'
					# Check if it's ReFS and supports block cloning
					if ($volume.FileSystem -eq 'ReFS') {{
						# Try to get ReFS-specific features
						$refsVolume = Get-RefsVolume -DriveLetter $volume.DriveLetter -ErrorAction SilentlyContinue
						if ($refsVolume) {{
							# ReFS volumes generally support block cloning
							Write-Output 'true'
						}} else {{
							Write-Output 'false'
						}}
					}} else {{
						Write-Output 'false'
					}}
				}} catch {{
					Write-Output 'false'
				}}
				"#,
				path.display()
			);

			let output = std::process::Command::new("powershell")
				.args(["-Command", &script])
				.output();

			match output {
				Ok(output) if output.status.success() => {
					let output_text = String::from_utf8_lossy(&output.stdout);
					output_text.trim() == "true"
				}
				_ => false,
			}
		})
		.await;

		result.unwrap_or(false)
	}

	/// Get all ReFS volumes on the system
	pub async fn get_all_refs_volumes(&self) -> VolumeResult<Vec<RefsVolumeInfo>> {
		task::spawn_blocking(|| {
			let script = r#"
				Get-Volume | Where-Object { $_.FileSystem -eq 'ReFS' } | ForEach-Object {
					$partition = Get-Partition -DriveLetter $_.DriveLetter -ErrorAction SilentlyContinue
					$disk = if ($partition) { Get-Disk -Number $partition.DiskNumber -ErrorAction SilentlyContinue } else { $null }

					[PSCustomObject]@{
						VolumeGuid = $_.UniqueId
						FileSystem = $_.FileSystem
						DriveLetter = $_.DriveLetter
						Label = $_.FileSystemLabel
						Size = $_.Size
						SizeRemaining = $_.SizeRemaining
						DiskNumber = if ($partition) { $partition.DiskNumber } else { $null }
						PartitionNumber = if ($partition) { $partition.PartitionNumber } else { $null }
						MediaType = if ($disk) { $disk.MediaType } else { $null }
					}
				} | ConvertTo-Json
			"#;

			let output = std::process::Command::new("powershell")
				.args(["-Command", script])
				.output()
				.map_err(|e| crate::volume::error::VolumeError::platform(format!("Failed to run PowerShell: {}", e)))?;

			if !output.status.success() {
				return Ok(Vec::new()); // Return empty if command fails
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			parse_volume_list(&output_text)
		})
		.await
		.map_err(|e| crate::volume::error::VolumeError::platform(format!("Task join error: {}", e)))?
	}
}

#[async_trait]
impl super::FilesystemHandler for RefsHandler {
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()> {
		// Add ReFS-specific information like block cloning support
		if let Some(mount_point) = volume.mount_point.to_str() {
			if self.supports_block_cloning(Path::new(mount_point)).await {
				debug!("ReFS volume supports block cloning: {}", mount_point);
				// Could store this capability in volume metadata
			}
		}
		Ok(())
	}

	async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		self.same_physical_storage(path1, path2).await
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		// Use fast copy strategy for ReFS (leverages block cloning)
		Box::new(crate::ops::files::copy::strategy::FastCopyStrategy)
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

		// TODO: ReFS-specific logic for junction points and mount points
		// Windows can have volumes mounted as folders (mount points) within other volumes
		// This would require checking Windows-specific mount point resolution

		false
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
}

/// Parse PowerShell volume info JSON output
fn parse_volume_info(json_output: &str) -> VolumeResult<RefsVolumeInfo> {
	// Simple JSON parsing - in production, you'd use serde_json
	let json_output = json_output.trim();

	// Extract values using simple string parsing (replace with proper JSON parsing)
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
	let supports_block_cloning = file_system == "ReFS"; // ReFS generally supports block cloning

	Ok(RefsVolumeInfo {
		volume_guid,
		file_system,
		drive_letter,
		label,
		size_bytes,
		available_bytes,
		disk_number,
		partition_number,
		media_type,
		supports_block_cloning,
	})
}

/// Parse PowerShell volume list JSON output
fn parse_volume_list(json_output: &str) -> VolumeResult<Vec<RefsVolumeInfo>> {
	// Simple parsing - in production, use proper JSON parser
	let json_output = json_output.trim();

	if json_output.is_empty() || json_output == "null" {
		return Ok(Vec::new());
	}

	// For now, assume single volume (extend for array parsing)
	match parse_volume_info(json_output) {
		Ok(volume) => Ok(vec![volume]),
		Err(_) => Ok(Vec::new()),
	}
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
		// Skip whitespace
		let remaining = json[start..].trim_start();
		if let Some(end) = remaining.find(|c: char| !c.is_ascii_digit()) {
			let number_str = &remaining[..end];
			return number_str.parse().ok();
		}
	}
	None
}

/// Enhance volume with ReFS-specific information from Windows
pub async fn enhance_volume_from_windows(volume: &mut Volume) -> VolumeResult<()> {
	let handler = RefsHandler::new();
	handler.enhance_volume(volume).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_json_string() {
		let json =
			r#"{"VolumeGuid": "12345678-1234-1234-1234-123456789abc", "FileSystem": "ReFS"}"#;
		assert_eq!(
			extract_json_string(json, "VolumeGuid"),
			Some("12345678-1234-1234-1234-123456789abc".to_string())
		);
		assert_eq!(
			extract_json_string(json, "FileSystem"),
			Some("ReFS".to_string())
		);
		assert_eq!(extract_json_string(json, "NonExistent"), None);
	}

	#[test]
	fn test_extract_json_number() {
		let json = r#"{"Size": 1000000000, "SizeRemaining": 500000000}"#;
		assert_eq!(extract_json_number(json, "Size"), Some(1000000000));
		assert_eq!(extract_json_number(json, "SizeRemaining"), Some(500000000));
		assert_eq!(extract_json_number(json, "NonExistent"), None);
	}
}
