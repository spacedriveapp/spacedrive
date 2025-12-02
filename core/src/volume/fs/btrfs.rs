//! Btrfs filesystem-specific detection and optimization
//!
//! This module handles Btrfs subvolume detection and provides Btrfs-specific
//! optimizations like reflink copy-on-write operations.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::task;
use tracing::{debug, warn};

/// Btrfs filesystem handler
pub struct BtrfsHandler;

impl BtrfsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same Btrfs filesystem and support reflinks
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		// Check if both paths are on Btrfs filesystems
		if let (Ok(fs1), Ok(fs2)) = (
			self.get_filesystem_info(path1).await,
			self.get_filesystem_info(path2).await,
		) {
			// Same filesystem UUID = same physical storage
			return fs1.uuid == fs2.uuid && fs1.supports_reflinks && fs2.supports_reflinks;
		}

		false
	}

	/// Get Btrfs filesystem information for a path
	async fn get_filesystem_info(&self, path: &Path) -> VolumeResult<BtrfsInfo> {
		let path = path.to_path_buf();

		task::spawn_blocking(move || {
			// Use btrfs filesystem show to get UUID and device info
			let output = Command::new("btrfs")
				.args(["filesystem", "show", path.to_str().unwrap_or("/")])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run btrfs: {}",
						e
					))
				})?;

			if !output.status.success() {
				return Err(crate::volume::error::VolumeError::platform(
					"btrfs command failed".to_string(),
				));
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			parse_btrfs_filesystem_info(&output_text)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}

	/// Check if a path supports reflinks
	async fn supports_reflinks(&self, path: &Path) -> bool {
		// Try to get filesystem features
		let path = path.to_path_buf();

		let result = task::spawn_blocking(move || {
			let output = Command::new("btrfs")
				.args(["filesystem", "features", path.to_str().unwrap_or("/")])
				.output();

			match output {
				Ok(output) if output.status.success() => {
					let output_text = String::from_utf8_lossy(&output.stdout);
					output_text.contains("reflink")
				}
				_ => true, // Assume reflinks are supported on Btrfs by default
			}
		})
		.await;

		result.unwrap_or(true)
	}

	/// Get subvolume information for enhanced volume detection
	pub async fn get_subvolume_info(&self, path: &Path) -> VolumeResult<Option<SubvolumeInfo>> {
		let path = path.to_path_buf();

		task::spawn_blocking(move || {
			let output = Command::new("btrfs")
				.args(["subvolume", "show", path.to_str().unwrap_or("/")])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run btrfs subvolume show: {}",
						e
					))
				})?;

			if !output.status.success() {
				return Ok(None); // Not a subvolume or btrfs command failed
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			Ok(Some(parse_subvolume_info(&output_text)?))
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}
}

#[async_trait]
impl super::FilesystemHandler for BtrfsHandler {
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()> {
		// Add Btrfs-specific information like subvolume details
		if let Some(mount_point) = volume.mount_point.to_str() {
			if let Ok(Some(subvol_info)) = self.get_subvolume_info(Path::new(mount_point)).await {
				debug!(
					"Enhanced Btrfs volume with subvolume info: {:?}",
					subvol_info
				);
				// Could store subvolume info in volume metadata if needed
			}
		}
		Ok(())
	}

	async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		self.same_physical_storage(path1, path2).await
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		// Use reflink copy strategy for Btrfs (copy-on-write)
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

		// TODO: Btrfs-specific logic for subvolumes and bind mounts
		// Btrfs can have subvolumes mounted at different locations within the same filesystem
		// This would require checking if paths are within the same Btrfs filesystem UUID
		// even if they have different mount points

		false
	}
}

/// Btrfs filesystem information
#[derive(Debug, Clone)]
struct BtrfsInfo {
	uuid: String,
	label: Option<String>,
	devices: Vec<String>,
	supports_reflinks: bool,
}

/// Btrfs subvolume information
#[derive(Debug, Clone)]
pub struct SubvolumeInfo {
	pub name: String,
	pub uuid: String,
	pub parent_uuid: Option<String>,
	pub creation_time: Option<String>,
	pub subvolume_id: u64,
	pub generation: u64,
}

/// Parse btrfs filesystem show output
fn parse_btrfs_filesystem_info(output: &str) -> VolumeResult<BtrfsInfo> {
	let mut uuid = String::new();
	let mut label = None;
	let mut devices = Vec::new();

	for line in output.lines() {
		let line = line.trim();

		// Parse UUID: "uuid: 12345678-1234-1234-1234-123456789abc"
		if line.starts_with("uuid:") {
			if let Some(uuid_str) = line.split_whitespace().nth(1) {
				uuid = uuid_str.to_string();
			}
		}
		// Parse label: "Label: 'MyVolume'  uuid: ..."
		else if line.starts_with("Label:") {
			if let Some(label_part) = line.split("uuid:").next() {
				if let Some(label_str) = label_part.strip_prefix("Label:").map(|s| s.trim()) {
					if label_str != "none" && !label_str.is_empty() {
						label = Some(label_str.trim_matches('\'').to_string());
					}
				}
			}
		}
		// Parse devices: "	devid    1 size 931.51GiB used 123.45GiB path /dev/sda1"
		else if line.contains("devid") && line.contains("path") {
			if let Some(path_part) = line.split("path").nth(1) {
				devices.push(path_part.trim().to_string());
			}
		}
	}

	if uuid.is_empty() {
		return Err(crate::volume::error::VolumeError::platform(
			"Could not parse Btrfs UUID".to_string(),
		));
	}

	Ok(BtrfsInfo {
		uuid,
		label,
		devices,
		supports_reflinks: true, // Btrfs supports reflinks by default
	})
}

/// Parse btrfs subvolume show output
fn parse_subvolume_info(output: &str) -> VolumeResult<SubvolumeInfo> {
	let mut name = String::new();
	let mut uuid = String::new();
	let mut parent_uuid = None;
	let mut creation_time = None;
	let mut subvolume_id = 0;
	let mut generation = 0;

	for line in output.lines() {
		let line = line.trim();

		if line.starts_with("Name:") {
			name = line.strip_prefix("Name:").unwrap_or("").trim().to_string();
		} else if line.starts_with("UUID:") {
			uuid = line.strip_prefix("UUID:").unwrap_or("").trim().to_string();
		} else if line.starts_with("Parent UUID:") {
			let parent = line.strip_prefix("Parent UUID:").unwrap_or("").trim();
			if parent != "-" {
				parent_uuid = Some(parent.to_string());
			}
		} else if line.starts_with("Creation time:") {
			creation_time = Some(
				line.strip_prefix("Creation time:")
					.unwrap_or("")
					.trim()
					.to_string(),
			);
		} else if line.starts_with("Subvolume ID:") {
			if let Some(id_str) = line
				.strip_prefix("Subvolume ID:")
				.and_then(|s| s.trim().parse().ok())
			{
				subvolume_id = id_str;
			}
		} else if line.starts_with("Generation:") {
			if let Some(gen_str) = line
				.strip_prefix("Generation:")
				.and_then(|s| s.trim().parse().ok())
			{
				generation = gen_str;
			}
		}
	}

	Ok(SubvolumeInfo {
		name,
		uuid,
		parent_uuid,
		creation_time,
		subvolume_id,
		generation,
	})
}

/// Enhance volume with Btrfs-specific information from mount point
pub async fn enhance_volume_from_mount(volume: &mut Volume) -> VolumeResult<()> {
	use super::FilesystemHandler;

	let handler = BtrfsHandler::new();
	handler.enhance_volume(volume).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_btrfs_filesystem_info() {
		let output = r#"
Label: 'MyVolume'  uuid: 12345678-1234-1234-1234-123456789abc
	Total devices 1 FS bytes used 123.45GiB
	devid    1 size 931.51GiB used 456.78GiB path /dev/sda1
"#;

		let info = parse_btrfs_filesystem_info(output).unwrap();
		assert_eq!(info.uuid, "12345678-1234-1234-1234-123456789abc");
		assert_eq!(info.label, Some("MyVolume".to_string()));
		assert_eq!(info.devices, vec!["/dev/sda1"]);
		assert!(info.supports_reflinks);
	}

	#[test]
	fn test_parse_subvolume_info() {
		let output = r#"
/home/user/subvol
	Name: 			subvol
	UUID: 			87654321-4321-4321-4321-210987654321
	Parent UUID: 		12345678-1234-1234-1234-123456789abc
	Received UUID: 		-
	Creation time: 		2023-01-01 12:00:00 +0000
	Subvolume ID: 		256
	Generation: 		123
	Gen at creation: 	100
	Parent ID: 		5
	Top level ID: 		5
	Flags: 			-
	Snapshot(s):
"#;

		let info = parse_subvolume_info(output).unwrap();
		assert_eq!(info.name, "subvol");
		assert_eq!(info.uuid, "87654321-4321-4321-4321-210987654321");
		assert_eq!(
			info.parent_uuid,
			Some("12345678-1234-1234-1234-123456789abc".to_string())
		);
		assert_eq!(info.subvolume_id, 256);
		assert_eq!(info.generation, 123);
	}
}
