//! Simple integration test for volume tracking

use sd_core_new::{
    infrastructure::database::entities,
    library::LibraryConfig,
    volume::{VolumeDetectionConfig, VolumeFingerprint, VolumeManager},
};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_volume_tracking_basic() {
    // Create temp directory for library
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let library_path = temp_dir.path().join("test.sdlibrary");
    
    // Create library config
    let config = LibraryConfig {
        id: Uuid::new_v4(),
        name: "Test Library".to_string(),
        description: Some("Test library for volume tracking".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        settings: sd_core_new::library::LibrarySettings::default(),
        statistics: sd_core_new::library::LibraryStatistics::default(),
    };
    
    // Save config
    std::fs::create_dir_all(&library_path).expect("Failed to create library dir");
    let config_path = library_path.join("library.json");
    let config_json = serde_json::to_string_pretty(&config).expect("Failed to serialize config");
    std::fs::write(config_path, config_json).expect("Failed to write config");
    
    // Create events and volume manager
    let events = Arc::new(sd_core_new::infrastructure::events::EventBus::default());
    let volume_config = VolumeDetectionConfig::default();
    let volume_manager = Arc::new(VolumeManager::new(volume_config, events.clone()));
    
    // Initialize volume manager to detect volumes
    volume_manager
        .initialize()
        .await
        .expect("Failed to initialize volume manager");
    
    // Stop monitoring to avoid background tasks
    volume_manager.stop_monitoring().await;
    
    // Get all volumes
    let volumes = volume_manager.get_all_volumes().await;
    println!("Detected {} volumes", volumes.len());
    
    // Print volume information
    for volume in &volumes {
        println!(
            "Volume: {} ({}), Mounted: {}, Capacity: {} GB",
            volume.name,
            volume.fingerprint,
            volume.is_mounted,
            volume.total_bytes_capacity / (1024 * 1024 * 1024)
        );
    }
    
    // If we have at least one volume, test fingerprint parsing
    if let Some(first_volume) = volumes.first() {
        let fingerprint_str = first_volume.fingerprint.to_string();
        let parsed = VolumeFingerprint::from_string(&fingerprint_str)
            .expect("Failed to parse fingerprint");
        assert_eq!(parsed.0, fingerprint_str);
    }
}

#[tokio::test]
async fn test_volume_fingerprint_operations() {
    // Test fingerprint creation from hex
    let hex_string = "abcdef1234567890";
    let fingerprint = VolumeFingerprint::from_hex(hex_string);
    assert_eq!(fingerprint.0, hex_string);
    assert_eq!(fingerprint.to_string(), hex_string);
    
    // Test fingerprint equality
    let fp1 = VolumeFingerprint::from_hex("test123");
    let fp2 = VolumeFingerprint::from_hex("test123");
    let fp3 = VolumeFingerprint::from_hex("test456");
    
    assert_eq!(fp1, fp2);
    assert_ne!(fp1, fp3);
}

#[tokio::test]
async fn test_volume_entity_conversion() {
    use chrono::Utc;
    
    // Create a volume entity model
    let model = entities::volume::Model {
        id: 1,
        uuid: Uuid::new_v4(),
        fingerprint: "test_fingerprint_123".to_string(),
        display_name: Some("Test Volume".to_string()),
        tracked_at: Utc::now(),
        last_seen_at: Utc::now(),
        is_online: true,
        total_capacity: Some(1000000000),
        available_capacity: Some(500000000),
        read_speed_mbps: Some(100),
        write_speed_mbps: Some(80),
        last_speed_test_at: None,
        file_system: Some("APFS".to_string()),
        mount_point: Some("/Volumes/Test".to_string()),
        is_removable: Some(false),
        is_network_drive: Some(false),
        device_model: Some("Samsung SSD".to_string()),
    };
    
    // Convert to tracked volume
    let tracked = model.to_tracked_volume();
    
    // Verify conversion
    assert_eq!(tracked.id, model.id);
    assert_eq!(tracked.uuid, model.uuid);
    assert_eq!(tracked.fingerprint.0, model.fingerprint);
    assert_eq!(tracked.display_name, model.display_name);
    assert_eq!(tracked.is_online, model.is_online);
    assert_eq!(tracked.total_capacity, Some(1000000000));
    assert_eq!(tracked.available_capacity, Some(500000000));
    assert_eq!(tracked.read_speed_mbps, Some(100));
    assert_eq!(tracked.write_speed_mbps, Some(80));
    assert_eq!(tracked.file_system, model.file_system);
    assert_eq!(tracked.mount_point, model.mount_point);
    assert_eq!(tracked.is_removable, model.is_removable);
    assert_eq!(tracked.is_network_drive, model.is_network_drive);
    assert_eq!(tracked.device_model, model.device_model);
}

#[test]
fn test_volume_error_types() {
    use sd_core_new::volume::VolumeError;
    
    // Test error creation and display
    let db_error = VolumeError::Database("Connection failed".to_string());
    assert_eq!(db_error.to_string(), "Database error: Connection failed");
    
    let already_tracked = VolumeError::AlreadyTracked("vol123".to_string());
    assert_eq!(already_tracked.to_string(), "Volume is already tracked: vol123");
    
    let not_tracked = VolumeError::NotTracked("vol456".to_string());
    assert_eq!(not_tracked.to_string(), "Volume is not tracked: vol456");
    
    let not_found = VolumeError::NotFound("vol789".to_string());
    assert_eq!(not_found.to_string(), "Volume not found: vol789");
}