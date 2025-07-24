//! Integration tests for volume tracking functionality

use sd_core_new::{
    Core,
    infrastructure::{
        actions::{Action, output::ActionOutput},
    },
    operations::volumes::{
        track::action::VolumeTrackAction,
        untrack::action::VolumeUntrackAction,
    },
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
        Core::new_with_config(data_path.clone())
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
    
    // Get first available volume for testing
    let test_volume = all_volumes
        .first()
        .expect("No volumes available for testing")
        .clone();
    
    info!("Using volume '{}' for testing", test_volume.name);
    
    let fingerprint = test_volume.fingerprint.clone();
    
    // Get action manager from core context
    let action_manager = core.context.get_action_manager().await
        .expect("Action manager should be initialized");
    
    // Test 1: Check if volume is already tracked (from auto-tracking)
    info!("Checking initial tracking status...");
    let initial_tracked = volume_manager
        .is_volume_tracked(&library, &fingerprint)
        .await
        .expect("Failed to check tracking status");
    
    if initial_tracked {
        info!("Volume is already tracked (from auto-tracking), untracking first");
        
        // Untrack it first so we can test tracking
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
            },
        };
        
        let result = action_manager.dispatch(untrack_action).await;
        assert!(result.is_ok(), "Failed to untrack volume: {:?}", result);
    }
    
    // Test 1: Track volume
    info!("Testing volume tracking...");
    {
        let track_action = Action::VolumeTrack {
            action: VolumeTrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
                name: Some("My Test Volume".to_string()),
            },
        };
        
        let result = action_manager.dispatch(track_action).await;
        
        assert!(result.is_ok(), "Failed to track volume: {:?}", result);
        
        if let Ok(ActionOutput::VolumeTracked { volume_name, .. }) = result {
            info!("Volume tracked successfully as '{}'", volume_name);
        }
        
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
        let our_volume = tracked_volumes.iter()
            .find(|v| v.fingerprint == fingerprint)
            .expect("Our volume should be in tracked volumes");
            
        assert_eq!(
            our_volume.display_name,
            Some("My Test Volume".to_string())
        );
    }
    
    // Test 2: Try to track same volume again (should fail)
    info!("Testing duplicate tracking prevention...");
    {
        let track_action = Action::VolumeTrack {
            action: VolumeTrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
                name: Some("Another Name".to_string()),
            },
        };
        
        let result = action_manager.dispatch(track_action).await;
        
        assert!(result.is_err(), "Should not be able to track volume twice");
        info!("Duplicate tracking correctly prevented");
    }
    
    // Test 3: Untrack volume
    info!("Testing volume untracking...");
    {
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
            },
        };
        
        let result = action_manager.dispatch(untrack_action).await;
        
        assert!(result.is_ok(), "Failed to untrack volume: {:?}", result);
        
        if let Ok(ActionOutput::VolumeUntracked { .. }) = result {
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
        
        let our_volume_still_tracked = tracked_volumes.iter()
            .any(|v| v.fingerprint == fingerprint);
        assert!(!our_volume_still_tracked, "Our volume should no longer be tracked");
    }
    
    // Test 4: Try to untrack volume that's not tracked (should fail)
    info!("Testing untrack of non-tracked volume...");
    {
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
            },
        };
        
        let result = action_manager.dispatch(untrack_action).await;
        
        assert!(result.is_err(), "Should not be able to untrack non-tracked volume");
        info!("Untrack of non-tracked volume correctly prevented");
    }
    
    info!("Volume tracking lifecycle test completed successfully");
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
        Core::new_with_config(data_path.clone())
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
    
    // Get first available volume
    let test_volume = volume_manager
        .get_all_volumes()
        .await
        .first()
        .expect("No volumes available for testing")
        .clone();
    
    let fingerprint = test_volume.fingerprint.clone();
    
    // Get action manager from core context
    let action_manager = core.context.get_action_manager().await
        .expect("Action manager should be initialized");
    
    // Check if volume is already tracked in library 1 (from auto-tracking)
    let is_tracked_lib1 = volume_manager
        .is_volume_tracked(&library1, &fingerprint)
        .await
        .expect("Failed to check tracking status");
        
    if is_tracked_lib1 {
        info!("Volume already tracked in library 1, untracking first");
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id: library1_id,
            },
        };
        action_manager.dispatch(untrack_action).await
            .expect("Failed to untrack from library 1");
    }
    
    // Track volume in library 1
    info!("Tracking volume in library 1...");
    {
        let track_action = Action::VolumeTrack {
            action: VolumeTrackAction {
                fingerprint: fingerprint.clone(),
                library_id: library1_id,
                name: Some("Library 1 Volume".to_string()),
            },
        };
        
        let result = action_manager.dispatch(track_action).await;
        assert!(result.is_ok(), "Failed to track volume in library 1");
    }
    
    // Check if volume is already tracked in library 2 (from auto-tracking)
    let is_tracked_lib2 = volume_manager
        .is_volume_tracked(&library2, &fingerprint)
        .await
        .expect("Failed to check tracking status");
        
    if is_tracked_lib2 {
        info!("Volume already tracked in library 2, untracking first");
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id: library2_id,
            },
        };
        action_manager.dispatch(untrack_action).await
            .expect("Failed to untrack from library 2");
    }
    
    // Track same volume in library 2 (should succeed)
    info!("Tracking same volume in library 2...");
    {
        let track_action = Action::VolumeTrack {
            action: VolumeTrackAction {
                fingerprint: fingerprint.clone(),
                library_id: library2_id,
                name: Some("Library 2 Volume".to_string()),
            },
        };
        
        let result = action_manager.dispatch(track_action).await;
        assert!(result.is_ok(), "Should be able to track volume in different library");
    }
    
    // Verify both libraries have the volume tracked
    let lib1_volumes = volume_manager
        .get_tracked_volumes(&library1)
        .await
        .expect("Failed to get library 1 volumes");
    
    let lib1_our_volume = lib1_volumes.iter()
        .find(|v| v.fingerprint == fingerprint)
        .expect("Our volume should be in library 1");
    assert_eq!(lib1_our_volume.display_name, Some("Library 1 Volume".to_string()));
    
    let lib2_volumes = volume_manager
        .get_tracked_volumes(&library2)
        .await
        .expect("Failed to get library 2 volumes");
        
    let lib2_our_volume = lib2_volumes.iter()
        .find(|v| v.fingerprint == fingerprint)
        .expect("Our volume should be in library 2");
    assert_eq!(lib2_our_volume.display_name, Some("Library 2 Volume".to_string()));
    
    // Untrack from library 1
    info!("Untracking volume from library 1...");
    {
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id: library1_id,
            },
        };
        
        let result = action_manager.dispatch(untrack_action).await;
        assert!(result.is_ok(), "Failed to untrack from library 1");
    }
    
    // Verify library 2 still has it tracked
    let lib2_volumes = volume_manager
        .get_tracked_volumes(&library2)
        .await
        .expect("Failed to get library 2 volumes");
    
    let lib2_still_has_volume = lib2_volumes.iter()
        .any(|v| v.fingerprint == fingerprint);
    assert!(lib2_still_has_volume, "Library 2 should still have volume tracked");
    
    info!("Multiple library volume tracking test completed successfully");
}