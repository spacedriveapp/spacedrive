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

/// The zfs and zpool binaries live in /sbin or /usr/sbin, which a daemon
/// launched outside a login shell (TrueNAS, systemd, ssh non-interactive)
/// often does not have on PATH. Resolve an absolute path once so detection
/// does not depend on the launching environment.
fn resolve_bin(name: &'static str, candidates: &[&'static str]) -> &'static str {
	candidates
		.iter()
		.find(|path| Path::new(path).exists())
		.copied()
		.unwrap_or(name)
}

fn zfs_bin() -> &'static str {
	static BIN: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
	BIN.get_or_init(|| {
		resolve_bin(
			"zfs",
			&["/sbin/zfs", "/usr/sbin/zfs", "/usr/local/sbin/zfs"],
		)
	})
}

fn zpool_bin() -> &'static str {
	static BIN: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
	BIN.get_or_init(|| {
		resolve_bin(
			"zpool",
			&["/sbin/zpool", "/usr/sbin/zpool", "/usr/local/sbin/zpool"],
		)
	})
}

/// The columns every `zfs list` here asks for.
///
/// `mounted`, `encryption` and `keystatus` are what tell a dataset whose key
/// is not loaded apart from one that is merely unmounted; `canmount` tells
/// a dataset that never mounts (a container for its children) from one that
/// is away. All of them are readable without privileges.
const LIST_COLUMNS: &str =
	"name,mountpoint,used,available,type,mounted,encryption,keystatus,canmount";

/// The first five columns, for a zfs too old to know `encryption` and
/// `keystatus`. [`ZfsDatasetInfo::parse_line`] reads such output as every
/// dataset mounted and unlocked, which is what the mount table said before.
const BASIC_LIST_COLUMNS: &str = "name,mountpoint,used,available,type";

/// Whether a zfs binary can be run, so detection can list datasets the
/// mount table does not show. A binary outside the usual prefixes (NixOS,
/// a custom install) is found through PATH the way `Command` finds it.
pub fn zfs_available() -> bool {
	let bin = zfs_bin();
	if Path::new(bin).is_absolute() {
		return Path::new(bin).exists();
	}
	std::env::var_os("PATH")
		.is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).exists()))
}

/// Run `zfs list -H -t filesystem` with `args` appended, asking for the
/// state columns first and falling back to the basic five when the zfs
/// rejects them.
fn zfs_list(args: &[&str]) -> VolumeResult<String> {
	let mut last_error = None;
	for columns in [LIST_COLUMNS, BASIC_LIST_COLUMNS] {
		let output = Command::new(zfs_bin())
			.args(["list", "-H", "-o", columns, "-t", "filesystem"])
			.args(args)
			.output()
			.map_err(|e| {
				crate::volume::error::VolumeError::platform(format!(
					"Failed to run zfs list: {}",
					e
				))
			})?;
		if output.status.success() {
			return Ok(String::from_utf8_lossy(&output.stdout).to_string());
		}
		last_error = Some(String::from_utf8_lossy(&output.stderr).trim().to_string());
	}
	Err(crate::volume::error::VolumeError::platform(format!(
		"zfs list command failed: {}",
		last_error.unwrap_or_default()
	)))
}

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

		task::spawn_blocking(move || find_dataset_for_path(&zfs_list(&[])?, &path))
			.await
			.map_err(|e| {
				crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
			})?
	}

	/// Get ZFS pool information
	pub async fn get_pool_info(&self, pool_name: &str) -> VolumeResult<ZfsPoolInfo> {
		let pool_name = pool_name.to_string();

		task::spawn_blocking(move || {
			let output = Command::new(zpool_bin())
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

		task::spawn_blocking(move || parse_zfs_datasets(&zfs_list(&["-r", &pool_name])?))
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
	/// The dataset is in the mount table.
	pub mounted: bool,
	/// The dataset is encrypted and its key is not loaded, so it cannot
	/// mount until `zfs load-key` runs.
	pub locked: bool,
	/// `canmount` is `on` or `noauto`: the dataset mounts at its mount point
	/// when asked. `off` names a dataset that exists only to hold children.
	pub can_mount: bool,
}

impl ZfsDatasetInfo {
	/// Parse one `zfs list -H` line in [`LIST_COLUMNS`] order.
	///
	/// A line with only the first five columns still parses; the state
	/// columns then read as mounted and unlocked, which is what the mount
	/// table said about every dataset before.
	fn parse_line(line: &str) -> Option<Self> {
		let fields: Vec<&str> = line.split('\t').collect();
		if fields.len() < 5 {
			return None;
		}
		let name = fields[0];
		let mountpoint = fields[1];
		let mount_point = (mountpoint != "-" && mountpoint != "legacy" && mountpoint != "none")
			.then(|| PathBuf::from(mountpoint));
		let field = |index: usize| fields.get(index).copied().unwrap_or("-");
		let encrypted = !matches!(field(6), "-" | "off");
		Some(Self {
			name: name.to_string(),
			pool_name: name.split('/').next().unwrap_or(name).to_string(),
			mount_point,
			used_bytes: parse_zfs_size(fields[2]).unwrap_or(0),
			available_bytes: parse_zfs_size(fields[3]).unwrap_or(0),
			dataset_type: fields[4].to_string(),
			readonly: false,
			mounted: field(5) != "no",
			locked: encrypted && field(7) == "unavailable",
			can_mount: field(8) != "off",
		})
	}
}

/// Datasets `zfs list` knows that are not in the mount table but would mount
/// at a real path if asked: an unmounted dataset, or one whose key is not
/// loaded. The mount table cannot show these, which is why a locked dataset
/// used to read as an empty directory on its parent.
pub fn unmounted_datasets(zfs_list_output: &str) -> Vec<ZfsDatasetInfo> {
	zfs_list_output
		.lines()
		.filter_map(ZfsDatasetInfo::parse_line)
		.filter(|dataset| {
			!dataset.mounted
				&& dataset.can_mount
				&& dataset.dataset_type == "filesystem"
				&& dataset.mount_point.is_some()
		})
		.collect()
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
	zfs_list_output
		.lines()
		.filter_map(ZfsDatasetInfo::parse_line)
		.filter(|dataset| {
			dataset
				.mount_point
				.as_ref()
				.is_some_and(|mount| target_path.starts_with(mount))
		})
		.max_by_key(|dataset| dataset.mount_point.as_ref().map(|m| m.as_os_str().len()))
		.ok_or_else(|| {
			crate::volume::error::VolumeError::platform(
				"Path not found in any ZFS dataset".to_string(),
			)
		})
}

/// Parse zfs list output to get all datasets
fn parse_zfs_datasets(zfs_list_output: &str) -> VolumeResult<Vec<ZfsDatasetInfo>> {
	Ok(zfs_list_output
		.lines()
		.filter_map(ZfsDatasetInfo::parse_line)
		.collect())
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

/// Fetch `zfs list` output once for reuse across multiple volumes
pub async fn fetch_zfs_list_output() -> VolumeResult<String> {
	task::spawn_blocking(|| zfs_list(&[])).await.map_err(|e| {
		crate::volume::error::VolumeError::platform(format!("Task join error: {}", e))
	})?
}

/// Enhance a volume using pre-fetched `zfs list` output (no subprocess call)
///
/// Also applies ZFS-aware user-visibility rules:
/// - Datasets on known system pools (e.g. `boot-pool` on TrueNAS Scale)
///   are marked as system-level and hidden.
/// - Datasets under known app/container parent paths (e.g. `ix-applications`)
///   are hidden.
///
/// Capacity fix: for pool-root datasets, `df` under-reports Size because it
/// only counts the root dataset's *own* used bytes plus avail. Descendant
/// datasets hold the real data but `df` can't see them from the root. ZFS's
/// `zfs list` exposes the root's `used` property which *does* include
/// descendants, so we override `total_capacity` with `used + available` to
/// reflect the pool's true usable capacity (e.g. 62 TB instead of 15 TB on
/// a 60 TB raidz2 pool that's mostly full of data in child datasets).
pub fn enhance_volume_with_cached_output(volume: &mut Volume, zfs_list_output: &str) {
	if let Some(mount_point) = volume.mount_point.to_str() {
		if let Ok(dataset_info) = find_dataset_for_path(zfs_list_output, Path::new(mount_point)) {
			debug!("Enhanced ZFS volume with dataset info: {:?}", dataset_info);

			// If this volume IS the pool root (dataset name equals pool name),
			// overwrite the df-derived capacity with the pool-wide total from
			// `zfs list`. Only the pool root carries descendant-inclusive
			// `used`, so this correction only applies there — leaf datasets
			// would over-report if we did it for them (each would claim the
			// whole pool's capacity).
			if dataset_info.name == dataset_info.pool_name {
				let pool_total = dataset_info
					.used_bytes
					.saturating_add(dataset_info.available_bytes);
				debug!(
					"ZFS pool root '{}' at {}: overriding total_capacity {} → {} (used={}, avail={})",
					dataset_info.pool_name,
					mount_point,
					volume.total_capacity,
					pool_total,
					dataset_info.used_bytes,
					dataset_info.available_bytes,
				);
				volume.total_capacity = pool_total;
				volume.available_space = dataset_info.available_bytes;
			}

			if is_system_zfs_pool(&dataset_info.pool_name) {
				debug!(
					"VISIBILITY: Hiding ZFS volume on system pool '{}': {}",
					dataset_info.pool_name, mount_point
				);
				volume.is_user_visible = false;
				volume.auto_track_eligible = false;
				volume.volume_type = crate::volume::types::VolumeType::System;
			} else if is_app_managed_dataset(&dataset_info.name) {
				debug!(
					"VISIBILITY: Hiding app-managed ZFS dataset '{}': {}",
					dataset_info.name, mount_point
				);
				volume.is_user_visible = false;
				volume.auto_track_eligible = false;
			}
		}
	}
}

/// Known ZFS pool names that contain the operating system rather than user
/// data. Datasets on these pools are never user-visible.
///
/// TrueNAS Scale uses `boot-pool` for its OS. Other appliances may add more
/// entries here as we encounter them.
fn is_system_zfs_pool(pool_name: &str) -> bool {
	matches!(pool_name, "boot-pool" | "rpool" | "zroot")
}

/// Heuristic: a ZFS dataset is "app-managed" if its hierarchical name
/// contains a segment that identifies it as belonging to a container/app
/// management layer rather than user-visible storage.
///
/// e.g. `pool/ix-applications/releases/plex/volumes/ix_volumes/ix-plex_data`
fn is_app_managed_dataset(dataset_name: &str) -> bool {
	dataset_name.contains("/ix-applications/")
		|| dataset_name.contains("/.ix-apps/")
		|| dataset_name.contains("/docker/")
		|| dataset_name.contains("/containerd/")
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
	fn a_dataset_whose_key_is_not_loaded_is_locked_not_merely_unmounted() {
		let output = "tank\t/tank\t100M\t900M\tfilesystem\tyes\toff\t-\ton\n\
			tank/vault\t/tank/vault\t0\t900M\tfilesystem\tno\taes-256-gcm\tunavailable\ton\n\
			tank/spare\t/tank/spare\t0\t900M\tfilesystem\tno\toff\t-\ton\n\
			tank/open\t/tank/open\t0\t900M\tfilesystem\tyes\taes-256-gcm\tavailable\ton\n\
			tank/holder\t/tank/holder\t0\t900M\tfilesystem\tno\toff\t-\toff\n\
			tank/legacy\tlegacy\t0\t900M\tfilesystem\tno\toff\t-\ton\n\
			tank/none\tnone\t0\t900M\tfilesystem\tno\toff\t-\ton";

		let away = unmounted_datasets(output);
		let names: Vec<&str> = away.iter().map(|d| d.name.as_str()).collect();
		assert_eq!(names, ["tank/vault", "tank/spare"]);
		assert!(away[0].locked, "an unavailable key is a locked dataset");
		assert!(!away[1].locked, "no key status is merely unmounted");
		assert_eq!(away[0].mount_point, Some(PathBuf::from("/tank/vault")));

		let open = find_dataset_for_path(output, Path::new("/tank/open/file")).unwrap();
		assert!(open.mounted && !open.locked);
	}

	#[test]
	fn five_column_output_still_parses_as_mounted() {
		let dataset = ZfsDatasetInfo::parse_line("tank\t/tank\t100M\t900M\tfilesystem").unwrap();
		assert!(dataset.mounted && !dataset.locked && dataset.can_mount);
		assert!(unmounted_datasets("tank\t/tank\t100M\t900M\tfilesystem").is_empty());
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
