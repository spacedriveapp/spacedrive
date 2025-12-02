//! ZFS filesystem-specific detection and optimization
//!
//! This module handles ZFS pool and dataset detection and provides ZFS-specific
//! optimizations like clone operations and snapshot-based copies.

use crate::volume::{error::VolumeResult, types::Volume};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::task;
use tracing::{debug, warn};

/// ZFS filesystem handler
pub struct ZfsHandler;

impl ZfsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same ZFS pool
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		// Check if both paths are on ZFS datasets in the same pool
		if let (Ok(dataset1), Ok(dataset2)) = (
			self.get_dataset_info(path1).await,
			self.get_dataset_info(path2).await,
		) {
			// Same pool = same physical storage (can use clones)
			return dataset1.pool_name == dataset2.pool_name;
		}

		false
	}

	/// Get ZFS dataset information for a path
	async fn get_dataset_info(&self, path: &Path) -> VolumeResult<ZfsDatasetInfo> {
		let path = path.to_path_buf();

		task::spawn_blocking(move || {
			// Use zfs list to find the dataset containing this path
			let output = Command::new("zfs")
				.args([
					"list",
					"-H",
					"-o",
					"name,mountpoint,used,available,type",
					"-t",
					"filesystem",
				])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run zfs list: {}",
						e
					))
				})?;

			if !output.status.success() {
				return Err(crate::volume::error::VolumeError::platform(
					"zfs list command failed".to_string(),
				));
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			find_dataset_for_path(&output_text, &path)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}

	/// Get ZFS pool information
	pub async fn get_pool_info(&self, pool_name: &str) -> VolumeResult<ZfsPoolInfo> {
		let pool_name = pool_name.to_string();

		task::spawn_blocking(move || {
			let output = Command::new("zpool")
				.args(["status", "-v", &pool_name])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run zpool status: {}",
						e
					))
				})?;

			if !output.status.success() {
				return Err(crate::volume::error::VolumeError::platform(
					"zpool status command failed".to_string(),
				));
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			parse_zpool_status(&output_text)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}

	/// Check if ZFS clone operations are supported
	pub async fn supports_clones(&self, path: &Path) -> bool {
		// ZFS always supports clones, but check if the dataset allows it
		if let Ok(dataset_info) = self.get_dataset_info(path).await {
			// Check if clones property is enabled (usually is by default)
			return !dataset_info.readonly;
		}
		false
	}

	/// Get all datasets in a pool
	pub async fn get_pool_datasets(&self, pool_name: &str) -> VolumeResult<Vec<ZfsDatasetInfo>> {
		let pool_name = pool_name.to_string();

		task::spawn_blocking(move || {
			let output = Command::new("zfs")
				.args([
					"list",
					"-H",
					"-r",
					"-o",
					"name,mountpoint,used,available,type",
					"-t",
					"filesystem",
					&pool_name,
				])
				.output()
				.map_err(|e| {
					crate::volume::error::VolumeError::platform(format!(
						"Failed to run zfs list: {}",
						e
					))
				})?;

			if !output.status.success() {
				return Err(crate::volume::error::VolumeError::platform(
					"zfs list command failed".to_string(),
				));
			}

			let output_text = String::from_utf8_lossy(&output.stdout);
			parse_zfs_datasets(&output_text)
		})
		.await
		.map_err(|e| {
			crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
		})?
	}
}

#[async_trait]
impl super::FilesystemHandler for ZfsHandler {
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()> {
		// Add ZFS-specific information like pool and dataset details
		if let Some(mount_point) = volume.mount_point.to_str() {
			if let Ok(dataset_info) = self.get_dataset_info(Path::new(mount_point)).await {
				debug!("Enhanced ZFS volume with dataset info: {:?}", dataset_info);
				// Could store dataset info in volume metadata if needed
			}
		}
		Ok(())
	}

	async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		self.same_physical_storage(path1, path2).await
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		// Use fast copy strategy for ZFS (can leverage clones)
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

		// TODO: ZFS-specific logic for datasets and pools
		// ZFS datasets can be mounted at arbitrary locations within the same pool
		// This would require checking if paths are within the same ZFS pool
		// even if they have different mount points

		false
	}
}

/// ZFS dataset information
#[derive(Debug, Clone)]
pub struct ZfsDatasetInfo {
	pub name: String,
	pub pool_name: String,
	pub mount_point: Option<PathBuf>,
	pub used_bytes: u64,
	pub available_bytes: u64,
	pub dataset_type: String,
	pub readonly: bool,
}

/// ZFS pool information
#[derive(Debug, Clone)]
pub struct ZfsPoolInfo {
	pub name: String,
	pub state: String,
	pub status: String,
	pub devices: Vec<String>,
	pub errors: u64,
}

/// Find the ZFS dataset that contains a given path
fn find_dataset_for_path(
	zfs_list_output: &str,
	target_path: &Path,
) -> VolumeResult<ZfsDatasetInfo> {
	let mut best_match: Option<ZfsDatasetInfo> = None;
	let mut best_match_len = 0;

	for line in zfs_list_output.lines() {
		let fields: Vec<&str> = line.split('\t').collect();
		if fields.len() >= 5 {
			let name = fields[0];
			let mountpoint = fields[1];
			let used = fields[2];
			let available = fields[3];
			let dataset_type = fields[4];

			if mountpoint != "-" && mountpoint != "legacy" {
				let mount_path = Path::new(mountpoint);
				if target_path.starts_with(mount_path) && mountpoint.len() > best_match_len {
					let pool_name = name.split('/').next().unwrap_or(name).to_string();

					best_match = Some(ZfsDatasetInfo {
						name: name.to_string(),
						pool_name,
						mount_point: Some(mount_path.to_path_buf()),
						used_bytes: parse_zfs_size(used).unwrap_or(0),
						available_bytes: parse_zfs_size(available).unwrap_or(0),
						dataset_type: dataset_type.to_string(),
						readonly: false, // Would need additional property check
					});
					best_match_len = mountpoint.len();
				}
			}
		}
	}

	best_match.ok_or_else(|| {
		crate::volume::error::VolumeError::platform("Path not found in any ZFS dataset".to_string())
	})
}

/// Parse zfs list output to get all datasets
fn parse_zfs_datasets(zfs_list_output: &str) -> VolumeResult<Vec<ZfsDatasetInfo>> {
	let mut datasets = Vec::new();

	for line in zfs_list_output.lines() {
		let fields: Vec<&str> = line.split('\t').collect();
		if fields.len() >= 5 {
			let name = fields[0];
			let mountpoint = fields[1];
			let used = fields[2];
			let available = fields[3];
			let dataset_type = fields[4];

			let pool_name = name.split('/').next().unwrap_or(name).to_string();
			let mount_point = if mountpoint != "-" && mountpoint != "legacy" {
				Some(PathBuf::from(mountpoint))
			} else {
				None
			};

			datasets.push(ZfsDatasetInfo {
				name: name.to_string(),
				pool_name,
				mount_point,
				used_bytes: parse_zfs_size(used).unwrap_or(0),
				available_bytes: parse_zfs_size(available).unwrap_or(0),
				dataset_type: dataset_type.to_string(),
				readonly: false, // Would need additional property check
			});
		}
	}

	Ok(datasets)
}

/// Parse zpool status output
fn parse_zpool_status(status_output: &str) -> VolumeResult<ZfsPoolInfo> {
	let mut name = String::new();
	let mut state = String::new();
	let mut status = String::new();
	let mut devices = Vec::new();
	let mut errors = 0;

	let mut in_config = false;

	for line in status_output.lines() {
		let line = line.trim();

		if line.starts_with("pool:") {
			name = line.strip_prefix("pool:").unwrap_or("").trim().to_string();
		} else if line.starts_with("state:") {
			state = line.strip_prefix("state:").unwrap_or("").trim().to_string();
		} else if line.starts_with("status:") {
			status = line
				.strip_prefix("status:")
				.unwrap_or("")
				.trim()
				.to_string();
		} else if line.starts_with("config:") {
			in_config = true;
		} else if in_config && line.starts_with("errors:") {
			in_config = false;
			if let Some(error_str) = line
				.strip_prefix("errors:")
				.and_then(|s| s.trim().split_whitespace().next())
			{
				errors = error_str.parse().unwrap_or(0);
			}
		} else if in_config && (line.starts_with("/dev/") || line.contains("disk")) {
			// Extract device names from config section
			if let Some(device) = line.split_whitespace().next() {
				if device.starts_with("/dev/") {
					devices.push(device.to_string());
				}
			}
		}
	}

	Ok(ZfsPoolInfo {
		name,
		state,
		status,
		devices,
		errors,
	})
}

/// Parse ZFS size strings like "123K", "456M", "789G"
fn parse_zfs_size(size_str: &str) -> Option<u64> {
	if size_str == "-" || size_str.is_empty() {
		return Some(0);
	}

	let size_str = size_str.trim();
	let (number_part, unit) = if let Some(pos) = size_str.find(char::is_alphabetic) {
		(&size_str[..pos], &size_str[pos..])
	} else {
		(size_str, "")
	};

	let number: f64 = number_part.parse().ok()?;

	let multiplier = match unit.to_uppercase().as_str() {
		"" | "B" => 1,
		"K" => 1024,
		"M" => 1024 * 1024,
		"G" => 1024 * 1024 * 1024,
		"T" => 1024u64.pow(4),
		"P" => 1024u64.pow(5),
		_ => 1,
	};

	Some((number * multiplier as f64) as u64)
}

/// Enhance volume with ZFS-specific information from mount point
pub async fn enhance_volume_from_mount(volume: &mut Volume) -> VolumeResult<()> {
	use super::FilesystemHandler;

	let handler = ZfsHandler;
	handler.enhance_volume(volume).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_zfs_size() {
		assert_eq!(parse_zfs_size("1024"), Some(1024));
		assert_eq!(parse_zfs_size("1K"), Some(1024));
		assert_eq!(parse_zfs_size("1M"), Some(1024 * 1024));
		assert_eq!(parse_zfs_size("1G"), Some(1024 * 1024 * 1024));
		assert_eq!(
			parse_zfs_size("1.5G"),
			Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64)
		);
		assert_eq!(parse_zfs_size("-"), Some(0));
	}

	#[test]
	fn test_find_dataset_for_path() {
		let zfs_output = "tank\t/tank\t100M\t900M\tfilesystem\ntank/home\t/home\t50M\t450M\tfilesystem\ntank/var\t/var\t25M\t225M\tfilesystem";

		let dataset = find_dataset_for_path(zfs_output, Path::new("/home/user/file.txt")).unwrap();
		assert_eq!(dataset.name, "tank/home");
		assert_eq!(dataset.pool_name, "tank");
		assert_eq!(dataset.mount_point, Some(PathBuf::from("/home")));
	}
}
