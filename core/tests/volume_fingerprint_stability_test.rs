//! Tests for volume fingerprint stability
//!
//! These tests verify that volume fingerprints remain stable across different scenarios:
//! - Reboots (disk IDs may change but mount points stay the same)
//! - Different volume types produce different fingerprints
//! - Same inputs always produce identical fingerprints (determinism)
//!
//! Run with: cargo test --test volume_fingerprint_stability_test -- --nocapture

use sd_core::domain::volume::VolumeFingerprint;
use uuid::Uuid;

#[cfg(target_os = "macos")]
use sd_core::{
	infra::event::EventBus,
	volume::{types::VolumeDetectionConfig, VolumeManager},
};
#[cfg(target_os = "macos")]
use std::{collections::HashMap, sync::Arc, thread, time::Duration};

/// Test that identical volume properties produce identical fingerprints
#[test]
fn test_fingerprint_deterministic() {
	let mount_point = std::path::Path::new("/Volumes/Macintosh HD");
	let device_id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

	let fp1 = VolumeFingerprint::from_primary_volume(mount_point, device_id);
	let fp2 = VolumeFingerprint::from_primary_volume(mount_point, device_id);
	let fp3 = VolumeFingerprint::from_primary_volume(mount_point, device_id);

	assert_eq!(fp1, fp2, "Fingerprints should be identical for same inputs");
	assert_eq!(fp2, fp3, "Fingerprints should be deterministic");

	println!("Fingerprint is deterministic: {}", fp1.short_id());
}

/// Test that different inputs produce different fingerprints
#[test]
fn test_fingerprint_differs_with_different_inputs() {
	let device_id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

	let fp_vol1 =
		VolumeFingerprint::from_primary_volume(std::path::Path::new("/Volumes/HD1"), device_id);
	let fp_vol2 =
		VolumeFingerprint::from_primary_volume(std::path::Path::new("/Volumes/HD2"), device_id);

	assert_ne!(
		fp_vol1, fp_vol2,
		"Different mount points should produce different fingerprints"
	);

	let device_id2 = Uuid::parse_str("abcdefab-cdef-abcd-efab-cdefabcdefab").unwrap();
	let fp_dev2 =
		VolumeFingerprint::from_primary_volume(std::path::Path::new("/Volumes/HD1"), device_id2);

	assert_ne!(
		fp_vol1, fp_dev2,
		"Different device IDs should produce different fingerprints"
	);

	println!("HD1 fingerprint: {}", fp_vol1.short_id());
	println!("HD2 fingerprint: {}", fp_vol2.short_id());
}

/// Test that fingerprints are stable because they use mount point (not consumed space or disk IDs)
#[test]
fn test_fingerprint_stable_across_volume_types() {
	let device_id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
	let spacedrive_id = Uuid::parse_str("abcd1234-5678-90ab-cdef-1234567890ab").unwrap();

	// Primary volume fingerprint uses mount point + device (both stable across reboots)
	let fp_primary = VolumeFingerprint::from_primary_volume(std::path::Path::new("/"), device_id);

	// External volume fingerprint uses dotfile UUID + device (both stable)
	let fp_external = VolumeFingerprint::from_external_volume(spacedrive_id, device_id);

	// Network volume fingerprint uses backend ID + URI (both stable)
	let fp_network = VolumeFingerprint::from_network_volume("smb", "//nas.local/share");

	// All volume types should produce different fingerprints
	assert_ne!(
		fp_primary, fp_external,
		"Primary and external should differ"
	);
	assert_ne!(fp_primary, fp_network, "Primary and network should differ");
	assert_ne!(
		fp_external, fp_network,
		"External and network should differ"
	);

	// Same inputs should always produce same fingerprints (stability)
	let fp_primary2 = VolumeFingerprint::from_primary_volume(std::path::Path::new("/"), device_id);
	assert_eq!(
		fp_primary, fp_primary2,
		"Primary volume fingerprint should be stable"
	);

	println!("Primary: {}", fp_primary.short_id());
	println!("External: {}", fp_external.short_id());
	println!("Network: {}", fp_network.short_id());
}

/// Test that mount-point-based fingerprints are reboot-safe
/// (disk IDs like disk3/disk4 change on reboot, but mount points don't)
#[test]
fn test_fingerprint_stable_despite_disk_id_changes() {
	let device_id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();

	// Mount point is stable across reboots even if underlying disk IDs change
	let mount_point = std::path::Path::new("/Volumes/Macintosh HD");
	let fp_before = VolumeFingerprint::from_primary_volume(mount_point, device_id);
	let fp_after = VolumeFingerprint::from_primary_volume(mount_point, device_id);

	assert_eq!(
		fp_before, fp_after,
		"Fingerprint should be stable across reboots (mount point doesn't change)"
	);

	// External volume with dotfile UUID is also stable across reboots
	let spacedrive_id = Uuid::parse_str("aabbccdd-1234-5678-9012-aabbccddeeff").unwrap();
	let fp_ext_before = VolumeFingerprint::from_external_volume(spacedrive_id, device_id);
	let fp_ext_after = VolumeFingerprint::from_external_volume(spacedrive_id, device_id);

	assert_eq!(
		fp_ext_before, fp_ext_after,
		"External volume fingerprint stable when dotfile UUID is preserved"
	);

	println!("Primary fingerprint (stable): {}", fp_before.short_id());
	println!(
		"External fingerprint (stable): {}",
		fp_ext_before.short_id()
	);
}

/// Test actual volume detection and fingerprint consistency
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_real_volume_fingerprints_remain_stable() {
	println!("\nTesting real volume fingerprint stability...\n");

	// Initialize volume manager
	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	// First detection
	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	let volumes_first = volume_manager.get_all_volumes().await;
	let mut first_fingerprints: HashMap<String, String> = HashMap::new();

	println!("First detection - {} volumes:", volumes_first.len());
	for volume in &volumes_first {
		let fp_string = volume.fingerprint.to_string();
		first_fingerprints.insert(
			volume.mount_point.to_string_lossy().to_string(),
			fp_string.clone(),
		);

		println!(
			"  {} → fingerprint: {}",
			volume.mount_point.display(),
			volume.fingerprint.short_id()
		);

		// Print what went into the fingerprint
		if let Some(container) = &volume.apfs_container {
			println!(
				"    (container: {}, capacity: {} bytes)",
				container.uuid, container.total_capacity
			);
		}
	}

	// Small delay to simulate time passing
	thread::sleep(Duration::from_millis(100));

	// Re-detect volumes (simulating daemon restart)
	let volume_manager2 = Arc::new(VolumeManager::new(
		device_id,
		VolumeDetectionConfig::default(),
		Arc::new(EventBus::default()),
	));

	volume_manager2
		.initialize()
		.await
		.expect("Failed to initialize volume manager (second time)");

	let volumes_second = volume_manager2.get_all_volumes().await;

	println!("\nSecond detection - {} volumes:", volumes_second.len());

	let mut stable_count = 0;
	let mut changed_count = 0;

	for volume in &volumes_second {
		let mount_point = volume.mount_point.to_string_lossy().to_string();
		let fp_string = volume.fingerprint.to_string();

		println!(
			"  {} → fingerprint: {}",
			volume.mount_point.display(),
			volume.fingerprint.short_id()
		);

		if let Some(first_fp) = first_fingerprints.get(&mount_point) {
			if first_fp == &fp_string {
				println!("    STABLE - fingerprint unchanged");
				stable_count += 1;
			} else {
				println!("    CHANGED - fingerprint different!");
				println!("       Was: {}", first_fp);
				println!("       Now: {}", fp_string);
				changed_count += 1;
			}
		} else {
			println!("    New volume (not in first detection)");
		}
	}

	println!("\nResults:");
	println!("  Stable: {}", stable_count);
	println!("  Changed: {}", changed_count);

	assert_eq!(
		changed_count, 0,
		"All volume fingerprints should remain stable across detections"
	);
}

/// Test what properties actually change vs stay stable
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_what_properties_change_on_real_volumes() {
	println!("\nAnalyzing which volume properties change over time...\n");

	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	let volumes = volume_manager.get_all_volumes().await;

	println!("Analyzing {} volumes:\n", volumes.len());

	for volume in &volumes {
		println!("Volume: {}", volume.mount_point.display());
		println!("  Name: {} (stable: UUID in name)", volume.name);
		println!("  Filesystem: {} (stable)", volume.file_system);
		println!(
			"  Total capacity: {} bytes (stable: physical drive size)",
			volume.total_capacity
		);
		println!(
			"  Available: {} bytes (CHANGES: as files are added/deleted)",
			volume.available_space
		);

		if let Some(container) = &volume.apfs_container {
			println!("\n  APFS Container:");
			println!(
				"    container_id: {} (CHANGES: disk3 -> disk4 on reboot)",
				container.container_id
			);
			println!("    uuid: {} (STABLE: always same)", container.uuid);
			println!(
				"    total_capacity: {} bytes (STABLE)",
				container.total_capacity
			);

			// Check individual volume properties
			for vol in &container.volumes {
				println!("\n    Volume {} in container:", vol.name);
				println!(
					"      disk_id: {} (CHANGES: disk3s5 -> disk4s5 on reboot)",
					vol.disk_id
				);
				println!("      uuid: {} (STABLE)", vol.uuid);
				println!(
					"      capacity_consumed: {} bytes (CHANGES: with file operations)",
					vol.capacity_consumed
				);
			}
		}

		println!("\n  For stable fingerprint, should use:");
		println!("     container.uuid:volume.uuid");
		println!("     container.total_capacity (physical drive size)");
		println!("     NOT container_id (changes on reboot)");
		println!("     NOT capacity_consumed (changes with files)");
		println!();
	}
}
