//! APFS filesystem-specific detection and optimization
//!
//! This module handles APFS container detection and provides APFS-specific
//! optimizations like copy-on-write cloning. While primarily used on macOS,
//! this module is designed to work on any platform that supports APFS.

use crate::volume::{
	error::{VolumeError, VolumeResult},
	types::{
		ApfsContainer, ApfsVolumeInfo, ApfsVolumeRole, DiskType, FileSystem, PathMapping, Volume,
		VolumeDetectionConfig,
	},
};
use std::path::PathBuf;
use std::process::Command;
use tokio::task;
use tracing::{debug, warn};
use uuid::Uuid;

/// Parse APFS container structure using diskutil
pub async fn detect_containers() -> VolumeResult<Vec<ApfsContainer>> {
	debug!("Starting APFS container detection");

	task::spawn_blocking(|| {
		// Use diskutil apfs list to get container information
		let output = Command::new("diskutil")
			.args(["apfs", "list"])
			.output()
			.map_err(|e| {
				VolumeError::platform(format!("Failed to run diskutil apfs list: {}", e))
			})?;

		if !output.status.success() {
			return Err(VolumeError::platform(format!(
				"diskutil apfs list failed with status: {}",
				output.status
			)));
		}

		let output_text = String::from_utf8_lossy(&output.stdout);
		parse_apfs_list_output(&output_text)
	})
	.await
	.map_err(|e| VolumeError::platform(format!("Task join error: {}", e)))?
}

/// Parse the output of `diskutil apfs list`
fn parse_apfs_list_output(output: &str) -> VolumeResult<Vec<ApfsContainer>> {
	debug!("APFS_PARSE: Starting to parse diskutil output");
	let mut containers = Vec::new();
	let mut current_container: Option<ApfsContainer> = None;
	let mut current_volumes = Vec::new();

	for line in output.lines() {
		let line = line.trim();

		// Container header: "+-- Container disk3 55E8C6B4-C7AC-48F5-B67A-A4B765DE3F41"
		if line.starts_with("+-- Container ") {
			// Save previous container if exists
			if let Some(mut container) = current_container.take() {
				container.volumes = current_volumes.clone();
				containers.push(container);
				current_volumes.clear();
			}

			// Parse new container
			let parts: Vec<&str> = line.split_whitespace().collect();
			if parts.len() >= 4 {
				let container_id = parts[2].to_string(); // e.g., "disk3"
				let uuid = parts[3].to_string();

				current_container = Some(ApfsContainer {
					container_id,
					uuid,
					physical_store: String::new(), // Will be filled later
					total_capacity: 0,             // Will be filled later
					capacity_in_use: 0,            // Will be filled later
					capacity_free: 0,              // Will be filled later
					volumes: Vec::new(),
				});
			}
		}
		// Container capacity info: "Size (Capacity Ceiling): 994662584320 B (994.7 GB)"
		else if line.contains("Size (Capacity Ceiling):") {
			if let Some(container) = &mut current_container {
				if let Some(bytes_str) = extract_bytes_from_line(line) {
					container.total_capacity = bytes_str;
				}
			}
		}
		// Capacity in use: "Capacity In Use By Volumes: 853884600320 B (853.9 GB)"
		else if line.contains("Capacity In Use By Volumes:") {
			if let Some(container) = &mut current_container {
				if let Some(bytes_str) = extract_bytes_from_line(line) {
					container.capacity_in_use = bytes_str;
				}
			}
		}
		// Capacity free: "Capacity Not Allocated: 140777984000 B (140.8 GB)"
		else if line.contains("Capacity Not Allocated:") {
			if let Some(container) = &mut current_container {
				if let Some(bytes_str) = extract_bytes_from_line(line) {
					container.capacity_free = bytes_str;
				}
			}
		}
		// Physical store: "+-< Physical Store disk0s2 138391DF-DFE1-4AF1-ADAD-65B5A50334FA"
		else if line.contains("Physical Store ") {
			if let Some(container) = &mut current_container {
				let parts: Vec<&str> = line.split_whitespace().collect();
				if parts.len() >= 4 {
					container.physical_store = parts[3].to_string(); // e.g., "disk0s2"
				}
			}
		}
		// Volume header: "+-> Volume disk3s5 589962A3-6036-4CAA-BE8E-0E90B5921035"
		// Can be prefixed with "|   " like "|   +-> Volume"
		else if line.starts_with("+-> Volume ") || line.contains("+-> Volume ") {
			let parts: Vec<&str> = line.split_whitespace().collect();
			debug!("APFS_PARSE: Found volume header, parts: {:?}", parts);
			if parts.len() >= 4 {
				let disk_id = parts[2].to_string(); // e.g., "disk3s5"
				let uuid = parts[3].to_string();

				let volume_info = ApfsVolumeInfo {
					disk_id: disk_id.clone(),
					uuid,
					role: ApfsVolumeRole::Other("Unknown".to_string()),
					name: String::new(),
					mount_point: None,
					snapshot_mount_point: None,
					capacity_consumed: 0,
					sealed: false,
					filevault: false,
				};
				debug!("APFS_PARSE: Added volume_info for {}", disk_id);
				current_volumes.push(volume_info);
			} else {
				debug!(
					"APFS_PARSE: Volume header has wrong number of parts: {}",
					parts.len()
				);
			}
		}
		// Volume role: "|   APFS Volume Disk (Role): disk3s5 (Data)"
		else if line.contains("APFS Volume Disk (Role):") && !current_volumes.is_empty() {
			let last_volume = current_volumes.last_mut().unwrap();
			if let Some(role_str) = extract_role_from_line(line) {
				last_volume.role = parse_apfs_role(&role_str);
			}
		}
		// Volume name: "|   Name: Data (Case-insensitive)"
		else if line.contains("Name:") && !current_volumes.is_empty() {
			let last_volume = current_volumes.last_mut().unwrap();
			if let Some(name) = extract_name_from_line(line) {
				last_volume.name = name;
			}
		}
		// Mount point: "|   Mount Point: /System/Volumes/Data"
		else if line.contains("Mount Point:") && !current_volumes.is_empty() {
			let last_volume = current_volumes.last_mut().unwrap();
			debug!("Found mount point line: '{}'", line);

			if line.contains("Snapshot Mount Point:") {
				// Prefer snapshot mount point (e.g., / for root)
				if let Some(mount_point) = extract_mount_point_from_line(line) {
					debug!("Extracted snapshot mount point: '{}'", mount_point);
					last_volume.snapshot_mount_point = Some(PathBuf::from(mount_point));
				}
			} else {
				// Regular volume mount point
				if let Some(mount_point) = extract_mount_point_from_line(line) {
					debug!("Extracted mount point: '{}'", mount_point);
					last_volume.mount_point = Some(PathBuf::from(mount_point));
				}
			}
		}
		// Capacity consumed: "|   Capacity Consumed: 821093748736 B (821.1 GB)"
		else if line.contains("Capacity Consumed:") && !current_volumes.is_empty() {
			let last_volume = current_volumes.last_mut().unwrap();
			if let Some(bytes_str) = extract_bytes_from_line(line) {
				last_volume.capacity_consumed = bytes_str;
			}
		}
		// Sealed status: "|   Sealed: Yes"
		else if line.contains("Sealed:")
			&& !line.contains("Snapshot Sealed:")
			&& !current_volumes.is_empty()
		{
			let last_volume = current_volumes.last_mut().unwrap();
			last_volume.sealed = line.contains("Yes");
		}
		// FileVault status: "|   FileVault: Yes (Unlocked)"
		else if line.contains("FileVault:") && !current_volumes.is_empty() {
			let last_volume = current_volumes.last_mut().unwrap();
			last_volume.filevault = line.contains("Yes");
		}
	}

	// Save the last container
	if let Some(mut container) = current_container {
		container.volumes = current_volumes;
		containers.push(container);
	}

	debug!("APFS_PARSE: Parsed {} containers", containers.len());
	for container in &containers {
		debug!(
			"APFS_PARSE: Container {} has {} volumes",
			container.container_id,
			container.volumes.len()
		);
		for vol in &container.volumes {
			debug!(
				"APFS_PARSE:   Volume '{}' ({}), Mount: {:?}, Role: {:?}",
				vol.name, vol.disk_id, vol.mount_point, vol.role
			);
		}
	}

	Ok(containers)
}

/// Extract byte value from a line like "Size: 994662584320 B (994.7 GB)"
fn extract_bytes_from_line(line: &str) -> Option<u64> {
	// Look for pattern "NUMBER B"
	let parts: Vec<&str> = line.split_whitespace().collect();
	for i in 0..parts.len().saturating_sub(1) {
		if parts[i + 1] == "B" {
			if let Ok(bytes) = parts[i].parse::<u64>() {
				return Some(bytes);
			}
		}
	}
	None
}

/// Extract role from a line like "APFS Volume Disk (Role): disk3s5 (Data)"
fn extract_role_from_line(line: &str) -> Option<String> {
	// Look for text in parentheses at the end
	if let Some(start) = line.rfind('(') {
		if let Some(end) = line.rfind(')') {
			if start < end {
				return Some(line[start + 1..end].to_string());
			}
		}
	}
	None
}

/// Extract name from a line like "|   Name: Data (Case-insensitive)"
fn extract_name_from_line(line: &str) -> Option<String> {
	// Find "Name:" in the line and extract everything after it
	let name_pos = line.find("Name:")?;
	let name_part = line[name_pos + "Name:".len()..].trim();
	if let Some(paren_pos) = name_part.find('(') {
		Some(name_part[..paren_pos].trim().to_string())
	} else {
		Some(name_part.to_string())
	}
}

/// Extract mount point from a line like "|   Mount Point: /System/Volumes/Data"
fn extract_mount_point_from_line(line: &str) -> Option<String> {
	// Find "Mount Point:" in the line and extract everything after it
	let mount_point_pos = line.find("Mount Point:")?;
	let mount_part = line[mount_point_pos + "Mount Point:".len()..].trim();
	if mount_part == "Not Mounted" {
		None
	} else {
		Some(mount_part.to_string())
	}
}

/// Parse APFS volume role from string
fn parse_apfs_role(role_str: &str) -> ApfsVolumeRole {
	match role_str.to_lowercase().as_str() {
		"system" => ApfsVolumeRole::System,
		"data" => ApfsVolumeRole::Data,
		"preboot" => ApfsVolumeRole::Preboot,
		"recovery" => ApfsVolumeRole::Recovery,
		"vm" => ApfsVolumeRole::VM,
		other => ApfsVolumeRole::Other(other.to_string()),
	}
}

/// Convert APFS containers to Volume objects
pub fn containers_to_volumes(
	container: ApfsContainer,
	device_id: Uuid,
	config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	debug!(
		"APFS_CONVERT: Converting container {} with {} volumes, include_system={}",
		container.container_id,
		container.volumes.len(),
		config.include_system
	);
	let mut volumes = Vec::new();

	for volume_info in &container.volumes {
		// Prefer snapshot mount point if available (e.g., / for root instead of /System/Volumes/Update/mnt1)
		let effective_mount_point = volume_info
			.snapshot_mount_point
			.as_ref()
			.or(volume_info.mount_point.as_ref());

		debug!(
			"APFS_CONVERT: Processing volume '{}' role={:?} mount={:?} snapshot={:?}",
			volume_info.name,
			volume_info.role,
			volume_info.mount_point,
			volume_info.snapshot_mount_point
		);

		// Only process mounted volumes (including snapshot mounts)
		if let Some(mount_point) = effective_mount_point {
			// Skip system volumes unless configured to include them
			if !config.include_system
				&& matches!(
					volume_info.role,
					ApfsVolumeRole::System | ApfsVolumeRole::Preboot | ApfsVolumeRole::Recovery
				) {
				debug!(
					"APFS_CONVERT: Skipping system volume: {} ({})",
					volume_info.name, volume_info.role
				);
				continue;
			}

			// Generate path mappings for Data volumes
			let path_mappings = if matches!(volume_info.role, ApfsVolumeRole::Data) {
				generate_macos_path_mappings()
			} else {
				Vec::new()
			};

			// Create stable volume fingerprint for APFS volumes
			// APFS volumes are always local system/primary volumes, use mount_point + device_id
			let fingerprint =
				crate::volume::types::VolumeFingerprint::from_primary_volume(mount_point, device_id);

			debug!(
				"APFS_CONVERT: Generated fingerprint {} for volume '{}' (consumed: {} bytes)",
				fingerprint.short_id(),
				volume_info.name,
				volume_info.capacity_consumed
			);

			// Determine mount and volume types
			let mount_type = determine_mount_type(&volume_info.role, mount_point);
			let volume_type = classify_volume_type(&volume_info.role, mount_point);

			// Determine if volume should be user-visible
			let is_user_visible =
				should_be_user_visible(mount_point, &volume_info.role, &volume_info.name);

			// Auto-track eligibility: Only Primary volume (Data volume on modern macOS)
			let auto_track_eligible =
				matches!(volume_type, crate::volume::types::VolumeType::Primary) && is_user_visible;

			debug!(
				"APFS_CONVERT: Volume '{}' classified as Type={:?}, user_visible={}, auto_track_eligible={}",
				volume_info.name, volume_type, is_user_visible, auto_track_eligible
			);

			// Get space information (total capacity and available space)
			let (total_bytes, available_bytes) = get_volume_space_info(mount_point)?;

			// Create volume with APFS container information
			let now = chrono::Utc::now();

			// Collect all mount points (both regular and snapshot)
			let mut all_mount_points = vec![mount_point.clone()];
			if let Some(physical) = &volume_info.mount_point {
				if physical != mount_point {
					all_mount_points.push(physical.clone());
				}
			}

			// Determine display name - show "Macintosh HD" for the main Data volume
			let display_name = if mount_point.to_string_lossy() == "/System/Volumes/Data" {
				"Macintosh HD".to_string()
			} else {
				volume_info.name.clone()
			};

			let volume = Volume {
				// Use fingerprint to generate stable UUID
				id: uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, fingerprint.0.as_bytes()),
				fingerprint,
				device_id,
				name: volume_info.name.clone(),
				library_id: None,
				is_tracked: false,
				mount_point: mount_point.clone(),
				mount_points: all_mount_points,
				volume_type,
				mount_type,
				disk_type: DiskType::Unknown,
				file_system: FileSystem::APFS,
				total_capacity: total_bytes,
				available_space: available_bytes,
				is_read_only: volume_info.sealed,
				is_mounted: true,
				hardware_id: Some(volume_info.disk_id.clone()),
				backend: None,
				cloud_identifier: None,
				cloud_config: None,
				apfs_container: Some(container.clone()),
				container_volume_id: Some(volume_info.disk_id.clone()),
				path_mappings,
				is_user_visible,
				auto_track_eligible,
				read_speed_mbps: None,
				write_speed_mbps: None,
				created_at: now,
				updated_at: now,
				last_seen_at: now,
				total_files: None,
				total_directories: None,
				last_stats_update: None,
				display_name: Some(display_name),
				is_favorite: false,
				color: None,
				icon: None,
				error_message: None,
			};

			volumes.push(volume);
			debug!(
				"APFS_CONVERT: Added APFS volume: {} at {}",
				volume_info.name,
				mount_point.display()
			);
		} else {
			debug!(
				"APFS_CONVERT: Skipping unmounted volume: {}",
				volume_info.name
			);
		}
	}

	debug!(
		"APFS_CONVERT: Converted {} volumes from container {}",
		volumes.len(),
		container.container_id
	);
	Ok(volumes)
}

/// Determine mount type based on APFS volume role and mount point
fn determine_mount_type(
	role: &ApfsVolumeRole,
	mount_point: &PathBuf,
) -> crate::volume::types::MountType {
	use crate::volume::types::MountType;

	match role {
		ApfsVolumeRole::System
		| ApfsVolumeRole::Preboot
		| ApfsVolumeRole::Recovery
		| ApfsVolumeRole::VM => MountType::System,
		ApfsVolumeRole::Data => MountType::System, // Data volume is still system-level
		ApfsVolumeRole::Other(_) => {
			// For other volumes, check mount point
			if mount_point.starts_with("/Volumes/") {
				MountType::External
			} else {
				MountType::System
			}
		}
	}
}

/// Classify APFS volume type based on role and mount point
fn classify_volume_type(
	role: &ApfsVolumeRole,
	mount_point: &PathBuf,
) -> crate::volume::types::VolumeType {
	use crate::volume::types::VolumeType;

	match role {
		ApfsVolumeRole::System => VolumeType::System,
		// Data volume is the primary volume on modern macOS (Catalina+)
		// This is where all user data, applications, and writable files live
		ApfsVolumeRole::Data => VolumeType::Primary,
		ApfsVolumeRole::Preboot | ApfsVolumeRole::Recovery => VolumeType::System,
		ApfsVolumeRole::VM => VolumeType::System,
		ApfsVolumeRole::Other(_) => {
			if mount_point.starts_with("/Volumes/") {
				VolumeType::External
			} else {
				VolumeType::Secondary
			}
		}
	}
}

/// Determine if a volume should be visible to the user
/// Filters out system volumes that are redundant or not useful for user interaction
fn should_be_user_visible(mount_point: &PathBuf, role: &ApfsVolumeRole, name: &str) -> bool {
	let mount_str = mount_point.to_string_lossy();
	debug!(
		"VISIBILITY: Checking volume: name='{}' role={:?} mount='{}'",
		name, role, mount_str
	);

	// Hide system utility volumes
	match role {
		ApfsVolumeRole::Preboot | ApfsVolumeRole::Recovery | ApfsVolumeRole::VM => return false,
		_ => {}
	}

	// Hide specific mount points
	if mount_str.starts_with("/System/Volumes/Preboot")
		|| mount_str.starts_with("/System/Volumes/VM")
		|| mount_str.starts_with("/System/Volumes/Hardware")
		|| mount_str.starts_with("/System/Volumes/Update")
		|| mount_str.starts_with("/System/Volumes/xarts")
		|| mount_str.starts_with("/System/Volumes/iSCPreboot")
	{
		return false;
	}

	// Hide iOS Simulator volumes
	if mount_str.starts_with("/Library/Developer/CoreSimulator") {
		return false;
	}

	// Hide home autofs mounts (e.g., /System/Volumes/Data/home)
	if name.to_lowercase() == "home" && mount_str.ends_with("/home") {
		debug!(
			"VISIBILITY: Hiding home volume: name='{}' mount='{}'",
			name, mount_str
		);
		return false;
	}

	// Hide snapshot mounts (usually contain @ symbol)
	if mount_str.contains("@") {
		return false;
	}

	// Hide cryptex volumes (e.g., MetalToolchainCryptex)
	if mount_str.starts_with("/private/var/run/com.apple.security.cryptexd/") {
		debug!(
			"VISIBILITY: Hiding cryptex volume: name='{}' mount='{}'",
			name, mount_str
		);
		return false;
	}

	// Hide the root "/" volume if it's a system volume (prefer showing Data volume instead)
	// The Data volume is where actual user files live in modern macOS
	if mount_str.as_ref() == "/" && matches!(role, ApfsVolumeRole::System) {
		return false;
	}

	true
}

/// Get volume space information (platform-specific implementation needed)
fn get_volume_space_info(mount_point: &PathBuf) -> VolumeResult<(u64, u64)> {
	// This would need platform-specific implementation
	// For now, return zeros as placeholder
	#[cfg(target_os = "macos")]
	{
		use std::process::Command;

		let output = Command::new("df")
			.args(["-k", mount_point.to_str().unwrap_or("/")])
			.output()
			.map_err(|e| VolumeError::platform(format!("Failed to run df: {}", e)))?;

		if !output.status.success() {
			return Ok((0, 0));
		}

		let df_stdout = String::from_utf8_lossy(&output.stdout);
		for line in df_stdout.lines().skip(1) {
			let fields: Vec<&str> = line.split_whitespace().collect();
			if fields.len() >= 4 {
				let total_kb = fields[1].parse::<u64>().unwrap_or(0);
				let available_kb = fields[3].parse::<u64>().unwrap_or(0);
				return Ok((total_kb * 1024, available_kb * 1024));
			}
		}
	}

	Ok((0, 0))
}

/// APFS filesystem handler
pub struct ApfsHandler;

impl ApfsHandler {
	pub fn new() -> Self {
		Self
	}

	/// Check if two paths are on the same APFS container
	pub async fn same_physical_storage(
		&self,
		path1: &std::path::Path,
		path2: &std::path::Path,
	) -> bool {
		// Resolve paths to actual storage locations (handle firmlinks)
		let resolved_path1 = self.resolve_apfs_path(path1).await;
		let resolved_path2 = self.resolve_apfs_path(path2).await;

		// Get APFS container information for both paths
		if let (Ok(container1), Ok(container2)) = (
			self.get_container_for_path(&resolved_path1).await,
			self.get_container_for_path(&resolved_path2).await,
		) {
			// Same container = same physical storage
			return container1.container_id == container2.container_id;
		}

		false
	}

	/// Resolve APFS path through firmlinks
	async fn resolve_apfs_path(&self, path: &std::path::Path) -> PathBuf {
		let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
		let path_str = canonical_path.to_string_lossy();

		// Handle common firmlinks to /System/Volumes/Data
		if path_str.starts_with("/Users/") {
			return PathBuf::from(format!("/System/Volumes/Data{}", path_str));
		}
		if path_str.starts_with("/Applications/")
			&& !path_str.starts_with("/Applications/Utilities/")
		{
			return PathBuf::from(format!("/System/Volumes/Data{}", path_str));
		}
		if path_str.starts_with("/Library/") && !path_str.starts_with("/Library/Apple/") {
			return PathBuf::from(format!("/System/Volumes/Data{}", path_str));
		}
		if path_str.starts_with("/tmp/") {
			return PathBuf::from(format!("/System/Volumes/Data{}", path_str));
		}
		if path_str.starts_with("/var/") && !path_str.starts_with("/var/db/") {
			return PathBuf::from(format!("/System/Volumes/Data{}", path_str));
		}
		// /private contains etc, tmp, var subdirectories on the Data volume
		// When /tmp symlink canonicalizes, it becomes /private/tmp
		if path_str.starts_with("/private/") {
			return PathBuf::from(format!("/System/Volumes/Data{}", path_str));
		}

		canonical_path
	}

	/// Get APFS container information for a path
	async fn get_container_for_path(&self, path: &std::path::Path) -> VolumeResult<ApfsContainer> {
		// Get all containers and find the one containing this path
		let containers = detect_containers().await?;

		for container in containers {
			for volume in &container.volumes {
				if let Some(mount_point) = &volume.mount_point {
					if path.starts_with(mount_point) {
						return Ok(container);
					}
				}
			}
		}

		Err(VolumeError::platform(
			"Path not found in any APFS container".to_string(),
		))
	}
}

#[async_trait::async_trait]
impl super::FilesystemHandler for ApfsHandler {
	async fn enhance_volume(&self, volume: &mut Volume) -> VolumeResult<()> {
		// APFS volumes should already have container info from detection
		// Could add additional APFS-specific metadata here if needed
		Ok(())
	}

	async fn same_physical_storage(
		&self,
		path1: &std::path::Path,
		path2: &std::path::Path,
	) -> bool {
		self.same_physical_storage(path1, path2).await
	}

	fn get_copy_strategy(&self) -> Box<dyn crate::ops::files::copy::strategy::CopyStrategy> {
		// Use fast copy strategy for APFS (leverages copy-on-write)
		Box::new(crate::ops::files::copy::strategy::FastCopyStrategy)
	}

	fn contains_path(&self, volume: &crate::volume::types::Volume, path: &std::path::Path) -> bool {
		// Check primary mount point
		if path.starts_with(&volume.mount_point) {
			return true;
		}

		// Check additional mount points
		if volume.mount_points.iter().any(|mp| path.starts_with(mp)) {
			return true;
		}

		// APFS-specific: Check path mappings (firmlinks)
		for mapping in &volume.path_mappings {
			if path.starts_with(&mapping.virtual_path) {
				// Convert virtual path to actual path and check if it's on this volume
				if let Ok(relative_path) = path.strip_prefix(&mapping.virtual_path) {
					let actual_path = mapping.actual_path.join(relative_path);
					if actual_path.starts_with(&volume.mount_point) {
						return true;
					}
				}
			}
		}

		false
	}
}

/// Generate macOS path mappings for firmlinks
pub fn generate_macos_path_mappings() -> Vec<PathMapping> {
	vec![
		PathMapping {
			virtual_path: PathBuf::from("/Users"),
			actual_path: PathBuf::from("/System/Volumes/Data/Users"),
		},
		PathMapping {
			virtual_path: PathBuf::from("/Applications"),
			actual_path: PathBuf::from("/System/Volumes/Data/Applications"),
		},
		PathMapping {
			virtual_path: PathBuf::from("/Library"),
			actual_path: PathBuf::from("/System/Volumes/Data/Library"),
		},
		PathMapping {
			virtual_path: PathBuf::from("/tmp"),
			actual_path: PathBuf::from("/System/Volumes/Data/tmp"),
		},
		PathMapping {
			virtual_path: PathBuf::from("/var"),
			actual_path: PathBuf::from("/System/Volumes/Data/var"),
		},
		// /private is a firmlink that contains etc, tmp, var
		// When /tmp canonicalizes, it becomes /private/tmp
		PathMapping {
			virtual_path: PathBuf::from("/private"),
			actual_path: PathBuf::from("/System/Volumes/Data/private"),
		},
	]
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_bytes_from_line() {
		assert_eq!(
			extract_bytes_from_line("Size (Capacity Ceiling): 994662584320 B (994.7 GB)"),
			Some(994662584320)
		);
		assert_eq!(
			extract_bytes_from_line("Capacity Consumed: 821093748736 B (821.1 GB)"),
			Some(821093748736)
		);
		assert_eq!(extract_bytes_from_line("No bytes here"), None);
	}

	#[test]
	fn test_extract_role_from_line() {
		assert_eq!(
			extract_role_from_line("APFS Volume Disk (Role): disk3s5 (Data)"),
			Some("Data".to_string())
		);
		assert_eq!(
			extract_role_from_line("APFS Volume Disk (Role): disk3s1 (System)"),
			Some("System".to_string())
		);
	}

	#[test]
	fn test_extract_name_from_line() {
		assert_eq!(
			extract_name_from_line("Name: Data (Case-insensitive)"),
			Some("Data".to_string())
		);
		assert_eq!(
			extract_name_from_line("Name: Macintosh HD (Case-insensitive)"),
			Some("Macintosh HD".to_string())
		);
	}

	#[test]
	fn test_extract_mount_point_from_line() {
		assert_eq!(
			extract_mount_point_from_line("Mount Point: /System/Volumes/Data"),
			Some("/System/Volumes/Data".to_string())
		);
		assert_eq!(
			extract_mount_point_from_line("Mount Point: Not Mounted"),
			None
		);
	}

	#[test]
	fn test_parse_apfs_role() {
		assert_eq!(parse_apfs_role("System"), ApfsVolumeRole::System);
		assert_eq!(parse_apfs_role("Data"), ApfsVolumeRole::Data);
		assert_eq!(parse_apfs_role("Preboot"), ApfsVolumeRole::Preboot);
		assert_eq!(parse_apfs_role("Recovery"), ApfsVolumeRole::Recovery);
		assert_eq!(parse_apfs_role("VM"), ApfsVolumeRole::VM);
		assert_eq!(
			parse_apfs_role("Custom"),
			ApfsVolumeRole::Other("custom".to_string())
		);
	}
}
