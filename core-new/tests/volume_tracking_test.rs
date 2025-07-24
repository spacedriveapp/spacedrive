//! Integration tests for volume tracking functionality

use core_new::{
    context::CoreContext,
    infrastructure::{
        actions::{Action, manager::ActionManager},
        database::Database,
        events::EventBus,
    },
    library::{Library, LibraryConfig, LibraryManager},
    operations::volumes::{
        track::action::VolumeTrackAction,
        untrack::action::VolumeUntrackAction,
    },
    volume::{VolumeDetectionConfig, VolumeFingerprint, VolumeManager},
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

/// Helper to create a test library
async fn create_test_library(
    context: Arc<CoreContext>,
    temp_dir: &TempDir,
) -> Arc<Library> {
    let library_path = temp_dir.path().join("test_library.sdlibrary");
    let config = LibraryConfig {
        id: Uuid::new_v4(),
        name: "Test Library".to_string(),
        description: Some("Test library for volume tracking".to_string()),
        ..Default::default()
    };
    
    context
        .library_manager
        .create_library(library_path, config)
        .await
        .expect("Failed to create test library")
}

/// Helper to get first available volume for testing
async fn get_test_volume(volume_manager: &Arc<VolumeManager>) -> Option<(VolumeFingerprint, String)> {
    let volumes = volume_manager.get_all_volumes().await;
    volumes.first().map(|v| (v.fingerprint.clone(), v.name.clone()))
}

#[tokio::test]
async fn test_volume_tracking_lifecycle() {
    // Setup test environment
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    
    // Initialize core context
    let events = Arc::new(EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize volume manager
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Create test context with volume manager
    let context = CoreContext::test_with_volume_manager(data_dir, volume_manager.clone())
        .await
        .expect("Failed to create test context");
    let context = Arc::new(context);
    
    // Create test library
    let library = create_test_library(context.clone(), &temp_dir).await;
    
    // Get a test volume
    let (fingerprint, volume_name) = get_test_volume(&volume_manager)
        .await
        .expect("No volumes available for testing");
    
    // Test 1: Track volume
    {
        let track_action = VolumeTrackAction {
            library_id: library.id(),
            fingerprint: fingerprint.clone(),
            name: Some("My Test Volume".to_string()),
        };
        
        let action = Action::VolumeTrack { action: track_action };
        let result = context.action_manager.execute(action).await;
        
        assert!(result.is_ok(), "Failed to track volume: {:?}", result);
        
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
        assert_eq!(tracked_volumes.len(), 1, "Should have one tracked volume");
        assert_eq!(tracked_volumes[0].fingerprint, fingerprint);
        assert_eq!(
            tracked_volumes[0].display_name,
            Some("My Test Volume".to_string())
        );
    }
    
    // Test 2: Try to track same volume again (should fail)
    {
        let track_action = VolumeTrackAction {
            library_id: library.id(),
            fingerprint: fingerprint.clone(),
            name: Some("Another Name".to_string()),
        };
        
        let action = Action::VolumeTrack { action: track_action };
        let result = context.action_manager.execute(action).await;
        
        assert!(result.is_err(), "Should not be able to track volume twice");
    }
    
    // Test 3: Untrack volume
    {
        let untrack_action = VolumeUntrackAction {
            library_id: library.id(),
            fingerprint: fingerprint.clone(),
        };
        
        let action = Action::VolumeUntrack { action: untrack_action };
        let result = context.action_manager.execute(action).await;
        
        assert!(result.is_ok(), "Failed to untrack volume: {:?}", result);
        
        // Verify volume is no longer tracked
        let is_tracked = volume_manager
            .is_volume_tracked(&library, &fingerprint)
            .await
            .expect("Failed to check tracking status");
        assert!(!is_tracked, "Volume should not be tracked");
        
        // Get tracked volumes (should be empty)
        let tracked_volumes = volume_manager
            .get_tracked_volumes(&library)
            .await
            .expect("Failed to get tracked volumes");
        assert_eq!(tracked_volumes.len(), 0, "Should have no tracked volumes");
    }
    
    // Test 4: Try to untrack non-tracked volume (should fail)
    {
        let untrack_action = VolumeUntrackAction {
            library_id: library.id(),
            fingerprint: fingerprint.clone(),
        };
        
        let action = Action::VolumeUntrack { action: untrack_action };
        let result = context.action_manager.execute(action).await;
        
        assert!(result.is_err(), "Should not be able to untrack non-tracked volume");
    }
}

#[tokio::test]
async fn test_volume_state_updates() {
    // Setup test environment
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    
    // Initialize core context
    let events = Arc::new(EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize volume manager
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Create test context
    let context = CoreContext::test_with_volume_manager(data_dir, volume_manager.clone())
        .await
        .expect("Failed to create test context");
    let context = Arc::new(context);
    
    // Create test library
    let library = create_test_library(context.clone(), &temp_dir).await;
    
    // Get a test volume
    let (fingerprint, _) = get_test_volume(&volume_manager)
        .await
        .expect("No volumes available for testing");
    
    // Track volume
    let track_action = VolumeTrackAction {
        library_id: library.id(),
        fingerprint: fingerprint.clone(),
        name: None,
    };
    
    let action = Action::VolumeTrack { action: track_action };
    context
        .action_manager
        .execute(action)
        .await
        .expect("Failed to track volume");
    
    // Get initial state
    let tracked_volumes = volume_manager
        .get_tracked_volumes(&library)
        .await
        .expect("Failed to get tracked volumes");
    let initial_state = tracked_volumes[0].clone();
    
    // Update volume state (simulate volume change)
    if let Some(current_volume) = volume_manager.get_volume(&fingerprint).await {
        volume_manager
            .update_tracked_volume_state(&library, &fingerprint, &current_volume)
            .await
            .expect("Failed to update volume state");
        
        // Get updated state
        let tracked_volumes = volume_manager
            .get_tracked_volumes(&library)
            .await
            .expect("Failed to get tracked volumes");
        let updated_state = tracked_volumes[0].clone();
        
        // Verify state was updated
        assert!(updated_state.last_seen_at > initial_state.last_seen_at);
    }
}

#[tokio::test]
async fn test_multiple_libraries_tracking_same_volume() {
    // Setup test environment
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    
    // Initialize core context
    let events = Arc::new(EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize volume manager
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Create test context
    let context = CoreContext::test_with_volume_manager(data_dir, volume_manager.clone())
        .await
        .expect("Failed to create test context");
    let context = Arc::new(context);
    
    // Create two test libraries
    let library1 = create_test_library(context.clone(), &temp_dir).await;
    let library2 = create_test_library(context.clone(), &temp_dir).await;
    
    // Get a test volume
    let (fingerprint, _) = get_test_volume(&volume_manager)
        .await
        .expect("No volumes available for testing");
    
    // Track volume in library 1
    let track_action1 = VolumeTrackAction {
        library_id: library1.id(),
        fingerprint: fingerprint.clone(),
        name: Some("Library 1 Volume".to_string()),
    };
    
    let action1 = Action::VolumeTrack { action: track_action1 };
    context
        .action_manager
        .execute(action1)
        .await
        .expect("Failed to track volume in library 1");
    
    // Track same volume in library 2 (should work)
    let track_action2 = VolumeTrackAction {
        library_id: library2.id(),
        fingerprint: fingerprint.clone(),
        name: Some("Library 2 Volume".to_string()),
    };
    
    let action2 = Action::VolumeTrack { action: track_action2 };
    context
        .action_manager
        .execute(action2)
        .await
        .expect("Failed to track volume in library 2");
    
    // Verify both libraries have the volume tracked
    let lib1_volumes = volume_manager
        .get_tracked_volumes(&library1)
        .await
        .expect("Failed to get library 1 volumes");
    assert_eq!(lib1_volumes.len(), 1);
    assert_eq!(lib1_volumes[0].display_name, Some("Library 1 Volume".to_string()));
    
    let lib2_volumes = volume_manager
        .get_tracked_volumes(&library2)
        .await
        .expect("Failed to get library 2 volumes");
    assert_eq!(lib2_volumes.len(), 1);
    assert_eq!(lib2_volumes[0].display_name, Some("Library 2 Volume".to_string()));
    
    // Untrack from library 1
    let untrack_action = VolumeUntrackAction {
        library_id: library1.id(),
        fingerprint: fingerprint.clone(),
    };
    
    let action = Action::VolumeUntrack { action: untrack_action };
    context
        .action_manager
        .execute(action)
        .await
        .expect("Failed to untrack from library 1");
    
    // Verify library 2 still has it tracked
    let lib2_volumes = volume_manager
        .get_tracked_volumes(&library2)
        .await
        .expect("Failed to get library 2 volumes");
    assert_eq!(lib2_volumes.len(), 1);
}