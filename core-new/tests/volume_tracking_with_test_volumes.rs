//! Enhanced volume tracking tests using real test volumes
//!
//! These tests create actual temporary volumes to test volume tracking
//! with different filesystems, capacities, and mount scenarios.

mod helpers;
use helpers::test_volumes::{TestFileSystem, TestVolumeBuilder, TestVolumeManager};

use sd_core_new::{
    Core,
    infrastructure::{
        actions::{Action, output::ActionOutput},
    },
    operations::volumes::{
        track::action::VolumeTrackAction,
        untrack::action::VolumeUntrackAction,
        speed_test::action::VolumeSpeedTestAction,
    },
};
use std::sync::Arc;
use tempfile::tempdir;
use tracing::{info, warn};

/// Check if we have the required privileges to run volume tests
async fn check_test_privileges() -> bool {
    let manager = TestVolumeManager::new();
    manager.check_privileges().await.is_ok()
}

#[tokio::test]
async fn test_real_volume_tracking_lifecycle() {
    let _ = tracing_subscriber::fmt::try_init();
    
    if !check_test_privileges().await {
        warn!("Skipping test - requires elevated privileges");
        return;
    }
    
    // Setup test environment
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();
    
    let core = Arc::new(
        Core::new_with_config(data_path.clone())
            .await
            .expect("Failed to create core"),
    );
    
    // Create test library
    let library = core
        .libraries
        .create_library(
            "Real Volume Test",
            Some(data_path.join("libraries").join("real-volume-test")),
            core.context.clone(),
        )
        .await
        .expect("Failed to create library");
    
    // Create a test volume
    let test_volume = TestVolumeBuilder::new("TestTrackingVol")
        .size_mb(50)
        .filesystem(TestFileSystem::Default)
        .build()
        .await
        .expect("Failed to create test volume");
    
    info!("Created test volume at {:?}", test_volume.path());
    
    // Refresh volumes to detect our new volume
    core.volumes
        .refresh_volumes()
        .await
        .expect("Failed to refresh volumes");
    
    // Find our test volume
    let all_volumes = core.volumes.get_all_volumes().await;
    let our_volume = all_volumes
        .iter()
        .find(|v| v.mount_point == test_volume.mount_point)
        .expect("Test volume should be detected")
        .clone();
    
    info!("Found test volume: {} ({})", our_volume.name, our_volume.fingerprint);
    
    let fingerprint = our_volume.fingerprint.clone();
    let action_manager = core.context.get_action_manager().await
        .expect("Action manager should be initialized");
    
    // Track the volume
    let track_action = Action::VolumeTrack {
        action: VolumeTrackAction {
            fingerprint: fingerprint.clone(),
            library_id: library.id(),
            name: Some("My Custom Test Volume".to_string()),
        },
    };
    
    let result = action_manager.dispatch(track_action).await;
    assert!(result.is_ok(), "Failed to track volume: {:?}", result);
    
    // Verify tracking
    let is_tracked = core.volumes
        .is_volume_tracked(&library, &fingerprint)
        .await
        .expect("Failed to check tracking status");
    assert!(is_tracked, "Volume should be tracked");
    
    // Get tracked volume info
    let tracked_volumes = core.volumes
        .get_tracked_volumes(&library)
        .await
        .expect("Failed to get tracked volumes");
    
    let tracked = tracked_volumes
        .iter()
        .find(|v| v.fingerprint == fingerprint)
        .expect("Our volume should be in tracked list");
    
    assert_eq!(tracked.display_name, Some("My Custom Test Volume".to_string()));
    assert!(tracked.is_online);
    assert_eq!(tracked.total_capacity, Some(50 * 1024 * 1024)); // 50MB
    
    // Untrack the volume
    let untrack_action = Action::VolumeUntrack {
        action: VolumeUntrackAction {
            fingerprint: fingerprint.clone(),
            library_id: library.id(),
        },
    };
    
    let result = action_manager.dispatch(untrack_action).await;
    assert!(result.is_ok(), "Failed to untrack volume");
    
    // Volume cleanup happens automatically via Drop
    info!("Real volume tracking lifecycle test completed");
}

#[tokio::test]
async fn test_different_filesystems() {
    let _ = tracing_subscriber::fmt::try_init();
    
    if !check_test_privileges().await {
        warn!("Skipping test - requires elevated privileges");
        return;
    }
    
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();
    
    let core = Arc::new(
        Core::new_with_config(data_path.clone())
            .await
            .expect("Failed to create core"),
    );
    
    let library = core
        .libraries
        .create_library(
            "Filesystem Test",
            Some(data_path.join("libraries").join("fs-test")),
            core.context.clone(),
        )
        .await
        .expect("Failed to create library");
    
    // Test different filesystems based on platform
    #[cfg(target_os = "macos")]
    let filesystems = vec![
        (TestFileSystem::Apfs, "APFS"),
        (TestFileSystem::HfsPlus, "HFS+"),
        (TestFileSystem::ExFat, "ExFAT"),
    ];
    
    #[cfg(target_os = "windows")]
    let filesystems = vec![
        (TestFileSystem::Ntfs, "NTFS"),
        (TestFileSystem::Fat32, "FAT32"),
        (TestFileSystem::ExFat, "ExFAT"),
    ];
    
    #[cfg(target_os = "linux")]
    let filesystems = vec![
        (TestFileSystem::Ext4, "ext4"),
        (TestFileSystem::Fat32, "FAT32"),
    ];
    
    for (fs_type, fs_name) in filesystems {
        info!("Testing {} filesystem", fs_name);
        
        let volume_name = format!("Test{}", fs_name.replace("+", "Plus"));
        let test_volume = match TestVolumeBuilder::new(&volume_name)
            .size_mb(30)
            .filesystem(fs_type)
            .build()
            .await
        {
            Ok(vol) => vol,
            Err(e) => {
                warn!("Failed to create {} volume: {}", fs_name, e);
                continue;
            }
        };
        
        // Refresh to detect the volume
        core.volumes.refresh_volumes().await.ok();
        
        let all_volumes = core.volumes.get_all_volumes().await;
        if let Some(volume) = all_volumes
            .iter()
            .find(|v| v.mount_point == test_volume.mount_point)
        {
            info!("Detected {} volume: {}", fs_name, volume.name);
            
            // Track the volume
            core.volumes
                .track_volume(&library, &volume.fingerprint, Some(format!("{} Test", fs_name)))
                .await
                .expect("Failed to track volume");
            
            // Verify filesystem info
            let tracked = core.volumes
                .get_tracked_volumes(&library)
                .await
                .expect("Failed to get tracked volumes")
                .into_iter()
                .find(|v| v.fingerprint == volume.fingerprint)
                .expect("Volume should be tracked");
            
            assert_eq!(tracked.file_system, Some(volume.file_system.to_string()));
            info!("Successfully tracked {} volume", fs_name);
            
            // Untrack before next iteration
            core.volumes
                .untrack_volume(&library, &volume.fingerprint)
                .await
                .ok();
        }
    }
    
    info!("Filesystem test completed");
}

#[tokio::test]
async fn test_volume_capacity_scenarios() {
    let _ = tracing_subscriber::fmt::try_init();
    
    if !check_test_privileges().await {
        warn!("Skipping test - requires elevated privileges");
        return;
    }
    
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();
    
    let core = Arc::new(
        Core::new_with_config(data_path.clone())
            .await
            .expect("Failed to create core"),
    );
    
    let library = core
        .libraries
        .create_library(
            "Capacity Test",
            Some(data_path.join("libraries").join("capacity-test")),
            core.context.clone(),
        )
        .await
        .expect("Failed to create library");
    
    // Create volumes with different sizes
    let test_cases = vec![
        ("TinyVol", 10),    // 10 MB
        ("SmallVol", 50),   // 50 MB
        ("MediumVol", 200), // 200 MB
    ];
    
    for (name, size_mb) in test_cases {
        info!("Testing {} MB volume", size_mb);
        
        let test_volume = match TestVolumeBuilder::new(name)
            .size_mb(size_mb)
            .build()
            .await
        {
            Ok(vol) => vol,
            Err(e) => {
                warn!("Failed to create {} MB volume: {}", size_mb, e);
                continue;
            }
        };
        
        // Refresh and find volume
        core.volumes.refresh_volumes().await.ok();
        
        let all_volumes = core.volumes.get_all_volumes().await;
        if let Some(volume) = all_volumes
            .iter()
            .find(|v| v.mount_point == test_volume.mount_point)
        {
            // Track the volume
            core.volumes
                .track_volume(&library, &volume.fingerprint, Some(name.to_string()))
                .await
                .expect("Failed to track volume");
            
            // Verify capacity
            let tracked = core.volumes
                .get_tracked_volumes(&library)
                .await
                .expect("Failed to get tracked volumes")
                .into_iter()
                .find(|v| v.fingerprint == volume.fingerprint)
                .expect("Volume should be tracked");
            
            assert_eq!(
                tracked.total_capacity,
                Some((size_mb as u64) * 1024 * 1024),
                "Volume capacity should match"
            );
            
            // Test speed on different sized volumes
            let action_manager = core.context.get_action_manager().await.unwrap();
            let speed_action = Action::VolumeSpeedTest {
                action: VolumeSpeedTestAction {
                    fingerprint: volume.fingerprint.clone(),
                },
            };
            
            match action_manager.dispatch(speed_action).await {
                Ok(ActionOutput::VolumeSpeedTested { read_speed_mbps, write_speed_mbps, .. }) => {
                    info!("{}: Read {:?} MB/s, Write {:?} MB/s", 
                          name, read_speed_mbps, write_speed_mbps);
                }
                _ => {
                    warn!("Speed test failed for {}", name);
                }
            }
            
            // Cleanup
            core.volumes
                .untrack_volume(&library, &volume.fingerprint)
                .await
                .ok();
        }
    }
    
    info!("Capacity test completed");
}

#[tokio::test]
async fn test_ram_disk_performance() {
    let _ = tracing_subscriber::fmt::try_init();
    
    if !check_test_privileges().await {
        warn!("Skipping test - requires elevated privileges");
        return;
    }
    
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();
    
    let core = Arc::new(
        Core::new_with_config(data_path.clone())
            .await
            .expect("Failed to create core"),
    );
    
    let library = core
        .libraries
        .create_library(
            "RAM Disk Test",
            Some(data_path.join("libraries").join("ramdisk-test")),
            core.context.clone(),
        )
        .await
        .expect("Failed to create library");
    
    // Create RAM disk for performance testing
    let ram_volume = match TestVolumeBuilder::new("RAMDisk")
        .size_mb(100)
        .use_ram_disk()
        .build()
        .await
    {
        Ok(vol) => vol,
        Err(e) => {
            warn!("Failed to create RAM disk: {} - skipping test", e);
            return;
        }
    };
    
    info!("Created RAM disk at {:?}", ram_volume.path());
    
    // Refresh and find the RAM disk
    core.volumes.refresh_volumes().await.ok();
    
    let all_volumes = core.volumes.get_all_volumes().await;
    if let Some(volume) = all_volumes
        .iter()
        .find(|v| v.mount_point == ram_volume.mount_point)
    {
        // Track the RAM disk
        core.volumes
            .track_volume(&library, &volume.fingerprint, Some("Fast RAM Disk".to_string()))
            .await
            .expect("Failed to track RAM disk");
        
        // Run speed test - should be very fast
        let action_manager = core.context.get_action_manager().await.unwrap();
        let speed_action = Action::VolumeSpeedTest {
            action: VolumeSpeedTestAction {
                fingerprint: volume.fingerprint.clone(),
            },
        };
        
        match action_manager.dispatch(speed_action).await {
            Ok(ActionOutput::VolumeSpeedTested { read_speed_mbps, write_speed_mbps, .. }) => {
                info!("RAM Disk speeds - Read: {:?} MB/s, Write: {:?} MB/s", 
                      read_speed_mbps, write_speed_mbps);
                
                // RAM disks should be very fast
                if let (Some(read), Some(write)) = (read_speed_mbps, write_speed_mbps) {
                    assert!(read > 100, "RAM disk read speed should be > 100 MB/s");
                    assert!(write > 100, "RAM disk write speed should be > 100 MB/s");
                }
            }
            _ => {
                warn!("Speed test failed for RAM disk");
            }
        }
    }
    
    info!("RAM disk performance test completed");
}

#[tokio::test]
async fn test_volume_mount_unmount_tracking() {
    let _ = tracing_subscriber::fmt::try_init();
    
    if !check_test_privileges().await {
        warn!("Skipping test - requires elevated privileges");
        return;
    }
    
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();
    
    let core = Arc::new(
        Core::new_with_config(data_path.clone())
            .await
            .expect("Failed to create core"),
    );
    
    let library = core
        .libraries
        .create_library(
            "Mount Test",
            Some(data_path.join("libraries").join("mount-test")),
            core.context.clone(),
        )
        .await
        .expect("Failed to create library");
    
    // Create a test volume that we'll unmount and remount
    let manager = TestVolumeManager::new();
    let config = helpers::test_volumes::TestVolumeConfig {
        name: "RemountTest".to_string(),
        size_bytes: 50 * 1024 * 1024,
        filesystem: TestFileSystem::Default,
        read_only: false,
        use_ram_disk: false,
    };
    
    let test_volume = manager.create_volume(config.clone()).await
        .expect("Failed to create test volume");
    
    // Refresh and find volume
    core.volumes.refresh_volumes().await.ok();
    
    let all_volumes = core.volumes.get_all_volumes().await;
    let volume = all_volumes
        .iter()
        .find(|v| v.mount_point == test_volume.mount_point)
        .expect("Test volume should be detected")
        .clone();
    
    // Track the volume
    core.volumes
        .track_volume(&library, &volume.fingerprint, Some("Remountable Volume".to_string()))
        .await
        .expect("Failed to track volume");
    
    // Verify it's tracked and online
    let tracked = core.volumes
        .get_tracked_volumes(&library)
        .await
        .expect("Failed to get tracked volumes")
        .into_iter()
        .find(|v| v.fingerprint == volume.fingerprint)
        .expect("Volume should be tracked");
    
    assert!(tracked.is_online, "Volume should be online initially");
    
    // Destroy the volume (unmount it)
    manager.destroy_volume(test_volume).await
        .expect("Failed to destroy volume");
    
    // Refresh volumes - the volume should no longer be detected
    core.volumes.refresh_volumes().await.ok();
    
    // Update tracked volume state
    if let Some(current) = core.volumes.get_volume(&volume.fingerprint).await {
        core.volumes
            .update_tracked_volume_state(&library, &volume.fingerprint, &current)
            .await
            .ok();
    }
    
    // Check if volume is now offline in tracking
    let tracked_after = core.volumes
        .get_tracked_volumes(&library)
        .await
        .expect("Failed to get tracked volumes")
        .into_iter()
        .find(|v| v.fingerprint == volume.fingerprint)
        .expect("Volume should still be tracked");
    
    // The volume should still be tracked but might be offline
    assert_eq!(tracked_after.display_name, Some("Remountable Volume".to_string()));
    
    info!("Mount/unmount tracking test completed");
}

#[tokio::test]
async fn test_concurrent_volume_operations() {
    let _ = tracing_subscriber::fmt::try_init();
    
    if !check_test_privileges().await {
        warn!("Skipping test - requires elevated privileges");
        return;
    }
    
    let data_dir = tempdir().unwrap();
    let data_path = data_dir.path().to_path_buf();
    
    let core = Arc::new(
        Core::new_with_config(data_path.clone())
            .await
            .expect("Failed to create core"),
    );
    
    // Create multiple libraries
    let mut libraries = Vec::new();
    for i in 0..3 {
        let lib = core
            .libraries
            .create_library(
                format!("Concurrent Lib {}", i),
                Some(data_path.join("libraries").join(format!("concurrent-{}", i))),
                core.context.clone(),
            )
            .await
            .expect("Failed to create library");
        libraries.push(lib);
    }
    
    // Create a test volume
    let test_volume = TestVolumeBuilder::new("ConcurrentVol")
        .size_mb(100)
        .build()
        .await
        .expect("Failed to create test volume");
    
    // Refresh to detect volume
    core.volumes.refresh_volumes().await.ok();
    
    let all_volumes = core.volumes.get_all_volumes().await;
    let volume = all_volumes
        .iter()
        .find(|v| v.mount_point == test_volume.mount_point)
        .expect("Test volume should be detected")
        .clone();
    
    // Track the same volume in all libraries concurrently
    let mut tasks = Vec::new();
    for (i, library) in libraries.iter().enumerate() {
        let lib = library.clone();
        let vol_manager = core.volumes.clone();
        let fingerprint = volume.fingerprint.clone();
        let name = format!("Concurrent Volume {}", i);
        
        let task = tokio::spawn(async move {
            vol_manager
                .track_volume(&lib, &fingerprint, Some(name))
                .await
        });
        
        tasks.push(task);
    }
    
    // Wait for all tracking operations
    let results: Vec<_> = futures::future::join_all(tasks).await;
    
    // All should succeed
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok(), "Task {} failed to join", i);
        assert!(result.as_ref().unwrap().is_ok(), 
                "Library {} failed to track volume", i);
    }
    
    // Verify all libraries have the volume tracked
    for (i, library) in libraries.iter().enumerate() {
        let tracked = core.volumes
            .get_tracked_volumes(library)
            .await
            .expect("Failed to get tracked volumes");
        
        // Find our specific test volume (there might be auto-tracked system volumes)
        let our_volume = tracked
            .iter()
            .find(|v| v.fingerprint == volume.fingerprint)
            .expect(&format!("Library {} should have our test volume tracked", i));
        
        assert_eq!(
            our_volume.display_name,
            Some(format!("Concurrent Volume {}", i)),
            "Library {} should have correct volume name", i
        );
    }
    
    info!("Concurrent volume operations test completed");
}