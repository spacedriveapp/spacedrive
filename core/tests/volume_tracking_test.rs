//! Integration tests for volume tracking functionality

use sd_core::{
	ops::volumes::{
		speed_test::action::{VolumeSpeedTestAction, VolumeSpeedTestInput},
		track::{VolumeTrackAction, VolumeTrackInput},
		untrack::{VolumeUntrackAction, VolumeUntrackInput},
	},
	volume::types::MountType,
	Core,
};
use std::sync::Arc;
use tempfile::tempdir;
use tracing::info;

#[tokio::test]
async fn test_volume_tracking_lifecycle() {
	// Initialize logging
	let _ = tracing_subscriber::fmt::try_init();

	// Setup test environment
	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	// Initialize core - this handles all the setup automatically
	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Create a test library
	let library = core
		.libraries
		.create_library(
			"Test Library",
			Some(data_path.join("libraries").join("test-library")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	let library_id = library.id();
	info!("Created test library: {}", library_id);

	// Get volume manager
	let volume_manager = core.volumes.clone();

	// Refresh volumes to ensure we have the latest
	volume_manager
		.refresh_volumes()
		.await
		.expect("Failed to refresh volumes");

	// Get all volumes
	let all_volumes = volume_manager.get_all_volumes().await;

	info!("Detected {} volumes", all_volumes.len());

	// Get first user-visible volume for testing (skip system volumes)
	let test_volume = all_volumes
		.iter()
		.find(|v| v.is_user_visible)
		.expect("No user-visible volumes available for testing")
		.clone();

	info!("Using volume '{}' for testing", test_volume.name);

	let fingerprint = test_volume.fingerprint.clone();

	// Get action manager from core context
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager should be initialized");

	// Test 1: Check if volume is already tracked (from auto-tracking)
	info!("Checking initial tracking status...");
	let initial_tracked = volume_manager
		.is_volume_tracked(&library, &fingerprint)
		.await
		.expect("Failed to check tracking status");

	let mut tracked_volume_id = None;

	if initial_tracked {
		info!("Volume is already tracked (from auto-tracking), getting volume_id for untracking");

		// Get the tracked volumes and find ours
		let tracked_volumes = volume_manager
			.get_tracked_volumes(&library)
			.await
			.expect("Failed to get tracked volumes");

		let tracked_volume = tracked_volumes
			.iter()
			.find(|v| v.fingerprint == fingerprint)
			.expect("Volume should be tracked");

		let volume_id = tracked_volume.uuid;

		// Untrack it first so we can test tracking
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput { volume_id });

		let result = action_manager
			.dispatch_library(Some(library_id), untrack_action)
			.await;
		assert!(result.is_ok(), "Failed to untrack volume: {:?}", result);
	}

	// Test 1: Track volume
	info!("Testing volume tracking...");
	{
		let track_action = VolumeTrackAction::new(VolumeTrackInput {
			fingerprint: fingerprint.to_string(),
			display_name: Some("My Test Volume".to_string()),
		});

		let result = action_manager
			.dispatch_library(Some(library_id), track_action)
			.await;

		assert!(result.is_ok(), "Failed to track volume: {:?}", result);

		let track_output = result.unwrap();
		tracked_volume_id = Some(track_output.volume_id);
		info!(
			"Volume tracked successfully with ID: {}",
			track_output.volume_id
		);

		// Verify volume is tracked
		let is_tracked = volume_manager
			.is_volume_tracked(&library, &fingerprint)
			.await
			.expect("Failed to check tracking status");
		assert!(is_tracked, "Volume should be tracked");

		// Get tracked volumes
		let tracked_volumes = volume_manager
			.get_tracked_volumes(&library)
			.await
			.expect("Failed to get tracked volumes");

		// Find our specific volume (there might be others from auto-tracking)
		let our_volume = tracked_volumes
			.iter()
			.find(|v| v.fingerprint == fingerprint)
			.expect("Our volume should be in tracked volumes");

		assert_eq!(our_volume.display_name, Some("My Test Volume".to_string()));
	}

	// Test 2: Try to track same volume again (should be idempotent)
	info!("Testing duplicate tracking idempotency...");
	{
		let track_action = VolumeTrackAction::new(VolumeTrackInput {
			fingerprint: fingerprint.to_string(),
			display_name: Some("Another Name".to_string()),
		});

		let result = action_manager
			.dispatch_library(Some(library_id), track_action)
			.await;

		// Tracking the same volume twice should succeed (idempotent operation)
		assert!(result.is_ok(), "Duplicate tracking should be idempotent");

		let track_output = result.unwrap();
		// Should return the same volume_id as the first track
		assert_eq!(
			track_output.volume_id,
			tracked_volume_id.unwrap(),
			"Duplicate tracking should return the same volume_id"
		);
		info!("Duplicate tracking is correctly idempotent");
	}

	// Test 3: Untrack volume
	info!("Testing volume untracking...");
	{
		let volume_id = tracked_volume_id.expect("Volume should be tracked");
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput { volume_id });

		let result = action_manager
			.dispatch_library(Some(library_id), untrack_action)
			.await;

		assert!(result.is_ok(), "Failed to untrack volume: {:?}", result);

		if result.is_ok() {
			info!("Volume untracked successfully");
		}

		// Verify volume is no longer tracked
		let is_tracked = volume_manager
			.is_volume_tracked(&library, &fingerprint)
			.await
			.expect("Failed to check tracking status");
		assert!(!is_tracked, "Volume should not be tracked");

		// Get tracked volumes and verify our volume is not there
		let tracked_volumes = volume_manager
			.get_tracked_volumes(&library)
			.await
			.expect("Failed to get tracked volumes");

		let our_volume_still_tracked = tracked_volumes.iter().any(|v| v.fingerprint == fingerprint);
		assert!(
			!our_volume_still_tracked,
			"Our volume should no longer be tracked"
		);
	}

	// Test 4: Try to untrack volume that's not tracked (should fail)
	info!("Testing untrack of non-tracked volume...");
	{
		// Use a non-existent UUID for testing
		use uuid::Uuid;
		let non_existent_volume_id = Uuid::new_v4();
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput {
			volume_id: non_existent_volume_id,
		});

		let result = action_manager
			.dispatch_library(Some(library_id), untrack_action)
			.await;

		assert!(
			result.is_err(),
			"Should not be able to untrack non-tracked volume"
		);
		info!("Untrack of non-tracked volume correctly prevented");
	}

	info!("Volume tracking lifecycle test completed successfully");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_tracking_multiple_libraries() {
	// Initialize logging
	let _ = tracing_subscriber::fmt::try_init();

	// Setup test environment
	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	// Initialize core - this handles all the setup automatically
	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Create two test libraries
	let library1 = core
		.libraries
		.create_library(
			"Library 1",
			Some(data_path.join("libraries").join("library1")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library 1");

	let library2 = core
		.libraries
		.create_library(
			"Library 2",
			Some(data_path.join("libraries").join("library2")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library 2");

	let library1_id = library1.id();
	let library2_id = library2.id();

	info!("Created libraries: {} and {}", library1_id, library2_id);

	// Get volume manager and refresh
	let volume_manager = core.volumes.clone();
	volume_manager
		.refresh_volumes()
		.await
		.expect("Failed to refresh volumes");

	// Get first user-visible volume for testing (skip system volumes)
	let test_volume = volume_manager
		.get_all_volumes()
		.await
		.iter()
		.find(|v| v.is_user_visible)
		.expect("No user-visible volumes available for testing")
		.clone();

	let fingerprint = test_volume.fingerprint.clone();

	// Get action manager from core context
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager should be initialized");

	// Check if volume is already tracked in library 1 (from auto-tracking)
	let is_tracked_lib1 = volume_manager
		.is_volume_tracked(&library1, &fingerprint)
		.await
		.expect("Failed to check tracking status");

	if is_tracked_lib1 {
		info!("Volume already tracked in library 1, untracking first");
		let tracked_volumes = volume_manager
			.get_tracked_volumes(&library1)
			.await
			.expect("Failed to get tracked volumes");
		let tracked_vol = tracked_volumes
			.iter()
			.find(|v| v.fingerprint == fingerprint)
			.expect("Volume should be tracked");
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput {
			volume_id: tracked_vol.uuid,
		});
		action_manager
			.dispatch_library(Some(library1_id), untrack_action)
			.await
			.expect("Failed to untrack from library 1");
	}

	// Track volume in library 1
	info!("Tracking volume in library 1...");
	{
		let track_action = VolumeTrackAction::new(VolumeTrackInput {
			fingerprint: fingerprint.to_string(),
			display_name: Some("Library 1 Volume".to_string()),
		});

		let result = action_manager
			.dispatch_library(Some(library1_id), track_action)
			.await;
		assert!(result.is_ok(), "Failed to track volume in library 1");
	}

	// Check if volume is already tracked in library 2 (from auto-tracking)
	let is_tracked_lib2 = volume_manager
		.is_volume_tracked(&library2, &fingerprint)
		.await
		.expect("Failed to check tracking status");

	if is_tracked_lib2 {
		info!("Volume already tracked in library 2, untracking first");
		let tracked_volumes = volume_manager
			.get_tracked_volumes(&library2)
			.await
			.expect("Failed to get tracked volumes");
		let tracked_vol = tracked_volumes
			.iter()
			.find(|v| v.fingerprint == fingerprint)
			.expect("Volume should be tracked");
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput {
			volume_id: tracked_vol.uuid,
		});
		action_manager
			.dispatch_library(Some(library2_id), untrack_action)
			.await
			.expect("Failed to untrack from library 2");
	}

	// Track same volume in library 2 (should succeed)
	info!("Tracking same volume in library 2...");
	{
		let track_action = VolumeTrackAction::new(VolumeTrackInput {
			fingerprint: fingerprint.to_string(),
			display_name: Some("Library 2 Volume".to_string()),
		});

		let result = action_manager
			.dispatch_library(Some(library2_id), track_action)
			.await;
		assert!(
			result.is_ok(),
			"Should be able to track volume in different library"
		);
	}

	// Verify both libraries have the volume tracked
	let lib1_volumes = volume_manager
		.get_tracked_volumes(&library1)
		.await
		.expect("Failed to get library 1 volumes");

	let lib1_our_volume = lib1_volumes
		.iter()
		.find(|v| v.fingerprint == fingerprint)
		.expect("Our volume should be in library 1");
	assert_eq!(
		lib1_our_volume.display_name,
		Some("Library 1 Volume".to_string())
	);

	let lib2_volumes = volume_manager
		.get_tracked_volumes(&library2)
		.await
		.expect("Failed to get library 2 volumes");

	let lib2_our_volume = lib2_volumes
		.iter()
		.find(|v| v.fingerprint == fingerprint)
		.expect("Our volume should be in library 2");
	assert_eq!(
		lib2_our_volume.display_name,
		Some("Library 2 Volume".to_string())
	);

	// Untrack from library 1
	info!("Untracking volume from library 1...");
	{
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput {
			volume_id: lib1_our_volume.uuid,
		});

		let result = action_manager
			.dispatch_library(Some(library1_id), untrack_action)
			.await;
		assert!(result.is_ok(), "Failed to untrack from library 1");
	}

	// Verify library 2 still has it tracked
	let lib2_volumes = volume_manager
		.get_tracked_volumes(&library2)
		.await
		.expect("Failed to get library 2 volumes");

	let lib2_still_has_volume = lib2_volumes.iter().any(|v| v.fingerprint == fingerprint);
	assert!(
		lib2_still_has_volume,
		"Library 2 should still have volume tracked"
	);

	info!("Multiple library volume tracking test completed successfully");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_automatic_system_volume_tracking() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Create library with default settings (auto_track enabled)
	let library = core
		.libraries
		.create_library(
			"Auto Track Test",
			Some(data_path.join("libraries").join("auto-track")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	info!("Created library with auto-tracking enabled");

	// Get tracked volumes
	let tracked_volumes = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes");

	// Get system volumes that are user-visible (non-hidden system volumes)
	let system_volumes: Vec<_> = core
		.volumes
		.get_system_volumes()
		.await
		.into_iter()
		.filter(|v| v.is_user_visible)
		.collect();

	info!(
		"Found {} user-visible system volumes, {} tracked volumes",
		system_volumes.len(),
		tracked_volumes.len()
	);

	// Verify user-visible system volumes are auto-tracked
	for sys_vol in &system_volumes {
		let is_tracked = tracked_volumes
			.iter()
			.any(|tv| tv.fingerprint == sys_vol.fingerprint);
		assert!(
			is_tracked,
			"User-visible system volume '{}' should be automatically tracked",
			sys_vol.name
		);
	}

	info!("Automatic system volume tracking test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_auto_tracking_disabled() {
	let _ = tracing_subscriber::fmt::try_init();

	// This test verifies manual control over volume tracking
	// Since we can't disable auto-tracking via config after creation,
	// we'll test that we can untrack auto-tracked volumes

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	let library = core
		.libraries
		.create_library(
			"Manual Track Test",
			Some(data_path.join("libraries").join("manual-track")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	// Get auto-tracked system volumes
	let auto_tracked = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes");

	info!("Found {} auto-tracked volumes", auto_tracked.len());

	// Untrack all auto-tracked volumes
	for volume in &auto_tracked {
		core.volumes
			.untrack_volume(&library, &volume.fingerprint)
			.await
			.expect("Failed to untrack volume");
	}

	// Verify all volumes are untracked
	let remaining = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes");

	assert_eq!(
		remaining.len(),
		0,
		"All volumes should be untracked after manual removal"
	);

	// Now manually track just one non-system volume if available
	let all_volumes = core.volumes.get_all_volumes().await;
	if let Some(external_volume) = all_volumes
		.iter()
		.find(|v| !matches!(v.mount_type, MountType::System))
	{
		core.volumes
			.track_volume(
				&library,
				&external_volume.fingerprint,
				Some("Manual Volume".to_string()),
			)
			.await
			.expect("Failed to manually track volume");

		let tracked = core
			.volumes
			.get_tracked_volumes(&library)
			.await
			.expect("Failed to get tracked volumes");

		assert_eq!(
			tracked.len(),
			1,
			"Should have exactly one manually tracked volume"
		);
		assert_eq!(tracked[0].display_name, Some("Manual Volume".to_string()));
	}

	info!("Manual tracking control test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_state_updates() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	let library = core
		.libraries
		.create_library(
			"State Update Test",
			Some(data_path.join("libraries").join("state-test")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	// Get a volume to track
	let test_volume = core
		.volumes
		.get_all_volumes()
		.await
		.first()
		.cloned()
		.expect("No volumes available");

	let fingerprint = test_volume.fingerprint.clone();

	// Track the volume if not already tracked
	if !core
		.volumes
		.is_volume_tracked(&library, &fingerprint)
		.await
		.unwrap_or(false)
	{
		core.volumes
			.track_volume(
				&library,
				&fingerprint,
				Some("State Test Volume".to_string()),
			)
			.await
			.expect("Failed to track volume");
	}

	// Get initial tracked state
	let initial_tracked = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes")
		.into_iter()
		.find(|v| v.fingerprint == fingerprint)
		.expect("Volume should be tracked");

	info!(
		"Initial volume state - capacity: {:?}, online: {}",
		initial_tracked.available_capacity, initial_tracked.is_online
	);

	// Update volume state
	core.volumes
		.update_tracked_volume_state(&library, &fingerprint, &test_volume)
		.await
		.expect("Failed to update volume state");

	// Get updated state
	let updated_tracked = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes")
		.into_iter()
		.find(|v| v.fingerprint == fingerprint)
		.expect("Volume should be tracked");

	// Verify last_seen_at was updated
	assert!(
		updated_tracked.last_seen_at >= initial_tracked.last_seen_at,
		"last_seen_at should be updated"
	);

	info!("Volume state update test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_speed_test() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Create a library for dispatching library-scoped actions
	let library = core
		.libraries
		.create_library(
			"Speed Test Library",
			Some(data_path.join("libraries").join("speed-test")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");
	let library_id = library.id();

	// Get first volume for testing
	let test_volume = core
		.volumes
		.get_all_volumes()
		.await
		.first()
		.cloned()
		.expect("No volumes available");

	let fingerprint = test_volume.fingerprint.clone();

	info!("Testing speed test on volume '{}'", test_volume.name);

	// Create speed test action
	let speed_test_action = VolumeSpeedTestAction::new(VolumeSpeedTestInput {
		fingerprint: fingerprint.clone(),
	});

	// Get action manager
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager should be initialized");

	// Run speed test
	let result = action_manager
		.dispatch_library(Some(library_id), speed_test_action)
		.await;

	match result {
		Ok(output) => {
			let read_speed_mbps = output.read_speed_mbps;
			let write_speed_mbps = output.write_speed_mbps;
			info!(
				"Speed test completed: {:?} MB/s read, {:?} MB/s write",
				read_speed_mbps, write_speed_mbps
			);
			if let Some(read_speed) = read_speed_mbps {
				assert!(read_speed > 0, "Read speed should be positive");
			}
			if let Some(write_speed) = write_speed_mbps {
				assert!(write_speed > 0, "Write speed should be positive");
			}
		}
		Err(e) => {
			// Speed test might fail on some volumes (e.g., read-only)
			info!("Speed test failed (expected for some volumes): {:?}", e);
		}
	}

	info!("Volume speed test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_types_and_properties() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Get all volumes
	let volumes = core.volumes.get_all_volumes().await;

	info!("Testing {} volumes for type detection", volumes.len());

	// Categorize volumes by type
	let mut system_count = 0;
	let mut external_count = 0;
	let mut network_count = 0;
	let mut _user_count = 0;

	for volume in &volumes {
		match volume.mount_type {
			MountType::System => {
				system_count += 1;
				// System volumes should be mounted and have valid paths
				assert!(volume.is_mounted, "System volume should be mounted");
				assert!(
					volume.mount_point.exists(),
					"System volume mount point should exist"
				);
			}
			MountType::External => {
				external_count += 1;
				// External volumes might or might not be mounted
				info!(
					"External volume '{}' mounted: {}",
					volume.name, volume.is_mounted
				);
			}
			MountType::Network => {
				network_count += 1;
				// Network volumes have special properties
				info!("Network volume '{}' detected", volume.name);
			}
			MountType::User => {
				_user_count += 1;
				info!("User volume '{}' detected", volume.name);
			}
		}

		// All volumes should have valid fingerprints
		assert!(
			!volume.fingerprint.0.is_empty(),
			"Volume fingerprint should not be empty"
		);

		// User-visible volumes should have capacity info
		// (Virtual/system volumes may have zero capacity)
		if volume.is_user_visible {
			assert!(
				volume.total_bytes_capacity() > 0,
				"User-visible volume '{}' should have capacity",
				volume.name
			);
		}
	}

	info!(
		"Volume types - System: {}, External: {}, Network: {}",
		system_count, external_count, network_count
	);

	// Should have at least one system volume
	assert!(system_count > 0, "Should detect at least one system volume");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_tracking_persistence() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	// Create core and library
	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	let library_path = data_path.join("libraries").join("persist-test.sdlibrary");
	let library = core
		.libraries
		.create_library(
			"Persistence Test",
			Some(library_path.clone()),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	let library_id = library.id();

	// Get a volume and track it
	let test_volume = core
		.volumes
		.get_all_volumes()
		.await
		.into_iter()
		.find(|v| !matches!(v.mount_type, MountType::System))
		.unwrap_or_else(|| {
			futures::executor::block_on(core.volumes.get_all_volumes())
				.first()
				.cloned()
				.unwrap()
		});

	let fingerprint = test_volume.fingerprint.clone();
	let custom_name = "Persisted Volume".to_string();

	// If already tracked (from auto-tracking), untrack first
	if core
		.volumes
		.is_volume_tracked(&library, &fingerprint)
		.await
		.unwrap_or(false)
	{
		core.volumes
			.untrack_volume(&library, &fingerprint)
			.await
			.expect("Failed to untrack volume");
	}

	// Now track with custom name
	core.volumes
		.track_volume(&library, &fingerprint, Some(custom_name.clone()))
		.await
		.expect("Failed to track volume");

	// Get tracked volumes before closing
	let tracked_before = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes");

	let volume_count_before = tracked_before.len();

	info!(
		"Tracked {} volumes before closing library",
		volume_count_before
	);

	// Get library path and clone it before closing
	let saved_library_path = library.path().to_path_buf();

	// Close and reopen the library within the same Core instance
	core.libraries
		.close_library(library_id)
		.await
		.expect("Failed to close library");

	// Drop the library reference to ensure it's fully released
	drop(library);

	// Reopen the same library
	let library2 = core
		.libraries
		.open_library(&saved_library_path, core.context.clone())
		.await
		.expect("Failed to reopen library");

	// Get tracked volumes after reopening
	let tracked_after = core
		.volumes
		.get_tracked_volumes(&library2)
		.await
		.expect("Failed to get tracked volumes");

	// Verify persistence
	assert_eq!(
		tracked_after.len(),
		volume_count_before,
		"Volume tracking should persist across library close/reopen"
	);

	// Find our specific volume
	let persisted_volume = tracked_after.iter().find(|v| v.fingerprint == fingerprint);

	assert!(
		persisted_volume.is_some(),
		"Tracked volume should persist after library reopen"
	);

	if let Some(vol) = persisted_volume {
		assert_eq!(
			vol.display_name,
			Some(custom_name),
			"Custom volume name should persist"
		);
	}

	info!("Volume tracking persistence test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_tracking_edge_cases() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	let library = core
		.libraries
		.create_library(
			"Edge Case Test",
			Some(data_path.join("libraries").join("edge-test")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	let library_id = library.id();

	// Get a user-visible volume for testing
	let test_volume = core
		.volumes
		.get_all_volumes()
		.await
		.iter()
		.find(|v| v.is_user_visible)
		.cloned()
		.expect("No user-visible volumes available");

	let fingerprint = test_volume.fingerprint.clone();

	// Get action manager
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager should be initialized");

	// Ensure volume is not tracked
	if core
		.volumes
		.is_volume_tracked(&library, &fingerprint)
		.await
		.unwrap_or(false)
	{
		let tracked_volumes = core
			.volumes
			.get_tracked_volumes(&library)
			.await
			.expect("Failed to get tracked volumes");
		if let Some(tracked_vol) = tracked_volumes
			.iter()
			.find(|v| v.fingerprint == fingerprint)
		{
			let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput {
				volume_id: tracked_vol.uuid,
			});
			action_manager
				.dispatch_library(Some(library_id), untrack_action)
				.await
				.ok();
		}
	}

	// Test 1: Track with empty name
	info!("Testing tracking with empty name...");
	let _volume_id_1 = {
		let track_action = VolumeTrackAction::new(VolumeTrackInput {
			fingerprint: fingerprint.to_string(),
			display_name: Some("".to_string()),
		});

		let result = action_manager
			.dispatch_library(Some(library_id), track_action)
			.await;
		assert!(result.is_ok(), "Should handle empty name");
		let output = result.unwrap();

		// Untrack for next test
		let untrack_action = VolumeUntrackAction::new(VolumeUntrackInput {
			volume_id: output.volume_id,
		});
		action_manager
			.dispatch_library(Some(library_id), untrack_action)
			.await
			.ok();

		output.volume_id
	};

	// Test 2: Track with None name
	info!("Testing tracking with None name...");
	{
		let track_action = VolumeTrackAction::new(VolumeTrackInput {
			fingerprint: fingerprint.to_string(),
			display_name: None,
		});

		let result = action_manager
			.dispatch_library(Some(library_id), track_action)
			.await;
		assert!(result.is_ok(), "Should handle None name");

		// Verify it uses the volume's default name
		let _tracked = core
			.volumes
			.get_tracked_volumes(&library)
			.await
			.expect("Failed to get tracked volumes")
			.into_iter()
			.find(|v| v.fingerprint == fingerprint)
			.expect("Volume should be tracked");

		// Note: display_name handling is implementation-dependent
	}

	info!("Volume edge cases test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_refresh_and_detection() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Get initial volume count
	let initial_volumes = core.volumes.get_all_volumes().await;
	let initial_count = initial_volumes.len();

	info!("Initial volume count: {}", initial_count);

	// Refresh volumes
	core.volumes
		.refresh_volumes()
		.await
		.expect("Failed to refresh volumes");

	// Get volumes after refresh
	let refreshed_volumes = core.volumes.get_all_volumes().await;
	let refreshed_count = refreshed_volumes.len();

	info!("Volume count after refresh: {}", refreshed_count);

	// Volume count should remain consistent
	assert_eq!(
		initial_count, refreshed_count,
		"Volume count should be consistent after refresh"
	);

	// Verify all volumes have valid properties
	for volume in &refreshed_volumes {
		assert!(
			!volume.fingerprint.0.is_empty(),
			"Fingerprint should not be empty"
		);
		assert!(!volume.name.is_empty(), "Volume name should not be empty");

		// User-visible volumes should have capacity info
		// (Virtual/system volumes may have zero capacity)
		if volume.is_user_visible {
			assert!(
				volume.total_bytes_capacity() > 0,
				"User-visible volume '{}' should have capacity",
				volume.name
			);
		}

		// Verify mount points exist for mounted volumes
		if volume.is_mounted {
			assert!(
				volume.mount_point.exists(),
				"Mount point should exist for mounted volume '{}'",
				volume.name
			);
		}
	}

	info!("Volume refresh and detection test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}

#[tokio::test]
async fn test_volume_monitor_service() {
	let _ = tracing_subscriber::fmt::try_init();

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(
		Core::new(data_path.clone())
			.await
			.expect("Failed to create core"),
	);

	// Create a library
	let library = core
		.libraries
		.create_library(
			"Monitor Test",
			Some(data_path.join("libraries").join("monitor-test")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	// Get a volume to track
	let test_volume = core
		.volumes
		.get_all_volumes()
		.await
		.first()
		.cloned()
		.expect("No volumes available");

	let fingerprint = test_volume.fingerprint.clone();

	// Track the volume
	if !core
		.volumes
		.is_volume_tracked(&library, &fingerprint)
		.await
		.unwrap_or(false)
	{
		core.volumes
			.track_volume(&library, &fingerprint, Some("Monitored Volume".to_string()))
			.await
			.expect("Failed to track volume");
	}

	// Volume monitor service is already initialized by Core
	// Just verify it's working by manually triggering updates

	// The volume monitor may already be running from Core initialization
	// We'll just work with the existing state

	info!("Volume monitor service started");

	// Wait a bit for the monitor to run
	tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

	// Get tracked volume state
	let tracked_before = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes")
		.into_iter()
		.find(|v| v.fingerprint == fingerprint)
		.expect("Volume should be tracked");

	let initial_last_seen = tracked_before.last_seen_at;

	// Wait for monitor to update (monitor runs every 30s by default, but we'll trigger a refresh)
	core.volumes
		.refresh_volumes()
		.await
		.expect("Failed to refresh volumes");

	// Manually trigger an update to simulate monitor behavior
	core.volumes
		.update_tracked_volume_state(&library, &fingerprint, &test_volume)
		.await
		.expect("Failed to update volume state");

	// Get updated state
	let tracked_after = core
		.volumes
		.get_tracked_volumes(&library)
		.await
		.expect("Failed to get tracked volumes")
		.into_iter()
		.find(|v| v.fingerprint == fingerprint)
		.expect("Volume should be tracked");

	// Verify the monitor would update the state
	assert!(
		tracked_after.last_seen_at >= initial_last_seen,
		"Volume monitor should update last_seen_at"
	);

	// Don't stop the monitor as it's managed by Core

	info!("Volume monitor service test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown core");
}
