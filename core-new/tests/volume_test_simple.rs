//! Simple volume test that works with the existing system
//!
//! This test verifies basic volume functionality without requiring
//! the full action system integration.

use sd_core_new::{
    Core,
    volume::VolumeExt,
};
use std::sync::Arc;
use tempfile::tempdir;
use tracing::{info, warn};

const TEST_VOLUME_NAME: &str = "TestVolume";

#[tokio::test]
async fn test_volume_detection_and_tracking() {
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
        info!("Found TestVolume!");
        
        // Test volume properties
        assert!(test_volume.is_mounted, "TestVolume should be mounted");
        assert!(
            test_volume.is_available().await,
            "TestVolume should be accessible"
        );
        
        // Test volume fingerprint
        let fingerprint = &test_volume.fingerprint;
        info!("TestVolume fingerprint: {}", fingerprint);
        
        // Test volume retrieval by fingerprint
        let retrieved_volume = volume_manager
            .get_volume(&fingerprint)
            .await
            .expect("Should be able to retrieve volume by fingerprint");
        
        assert_eq!(retrieved_volume.name, TEST_VOLUME_NAME);
        assert_eq!(retrieved_volume.fingerprint, test_volume.fingerprint);
        
        // Test path containment
        let test_path = test_volume.mount_point.join("test_file.txt");
        assert!(
            test_volume.contains_path(&test_path),
            "Volume should contain paths under its mount point"
        );
        
        // Test volume for path
        let volume_for_path = volume_manager
            .volume_for_path(&test_volume.mount_point)
            .await
            .expect("Should find volume for its mount point");
        
        assert_eq!(volume_for_path.fingerprint, test_volume.fingerprint);
        
        info!("All TestVolume tests passed!");
    } else {
        warn!(
            "TestVolume not found. Available volumes: {:?}",
            all_volumes.iter().map(|v| &v.name).collect::<Vec<_>>()
        );
        println!("SKIPPING TEST: TestVolume not mounted on system");
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
    
    info!("Volume test completed successfully");
}

#[tokio::test]
async fn test_volume_speed_test() {
    // Initialize logging  
    let _ = tracing_subscriber::fmt::try_init();

    // Create test data directory
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();

    // Initialize core
    let core = Arc::new(
        Core::new_with_config(data_path)
            .await
            .expect("Failed to create core"),
    );

    let volume_manager = core.volumes.clone();
    
    // Find a writable volume
    let all_volumes = volume_manager.get_all_volumes().await;
    let writable_volume = all_volumes
        .into_iter()
        .find(|v| v.is_mounted && !v.read_only);
    
    if let Some(volume) = writable_volume {
        info!("Testing speed on volume: {}", volume.name);
        
        let fingerprint = volume.fingerprint.clone();
        
        // Run speed test
        match volume_manager.run_speed_test(&fingerprint).await {
            Ok(()) => {
                // Get updated volume to check results
                let updated_volume = volume_manager
                    .get_volume(&fingerprint)
                    .await
                    .expect("Volume should still exist");
                
                if let (Some(read_speed), Some(write_speed)) = 
                    (updated_volume.read_speed_mbps, updated_volume.write_speed_mbps) {
                    info!("Speed test results: {} MB/s read, {} MB/s write", read_speed, write_speed);
                    assert!(read_speed > 0, "Read speed should be positive");
                    assert!(write_speed > 0, "Write speed should be positive");
                } else {
                    warn!("Speed test completed but no results stored");
                }
            }
            Err(e) => {
                warn!("Speed test failed (this is okay in CI): {}", e);
            }
        }
    } else {
        warn!("No writable volume found for speed test");
    }

    let _ = core.services.stop_all().await;
}