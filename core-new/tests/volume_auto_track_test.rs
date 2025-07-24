//! Test automatic volume tracking functionality

use sd_core_new::{
    context::CoreContext,
    infrastructure::events::EventBus,
    library::{LibraryConfig, LibraryManager, LibrarySettings},
    volume::{VolumeDetectionConfig, VolumeManager},
};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_auto_track_system_volumes_on_library_open() {
    // Setup test environment
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    
    // Initialize volume manager
    let events = Arc::new(EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize volume manager to detect volumes
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Get system volumes before library creation
    let system_volumes = volume_manager.get_system_volumes().await;
    println!("Found {} system volumes", system_volumes.len());
    
    // Create library manager
    let library_manager = Arc::new(LibraryManager::new_with_dir(
        temp_dir.path().join("libraries"),
        events.clone(),
    ));
    
    // Create core context
    let context = CoreContext::test_with_volume_manager(
        data_dir,
        volume_manager.clone(),
    )
    .await
    .expect("Failed to create test context");
    let context = Arc::new(context);
    
    // Create a library with auto-tracking enabled (default)
    let library = library_manager
        .create_library("Test Library", None, context.clone())
        .await
        .expect("Failed to create library");
    
    // Verify system volumes were auto-tracked
    let tracked_volumes = volume_manager
        .get_tracked_volumes(&library)
        .await
        .expect("Failed to get tracked volumes");
    
    // Should have tracked all system volumes
    assert_eq!(
        tracked_volumes.len(),
        system_volumes.len(),
        "Should have auto-tracked all system volumes"
    );
    
    // Verify each system volume is tracked
    for sys_vol in &system_volumes {
        let is_tracked = tracked_volumes
            .iter()
            .any(|tv| tv.fingerprint.0 == sys_vol.fingerprint.0);
        assert!(
            is_tracked,
            "System volume '{}' should be tracked",
            sys_vol.name
        );
    }
}

#[tokio::test]
async fn test_auto_track_disabled() {
    // Setup test environment
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    
    // Initialize volume manager
    let events = Arc::new(EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize volume manager
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Create library manager
    let library_manager = Arc::new(LibraryManager::new_with_dir(
        temp_dir.path().join("libraries"),
        events.clone(),
    ));
    
    // Create library path manually
    let library_path = temp_dir.path().join("libraries").join("test.sdlibrary");
    std::fs::create_dir_all(&library_path).expect("Failed to create library dir");
    
    // Create config with auto-tracking disabled
    let mut settings = LibrarySettings::default();
    settings.auto_track_system_volumes = false;
    
    let config = LibraryConfig {
        version: 1,
        id: Uuid::new_v4(),
        name: "Test Library".to_string(),
        description: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        settings,
        statistics: sd_core_new::library::LibraryStatistics::default(),
    };
    
    // Save config
    let config_json = serde_json::to_string_pretty(&config).expect("Failed to serialize config");
    std::fs::write(library_path.join("library.json"), config_json).expect("Failed to write config");
    
    // Create database
    let db_path = library_path.join("database.db");
    let db = sd_core_new::infrastructure::database::Database::create(&db_path)
        .await
        .expect("Failed to create database");
    db.migrate().await.expect("Failed to run migrations");
    
    // Create context
    let context = CoreContext::test_with_volume_manager(
        data_dir,
        volume_manager.clone(),
    )
    .await
    .expect("Failed to create test context");
    let context = Arc::new(context);
    
    // Open library with auto-tracking disabled
    let library = library_manager
        .open_library_with_context(&library_path, context.clone())
        .await
        .expect("Failed to open library");
    
    // Verify no volumes were auto-tracked
    let tracked_volumes = volume_manager
        .get_tracked_volumes(&library)
        .await
        .expect("Failed to get tracked volumes");
    
    assert_eq!(
        tracked_volumes.len(),
        0,
        "Should not have auto-tracked any volumes"
    );
}

#[tokio::test]
async fn test_system_volume_properties() {
    // Initialize volume manager
    let events = Arc::new(EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize to detect volumes
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Get system volumes
    let system_volumes = volume_manager.get_system_volumes().await;
    
    // Verify system volume properties
    for volume in system_volumes {
        // System volumes should be mounted
        assert!(volume.is_mounted, "System volume should be mounted");
        
        // System volumes should have capacity info
        assert!(volume.total_bytes_capacity > 0, "System volume should have capacity");
        
        // System volumes typically have specific names
        println!(
            "System volume: {} ({}), FS: {}, Capacity: {} GB",
            volume.name,
            volume.mount_point.display(),
            volume.file_system,
            volume.total_bytes_capacity / (1024 * 1024 * 1024)
        );
    }
}