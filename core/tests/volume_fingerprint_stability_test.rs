//! Tests for volume fingerprint stability
//!
//! These tests verify that volume fingerprints remain stable across different scenarios:
//! - Reboots (disk IDs may change: disk3 → disk4)
//! - File operations (consumed space changes)
//! - Mount/unmount cycles
//!
//! Run with: cargo test --test volume_fingerprint_stability_test -- --nocapture

use sd_core::{
	domain::volume::VolumeFingerprint,
	infra::event::EventBus,
	volume::{types::VolumeDetectionConfig, VolumeManager},
};
use std::{collections::HashMap, sync::Arc, thread, time::Duration};
use uuid::Uuid;

/// Test that identical volume properties produce identical fingerprints
#[test]
fn test_fingerprint_deterministic() {
	let uuid_str = "12345678-1234-5678-1234-567812345678";
	let capacity = 1_000_000_000_000u64; // 1TB

	// Generate fingerprint multiple times with same inputs
	let fp1 = VolumeFingerprint::new(uuid_str, capacity, "APFS");
	let fp2 = VolumeFingerprint::new(uuid_str, capacity, "APFS");
	let fp3 = VolumeFingerprint::new(uuid_str, capacity, "APFS");

	assert_eq!(
		fp1, fp2,
		"Fingerprints should be identical for same inputs"
	);
	assert_eq!(fp2, fp3, "Fingerprints should be deterministic");

	println!("Fingerprint is deterministic: {}", fp1.short_id());
}

/// Test that capacity changes produce different fingerprints (expected)
#[test]
fn test_fingerprint_changes_with_capacity() {
	let uuid_str = "12345678-1234-5678-1234-567812345678";

	let fp_1tb = VolumeFingerprint::new(uuid_str, 1_000_000_000_000, "APFS");
	let fp_2tb = VolumeFingerprint::new(uuid_str, 2_000_000_000_000, "APFS");

	assert_ne!(
		fp_1tb, fp_2tb,
		"Different capacities should produce different fingerprints"
	);

	println!("1TB fingerprint: {}", fp_1tb.short_id());
	println!("2TB fingerprint: {}", fp_2tb.short_id());
}

/// Test that consumed space changes DON'T affect fingerprint (if we use total capacity)
#[test]
fn test_fingerprint_stable_despite_consumed_changes() {
	// Simulate same volume with different consumed space
	let container_uuid = "ABCD1234-5678-90AB-CDEF-1234567890AB";
	let volume_uuid = "VOLUME12-3456-7890-ABCD-EF1234567890";
	let container_total = 1_000_000_000_000u64; // 1TB total (stable)

	let identifier = format!("{}:{}", container_uuid, volume_uuid);

	// Fingerprint should use TOTAL capacity (stable)
	let fp1 = VolumeFingerprint::new(&identifier, container_total, "APFS");
	let fp2 = VolumeFingerprint::new(&identifier, container_total, "APFS");

	assert_eq!(
		fp1, fp2,
		"Fingerprints should be identical when using stable total capacity"
	);

	println!("Fingerprint stable with total capacity: {}", fp1.short_id());

	// But if someone mistakenly uses consumed capacity, it would change
	let consumed_50gb = 50_000_000_000u64;
	let consumed_100gb = 100_000_000_000u64;

	let fp_consumed_50 = VolumeFingerprint::new(&identifier, consumed_50gb, "APFS");
	let fp_consumed_100 = VolumeFingerprint::new(&identifier, consumed_100gb, "APFS");

	assert_ne!(
		fp_consumed_50, fp_consumed_100,
		"Using consumed capacity WOULD create different fingerprints (BAD!)"
	);

	println!(
		"️  WARNING: If consumed capacity used, fingerprint would change: {} vs {}",
		fp_consumed_50.short_id(),
		fp_consumed_100.short_id()
	);
}

/// Test that UUID-based identifiers are stable even if disk IDs change
#[test]
fn test_fingerprint_stable_despite_disk_id_changes() {
	let container_uuid = "ABCD1234-5678-90AB-CDEF-1234567890AB";
	let volume_uuid = "VOLUME12-3456-7890-ABCD-EF1234567890";
	let capacity = 1_000_000_000_000u64;

	// UUID-based identifier (stable across reboots)
	let uuid_identifier = format!("{}:{}", container_uuid, volume_uuid);
	let fp_uuid = VolumeFingerprint::new(&uuid_identifier, capacity, "APFS");

	// Disk ID-based identifier (changes on reboot)
	let disk_id_before_reboot = "disk3:disk3s5";
	let disk_id_after_reboot = "disk4:disk4s5"; // Same volume, different disk number

	let fp_disk3 = VolumeFingerprint::new(disk_id_before_reboot, capacity, "APFS");
	let fp_disk4 = VolumeFingerprint::new(disk_id_after_reboot, capacity, "APFS");

	// Disk ID-based fingerprints would be different (BAD)
	assert_ne!(
		fp_disk3, fp_disk4,
		"Disk ID-based fingerprints change on reboot (demonstrating the bug)"
	);

	println!("UUID-based fingerprint (stable): {}", fp_uuid.short_id());
	println!(
		"️  disk3-based fingerprint: {} (would change to {} on reboot)",
		fp_disk3.short_id(),
		fp_disk4.short_id()
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
		first_fingerprints.insert(volume.mount_point.to_string_lossy().to_string(), fp_string.clone());

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
			println!("    ️  New volume (not in first detection)");
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
				"    container_id: {} (CHANGES: disk3 → disk4 on reboot)",
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
					"      disk_id: {} (CHANGES: disk3s5 → disk4s5 on reboot)",
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

