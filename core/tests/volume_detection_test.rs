//! Integration tests for volume detection and filesystem-aware copy strategy selection
//!
//! These tests verify that the volume detection system correctly identifies volumes,
//! resolves paths to their storage locations, and selects optimal copy strategies.

use sd_core::{
	device::get_current_device_slug,
	domain::addressing::SdPath,
	infra::event::EventBus,
	ops::files::copy::{input::CopyMethod, routing::CopyStrategyRouter},
	volume::{
		types::{VolumeDetectionConfig, VolumeType},
		VolumeManager,
	},
};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Test volume detection on macOS
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_macos_volume_detection() {
	// Initialize volume manager
	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	// Initialize and detect volumes
	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	// Get all detected volumes
	let volumes = volume_manager.get_all_volumes().await;
	println!("Detected {} volumes:", volumes.len());

	for volume in &volumes {
		println!(
			"  - {} ({}) at {} [{}] - {}",
			volume.name,
			volume.file_system,
			volume.mount_point.display(),
			volume.volume_type.display_name(),
			if volume.apfs_container.is_some() {
				"APFS Container"
			} else {
				"Standalone"
			}
		);

		// Print APFS container info if available
		if let Some(container) = &volume.apfs_container {
			println!(
				"    Container: {} ({} volumes)",
				container.container_id,
				container.volumes.len()
			);
		}

		// Print path mappings if available
		if !volume.path_mappings.is_empty() {
			println!("    Path mappings:");
			for mapping in &volume.path_mappings {
				println!(
					"      {} -> {}",
					mapping.virtual_path.display(),
					mapping.actual_path.display()
				);
			}
		}
	}

	// Verify we have at least one volume
	assert!(!volumes.is_empty(), "No volumes detected");

	// On macOS, we should have APFS volumes
	let apfs_volumes: Vec<_> = volumes
		.iter()
		.filter(|v| matches!(v.file_system, sd_core::volume::types::FileSystem::APFS))
		.collect();

	assert!(
		!apfs_volumes.is_empty(),
		"No APFS volumes detected on macOS"
	);
	println!("Found {} APFS volumes", apfs_volumes.len());

	// Check for Data volume (should be Primary type) with path mappings
	let data_volumes: Vec<_> = apfs_volumes
		.iter()
		.filter(|v| matches!(v.volume_type, VolumeType::Primary) && !v.path_mappings.is_empty())
		.collect();

	if !data_volumes.is_empty() {
		println!(
			"Found {} APFS Data volumes (Primary type) with path mappings",
			data_volumes.len()
		);
	}

	// Test that user paths resolve to the correct volume (Data, not Macintosh HD at /)
	println!("\nTesting user path resolution:");
	let user_paths = vec![
		("/Users", "Data"),
		("/Applications", "Data"),
		("/Library", "Data"),
		("/tmp", "Data"),
		("/var", "Data"),
	];

	for (path_str, expected_volume) in user_paths {
		let path = std::path::PathBuf::from(path_str);
		if path.exists() {
			if let Some(volume) = volume_manager.volume_for_path(&path).await {
				println!(
					"  {} -> {} ({})",
					path_str,
					volume.name,
					if volume.name == expected_volume {
						"✓"
					} else {
						"✗ WRONG"
					}
				);
				assert_eq!(
					volume.name, expected_volume,
					"Path {} should resolve to '{}' volume, not '{}'",
					path_str, expected_volume, volume.name
				);
			} else {
				panic!("Path {} should resolve to a volume", path_str);
			}
		}
	}
}

/// Test path resolution for common macOS paths
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_macos_path_resolution() {
	// Initialize volume manager
	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	// Initialize and detect volumes
	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	// Test common macOS paths
	let test_paths = vec![
		"/Users",
		"/Applications",
		"/Library",
		"/tmp",
		"/var",
		"/System/Volumes/Data/Users",
		"/System/Volumes/Data/Applications",
	];

	println!("Testing path resolution:");
	for path_str in test_paths {
		let path = PathBuf::from(path_str);
		if path.exists() {
			if let Some(volume) = volume_manager.volume_for_path(&path).await {
				println!(
					"  {} -> Volume: {} ({})",
					path_str, volume.name, volume.file_system
				);
			} else {
				println!("  {} -> No volume found", path_str);
			}
		} else {
			println!("  {} -> Path does not exist", path_str);
		}
	}
}

/// Test same physical storage detection for common paths
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_same_physical_storage_detection() {
	// Initialize volume manager
	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	// Initialize and detect volumes
	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	// Test same-storage detection for common macOS paths
	let test_cases = vec![
		(
			"/Users",
			"/Applications",
			true,
			"Both should be on Data volume",
		),
		("/Users", "/tmp", true, "Both should be on Data volume"),
		(
			"/Users",
			"/System/Volumes/Data/Users",
			true,
			"Virtual vs actual path",
		),
		(
			"/Applications",
			"/System/Volumes/Data/Applications",
			true,
			"Virtual vs actual path",
		),
		("/Users", "/Volumes", false, "Different storage locations"),
	];

	println!("Testing same physical storage detection:");
	for (path1_str, path2_str, expected, description) in test_cases {
		let path1 = PathBuf::from(path1_str);
		let path2 = PathBuf::from(path2_str);

		// Only test if both paths exist
		if path1.exists() && path2.exists() {
			let same_storage = volume_manager.same_physical_storage(&path1, &path2).await;
			let status = if same_storage == expected { "" } else { "" };

			println!(
				"  {} {} <-> {} = {} (expected: {}) - {}",
				status, path1_str, path2_str, same_storage, expected, description
			);

			// For critical paths, assert the result
			if path1_str == "/Users" && path2_str == "/Applications" {
				assert_eq!(
					same_storage, expected,
					"Users and Applications should be detected as same storage on macOS APFS"
				);
			}
		} else {
			println!(
				"   {} <-> {} - Skipped (paths don't exist)",
				path1_str, path2_str
			);
		}
	}
}

/// Test copy strategy selection for same-storage vs cross-storage operations
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_copy_strategy_selection() {
	// Initialize volume manager
	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	// Initialize and detect volumes
	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	// Create test paths (using existing directories)
	let test_cases = vec![
		(
			"/Users",
			"/Applications",
			"Same APFS container - should use FastCopyStrategy",
		),
		(
			"/Users",
			"/tmp",
			"Same APFS container - should use FastCopyStrategy",
		),
	];

	println!("Testing copy strategy selection:");
	for (source_str, dest_str, expected_behavior) in test_cases {
		let source_path = PathBuf::from(source_str);
		let dest_path = PathBuf::from(dest_str);

		// Only test if both paths exist
		if source_path.exists() && dest_path.exists() {
			// Create SdPath instances (using current device slug)
			let device_slug = get_current_device_slug();
			let source_sdpath = SdPath::new(device_slug.clone(), source_path.clone());
			let dest_sdpath = SdPath::new(device_slug, dest_path.clone());

			// Test strategy selection
			let strategy = CopyStrategyRouter::select_strategy(
				&source_sdpath,
				&dest_sdpath,
				false, // is_move = false
				&CopyMethod::Auto,
				Some(&*volume_manager),
			)
			.await;

			// Get strategy description
			let description = CopyStrategyRouter::describe_strategy(
				&source_sdpath,
				&dest_sdpath,
				false,
				&CopyMethod::Auto,
				Some(&*volume_manager),
			)
			.await;

			println!(
				"  {} -> {} = {} ({})",
				source_str, dest_str, description, expected_behavior
			);

			// For same-storage operations, we should get a fast copy strategy
			if source_str == "/Users" && dest_str == "/Applications" {
				assert!(
					description.contains("Fast copy") || description.contains("APFS clone"),
					"Same-storage copy should use fast copy strategy, got: {}",
					description
				);
			}
		} else {
			println!(
				"   {} -> {} - Skipped (paths don't exist)",
				source_str, dest_str
			);
		}
	}
}

/// Test APFS container detection specifically
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_apfs_container_detection() {
	use sd_core::volume::fs::apfs;

	println!("Testing APFS container detection:");

	// Detect APFS containers directly
	let containers = apfs::detect_containers()
		.await
		.expect("Failed to detect APFS containers");

	println!("Detected {} APFS containers:", containers.len());
	for container in &containers {
		println!(
			"  Container: {} ({})",
			container.container_id, container.uuid
		);
		println!("    Physical Store: {}", container.physical_store);
		println!(
			"    Total Capacity: {} GB",
			container.total_capacity / (1024 * 1024 * 1024)
		);
		println!("    Volumes: {}", container.volumes.len());

		for volume in &container.volumes {
			println!(
				"      - {} ({}) at {:?} [{}]",
				volume.name, volume.disk_id, volume.mount_point, volume.role
			);
		}
	}

	// Verify we have at least one container
	assert!(!containers.is_empty(), "No APFS containers detected");

	// Check for Data volume
	let has_data_volume = containers.iter().any(|container| {
		container
			.volumes
			.iter()
			.any(|volume| matches!(volume.role, sd_core::volume::types::ApfsVolumeRole::Data))
	});

	assert!(has_data_volume, "No APFS Data volume found");
	println!("Found APFS Data volume");
}

/// Test filesystem handler selection
#[tokio::test]
async fn test_filesystem_handler_selection() {
	use sd_core::volume::{fs, types::FileSystem};

	println!("Testing filesystem handler selection:");

	let filesystems = vec![
		FileSystem::APFS,
		FileSystem::NTFS,
		FileSystem::ExFAT,
		FileSystem::FAT32,
		FileSystem::Other("Unknown".to_string()),
	];

	for fs_type in filesystems {
		let handler = fs::get_filesystem_handler(&fs_type);
		let strategy = handler.get_copy_strategy();

		println!(
			"  {} -> Strategy type: {}",
			fs_type,
			std::any::type_name_of_val(&*strategy)
				.split("::")
				.last()
				.unwrap_or("Unknown")
		);
	}
}

/// Integration test that simulates the full copy workflow
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_full_copy_workflow_simulation() {
	// Initialize volume manager
	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig::default();
	let events = Arc::new(EventBus::default());
	let volume_manager = Arc::new(VolumeManager::new(device_id, config, events));

	// Initialize and detect volumes
	volume_manager
		.initialize()
		.await
		.expect("Failed to initialize volume manager");

	// Simulate common copy scenarios
	let scenarios = vec![
		("/Users/Shared", "/Applications", "Desktop to Applications"),
		("/tmp", "/Users/Shared", "Temp to Desktop"),
	];

	println!("Testing full copy workflow simulation:");
	for (source_str, dest_str, scenario_name) in scenarios {
		let source_path = PathBuf::from(source_str);
		let dest_path = PathBuf::from(dest_str);

		// Only test if both paths exist
		if source_path.exists() && dest_path.exists() {
			println!("\nScenario: {}", scenario_name);

			// Step 1: Check if paths are on same physical storage
			let same_storage = volume_manager
				.same_physical_storage(&source_path, &dest_path)
				.await;
			println!("  Same physical storage: {}", same_storage);

			// Step 2: Get volumes for both paths
			let source_volume = volume_manager.volume_for_path(&source_path).await;
			let dest_volume = volume_manager.volume_for_path(&dest_path).await;

			match (&source_volume, &dest_volume) {
				(Some(src_vol), Some(dst_vol)) => {
					println!(
						"  Source volume: {} ({})",
						src_vol.name, src_vol.file_system
					);
					println!("  Dest volume: {} ({})", dst_vol.name, dst_vol.file_system);

					// Step 3: Select copy strategy
					let device_slug = get_current_device_slug();
					let source_sdpath = SdPath::new(device_slug.clone(), source_path.clone());
					let dest_sdpath = SdPath::new(device_slug, dest_path.clone());

					let description = CopyStrategyRouter::describe_strategy(
						&source_sdpath,
						&dest_sdpath,
						false,
						&CopyMethod::Auto,
						Some(&*volume_manager),
					)
					.await;

					println!("  Selected strategy: {}", description);

					// Step 4: Verify expected behavior
					if same_storage
						&& matches!(
							src_vol.file_system,
							sd_core::volume::types::FileSystem::APFS
						) {
						assert!(
							description.contains("Fast copy") || description.contains("APFS clone"),
							"Same-storage APFS copy should use fast strategy, got: {}",
							description
						);
						println!("  Correctly selected fast copy for same-storage APFS operation");
					}
				}
				_ => {
					println!("  Could not find volumes for one or both paths");
				}
			}
		} else {
			println!(
				"   Scenario '{}' skipped (paths don't exist)",
				scenario_name
			);
		}
	}
}
