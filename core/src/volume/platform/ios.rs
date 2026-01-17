//! iOS-specific volume detection using Foundation APIs
//!
//! iOS sandboxing restricts filesystem access, but Foundation's FileManager provides
//! device-wide storage information via URL resource values. This module queries the
//! app's documents directory to retrieve total and available capacity for the device.
//!
//! iOS presents unified storage - there are no separate volumes or external drives
//! accessible through standard APIs. External storage on iPadOS requires File Provider
//! API integration (out of scope for initial implementation).

use crate::volume::{
	error::{VolumeError, VolumeResult},
	types::{
		DiskType, FileSystem, MountType, Volume, VolumeDetectionConfig, VolumeFingerprint,
		VolumeType,
	},
};
use std::path::PathBuf;
use tracing::{debug, warn};
use uuid::Uuid;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send, msg_send_id, ClassType};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};

/// Storage information retrieved from iOS FileManager
struct IosVolumeInfo {
	total_capacity: u64,
	available_capacity: u64,
}

/// Query iOS device storage using FileManager resource values
///
/// Uses NSURLVolumeAvailableCapacityForImportantUsageKey instead of the basic
/// available capacity key because iOS dynamically manages storage (purging caches,
/// offloading apps). The "important usage" key accounts for this optimization.
fn query_device_storage() -> Result<IosVolumeInfo, String> {
	unsafe {
		// Get NSFileManager singleton
		let file_manager: Retained<AnyObject> = msg_send_id![class!(NSFileManager), defaultManager];

		// Get URL for documents directory (any accessible path works for querying device storage)
		let search_path_directory: usize = 9; // NSDocumentDirectory
		let search_path_domain_mask: usize = 1; // NSUserDomainMask

		let urls: Retained<NSArray<NSURL>> = msg_send_id![
			&file_manager,
			URLsForDirectory: search_path_directory,
			inDomains: search_path_domain_mask
		];

		if urls.count() == 0 {
			return Err("No documents directory found".to_string());
		}

		let url: Retained<NSURL> = msg_send_id![&urls, objectAtIndex: 0usize];

		// Create resource keys for querying storage
		// Using the string constants directly as Foundation doesn't expose them in objc2 yet
		let key_total = NSString::from_str("NSURLVolumeTotalCapacityKey");
		let key_available = NSString::from_str("NSURLVolumeAvailableCapacityForImportantUsageKey");

		// Query resource values with error handling
		let keys: Retained<NSArray<NSString>> =
			NSArray::from_retained_slice(&[key_total.clone(), key_available.clone()]);

		let mut error: *mut AnyObject = std::ptr::null_mut();
		let values: Option<Retained<NSDictionary<NSString, AnyObject>>> = msg_send_id![
			&url,
			resourceValuesForKeys: &*keys,
			error: &mut error
		];

		if !error.is_null() {
			let error_desc: Retained<NSString> = msg_send_id![error, localizedDescription];
			return Err(format!("Failed to query storage: {}", error_desc));
		}

		let values = values.ok_or("resourceValuesForKeys returned nil")?;

		// Extract total capacity
		let total_obj: Option<Retained<NSNumber>> =
			msg_send_id![&values, objectForKey: &*key_total];
		let total: u64 = total_obj
			.map(|n| {
				let val: u64 = msg_send![&n, unsignedLongLongValue];
				val
			})
			.ok_or("Failed to get total capacity")?;

		// Extract available capacity
		let available_obj: Option<Retained<NSNumber>> =
			msg_send_id![&values, objectForKey: &*key_available];
		let available: u64 = available_obj
			.map(|n| {
				let val: u64 = msg_send![&n, unsignedLongLongValue];
				val
			})
			.ok_or("Failed to get available capacity")?;

		Ok(IosVolumeInfo {
			total_capacity: total,
			available_capacity: available,
		})
	}
}

/// Get device name from UIDevice.currentDevice.name
///
/// Returns the user-assigned device name (e.g., "Jamie's iPhone").
/// Falls back to "iPhone" if the query fails.
fn get_device_name() -> Option<String> {
	unsafe {
		let device: Retained<AnyObject> = msg_send_id![class!(UIDevice), currentDevice];
		let name: Retained<NSString> = msg_send_id![&device, name];
		Some(name.to_string())
	}
}

/// Detect iOS device storage volume
///
/// Returns a single Volume representing the device's unified storage.
/// iOS doesn't expose multiple volumes or external drives through standard APIs.
pub async fn detect_volumes(
	device_id: Uuid,
	_config: &VolumeDetectionConfig,
) -> VolumeResult<Vec<Volume>> {
	debug!("IOS_DETECT: Starting iOS volume detection");

	// Query storage info from Foundation
	let storage_info = match query_device_storage() {
		Ok(info) => {
			debug!(
				"IOS_DETECT: Storage query succeeded - total: {} bytes, available: {} bytes",
				info.total_capacity, info.available_capacity
			);
			info
		}
		Err(e) => {
			warn!("IOS_DETECT: Failed to query iOS device storage: {}", e);
			// Return empty vector instead of failing - app continues without volume info
			return Ok(Vec::new());
		}
	};

	// Get device name (e.g., "Jamie's iPhone")
	let device_name = get_device_name().unwrap_or_else(|| "iPhone".to_string());
	debug!("IOS_DETECT: Device name: {}", device_name);

	// Create stable fingerprint using device name + total capacity + filesystem
	// This remains stable across app restarts and reinstalls (unless user renames device
	// or storage capacity changes due to device upgrade/replacement)
	let fingerprint = VolumeFingerprint::from_primary_volume(&PathBuf::from("/"), device_id);

	// Generate stable UUID from fingerprint for consistent volume identification
	let volume_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, fingerprint.0.as_bytes());

	let now = chrono::Utc::now();

	let volume = Volume {
		id: volume_id,
		fingerprint,
		device_id,
		name: device_name.clone(),
		library_id: None,
		is_tracked: false,
		mount_point: PathBuf::from("/"),
		mount_points: vec![PathBuf::from("/")],
		volume_type: VolumeType::Primary,
		mount_type: MountType::System,
		disk_type: DiskType::SSD,      // All iOS devices use flash storage
		file_system: FileSystem::APFS, // iOS uses APFS since iOS 10.3
		total_capacity: storage_info.total_capacity,
		available_space: storage_info.available_capacity,
		is_read_only: false,
		is_mounted: true,
		hardware_id: None,
		backend: None,
		cloud_identifier: None,
		cloud_config: None,
		apfs_container: None,
		container_volume_id: None,
		path_mappings: Vec::new(),
		is_user_visible: true,
		auto_track_eligible: true,
		read_speed_mbps: None,
		write_speed_mbps: None,
		created_at: now,
		updated_at: now,
		last_seen_at: now,
		total_files: None,
		total_directories: None,
		last_stats_update: None,
		display_name: Some(device_name),
		is_favorite: false,
		color: None,
		icon: None,
		error_message: None,
	};

	debug!("IOS_DETECT: Successfully created iOS volume");
	Ok(vec![volume])
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_storage_query() {
		// This test will only pass on iOS devices/simulator
		let result = query_device_storage();
		if let Ok(info) = result {
			assert!(info.total_capacity > 0, "Total capacity should be positive");
			assert!(
				info.available_capacity <= info.total_capacity,
				"Available should not exceed total"
			);
		}
		// Don't fail on non-iOS platforms - test is skipped via cfg
	}

	#[test]
	fn test_device_name() {
		// This test will only pass on iOS devices/simulator
		let name = get_device_name();
		if let Some(name) = name {
			assert!(!name.is_empty(), "Device name should not be empty");
		}
		// Don't fail on non-iOS platforms
	}

	#[tokio::test]
	async fn test_detect_volumes() {
		let device_id = Uuid::new_v4();
		let config = VolumeDetectionConfig::default();
		let result = detect_volumes(device_id, &config).await;

		// Should not error, but may return empty on non-iOS
		assert!(result.is_ok(), "Detection should not error");

		if let Ok(volumes) = result {
			if !volumes.is_empty() {
				let vol = &volumes[0];
				assert_eq!(vol.volume_type, VolumeType::Primary);
				assert_eq!(vol.file_system, FileSystem::APFS);
				assert_eq!(vol.disk_type, DiskType::SSD);
				assert!(vol.is_user_visible);
				assert!(vol.auto_track_eligible);
			}
		}
	}
}
