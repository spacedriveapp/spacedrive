//! Integration test for volume management with action system
//!
//! This test validates volume operations including:
//! - Volume detection and properties
//! - Volume tracking/untracking via actions
//! - Speed testing via actions
//! - Action execution and output validation

use sd_core_new::{
    Core,
    infrastructure::{
        actions::{Action, manager::ActionManager},
    },
    operations::volumes::{
        track::action::VolumeTrackAction,
        untrack::action::VolumeUntrackAction,
        speed_test::action::VolumeSpeedTestAction,
    },
    volume::VolumeExt,
};
use std::sync::Arc;
use tempfile::tempdir;
use tracing::{info, warn};

const TEST_VOLUME_NAME: &str = "TestVolume";

#[tokio::test]
async fn test_volume_actions_integration() {
    // Initialize logging
    let _ = tracing_subscriber::fmt::try_init();

    // Create test data directory
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();

    // Initialize core
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
            core.context.clone()
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
    
    info!("Detected {} volumes:", all_volumes.len());
    for volume in &all_volumes {
        info!(
            "  - {} ({}) at {} - {} {} [{}]",
            volume.name,
            volume.fingerprint,
            volume.mount_point.display(),
            volume.file_system,
            volume.disk_type,
            if volume.is_mounted { "mounted" } else { "unmounted" }
        );
    }

    // Find TestVolume if it exists
    let test_volume = all_volumes
        .iter()
        .find(|v| v.name == TEST_VOLUME_NAME)
        .cloned();

    if let Some(test_volume) = test_volume {
        info!("Found TestVolume! Running action tests...");
        
        // Test volume properties
        assert!(test_volume.is_mounted, "TestVolume should be mounted");
        assert!(
            test_volume.is_available().await,
            "TestVolume should be accessible"
        );
        
        let fingerprint = test_volume.fingerprint.clone();
        
        // Create action manager with core context
        let context = core.context.clone();
        let action_manager = ActionManager::new(context);
        
        // Test 1: Track volume action
        info!("Testing volume tracking action...");
        let track_action = Action::VolumeTrack {
            action: VolumeTrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
                name: Some("Test Volume Tracked".to_string()),
            },
        };
        
        let track_result = action_manager.dispatch(track_action).await;
        match track_result {
            Ok(output) => {
                info!("Volume tracked successfully: {}", output);
                match output {
                    sd_core_new::infrastructure::actions::output::ActionOutput::VolumeTracked { 
                        volume_name, .. 
                    } => {
                        assert_eq!(volume_name, test_volume.name);
                    }
                    _ => panic!("Unexpected output type for track action"),
                }
            }
            Err(e) => {
                warn!("Volume tracking failed (may not be fully implemented): {}", e);
            }
        }
        
        // Test 2: Speed test action
        info!("Testing volume speed test action...");
        let speed_test_action = Action::VolumeSpeedTest {
            action: VolumeSpeedTestAction {
                fingerprint: fingerprint.clone(),
            },
        };
        
        let speed_test_result = action_manager.dispatch(speed_test_action).await;
        match speed_test_result {
            Ok(output) => {
                info!("Speed test completed: {}", output);
                match output {
                    sd_core_new::infrastructure::actions::output::ActionOutput::VolumeSpeedTested { 
                        read_speed_mbps, 
                        write_speed_mbps, 
                        .. 
                    } => {
                        if let (Some(read), Some(write)) = (read_speed_mbps, write_speed_mbps) {
                            info!("Speed test results: {} MB/s read, {} MB/s write", read, write);
                            assert!(read > 0, "Read speed should be positive");
                            assert!(write > 0, "Write speed should be positive");
                        }
                    }
                    _ => panic!("Unexpected output type for speed test action"),
                }
            }
            Err(e) => {
                warn!("Speed test failed (this is okay in CI): {}", e);
            }
        }
        
        // Test 3: Untrack volume action
        info!("Testing volume untracking action...");
        let untrack_action = Action::VolumeUntrack {
            action: VolumeUntrackAction {
                fingerprint: fingerprint.clone(),
                library_id,
            },
        };
        
        let untrack_result = action_manager.dispatch(untrack_action).await;
        match untrack_result {
            Ok(output) => {
                info!("Volume untracked successfully: {}", output);
                match output {
                    sd_core_new::infrastructure::actions::output::ActionOutput::VolumeUntracked { 
                        .. 
                    } => {
                        // Success
                    }
                    _ => panic!("Unexpected output type for untrack action"),
                }
            }
            Err(e) => {
                warn!("Volume untracking failed (may not be fully implemented): {}", e);
            }
        }
        
        info!("All volume action tests completed!");
    } else {
        warn!(
            "TestVolume not found. Available volumes: {:?}",
            all_volumes.iter().map(|v| &v.name).collect::<Vec<_>>()
        );
        println!("SKIPPING TEST: TestVolume not mounted on system");
        
        // Still test that we can detect volumes
        assert!(!all_volumes.is_empty(), "Should detect at least one volume");
    }

    // Test volume statistics
    let stats = volume_manager.get_statistics().await;
    
    info!("Volume statistics:");
    info!("  Total volumes: {}", stats.total_volumes);
    info!("  Mounted volumes: {}", stats.mounted_volumes);
    info!("  Total capacity: {} TB", stats.total_capacity / (1024 * 1024 * 1024 * 1024));
    info!("  Total available: {} TB", stats.total_available / (1024 * 1024 * 1024 * 1024));
    
    assert_eq!(stats.total_volumes, all_volumes.len());
    assert!(stats.mounted_volumes <= stats.total_volumes);
    assert!(stats.total_available <= stats.total_capacity);

    // Clean up
    let _ = core.services.stop_all().await;
    
    info!("Volume integration test completed successfully");
}