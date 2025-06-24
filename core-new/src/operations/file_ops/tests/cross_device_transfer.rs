//! Integration tests for cross-device file transfer

use crate::{
    infrastructure::api::{FileSharing, SharingTarget, SharingOptions, TransferId},
    operations::file_ops::copy_job::FileCopyJob,
    shared::types::SdPath,
    device::DeviceManager,
};
use std::{path::PathBuf, sync::Arc};
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_file_sharing_api_creation() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    // Should be able to create without networking
    assert!(file_sharing.get_nearby_devices().await.is_ok());
    assert!(file_sharing.get_paired_devices().await.is_ok());
}

#[tokio::test] 
async fn test_cross_device_copy_job_creation() {
    // Create temporary test files
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    tokio::fs::write(&test_file, b"test content").await.unwrap();
    
    // Create SdPath objects
    let source = SdPath::local(test_file);
    let destination = SdPath::new(Uuid::new_v4(), PathBuf::from("/tmp/dest.txt"));
    
    // Create copy job
    let copy_job = FileCopyJob::from_paths(vec![source], destination);
    
    assert_eq!(copy_job.sources.paths.len(), 1);
    assert_eq!(copy_job.sources.paths[0].path.file_name().unwrap(), "test.txt");
}

#[tokio::test]
async fn test_file_sharing_with_nonexistent_file() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    let result = file_sharing.share_files(
        vec![PathBuf::from("/nonexistent/file.txt")],
        SharingTarget::PairedDevice(Uuid::new_v4()),
        SharingOptions::default(),
    ).await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), crate::infrastructure::api::SharingError::FileNotFound(_)));
}

#[tokio::test]
async fn test_file_sharing_with_existing_file() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    // Create temporary test file
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    tokio::fs::write(&test_file, b"test content").await.unwrap();
    
    let result = file_sharing.share_files(
        vec![test_file],
        SharingTarget::PairedDevice(Uuid::new_v4()),
        SharingOptions::default(),
    ).await;
    
    // Should succeed and return transfer IDs
    assert!(result.is_ok());
    let transfer_ids = result.unwrap();
    assert_eq!(transfer_ids.len(), 1);
    assert!(matches!(transfer_ids[0], TransferId::JobId(_)));
}

#[tokio::test]
async fn test_spacedrop_sharing() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    // Create temporary test file
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("spacedrop_test.txt");
    tokio::fs::write(&test_file, b"spacedrop content").await.unwrap();
    
    let options = SharingOptions {
        sender_name: "Test User".to_string(),
        message: Some("Test spacedrop message".to_string()),
        ..Default::default()
    };
    
    let result = file_sharing.share_files(
        vec![test_file],
        SharingTarget::NearbyDevices,
        options,
    ).await;
    
    // Should succeed and return spacedrop transfer IDs
    assert!(result.is_ok());
    let transfer_ids = result.unwrap();
    assert_eq!(transfer_ids.len(), 1);
    assert!(matches!(transfer_ids[0], TransferId::SpacedropId(_)));
}

#[tokio::test]
async fn test_transfer_status_tracking() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    // Create temporary test file
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("status_test.txt");
    tokio::fs::write(&test_file, b"status content").await.unwrap();
    
    let transfer_ids = file_sharing.share_files(
        vec![test_file],
        SharingTarget::PairedDevice(Uuid::new_v4()),
        SharingOptions::default(),
    ).await.unwrap();
    
    // Should be able to get status
    let status = file_sharing.get_transfer_status(&transfer_ids[0]).await;
    assert!(status.is_ok());
    
    // Should be able to cancel
    let cancel_result = file_sharing.cancel_transfer(&transfer_ids[0]).await;
    assert!(cancel_result.is_ok());
}

#[tokio::test]
async fn test_multiple_file_sharing() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    // Create multiple temporary test files
    let temp_dir = tempdir().unwrap();
    let mut files = Vec::new();
    
    for i in 0..3 {
        let test_file = temp_dir.path().join(format!("test_{}.txt", i));
        tokio::fs::write(&test_file, format!("content {}", i).as_bytes()).await.unwrap();
        files.push(test_file);
    }
    
    let result = file_sharing.share_files(
        files,
        SharingTarget::PairedDevice(Uuid::new_v4()),
        SharingOptions::default(),
    ).await;
    
    // Should succeed and return one transfer ID (job handles multiple files)
    assert!(result.is_ok());
    let transfer_ids = result.unwrap();
    assert_eq!(transfer_ids.len(), 1);
}

#[tokio::test]
async fn test_file_metadata_creation() {
    let device_manager = Arc::new(DeviceManager::init().unwrap());
    let file_sharing = FileSharing::new(None, device_manager);
    
    // Create temporary test file with known content
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("metadata_test.txt");
    let content = b"test content for metadata";
    tokio::fs::write(&test_file, content).await.unwrap();
    
    let metadata = file_sharing.create_file_metadata(&test_file).await.unwrap();
    
    assert_eq!(metadata.name, "metadata_test.txt");
    assert_eq!(metadata.size, content.len() as u64);
    assert!(!metadata.is_directory);
    assert!(metadata.modified.is_some());
}

#[tokio::test]
async fn test_sharing_options() {
    let options = SharingOptions {
        destination_path: PathBuf::from("/custom/destination"),
        overwrite: true,
        preserve_timestamps: false,
        sender_name: "Custom Sender".to_string(),
        message: Some("Custom message".to_string()),
    };
    
    assert_eq!(options.destination_path, PathBuf::from("/custom/destination"));
    assert!(options.overwrite);
    assert!(!options.preserve_timestamps);
    assert_eq!(options.sender_name, "Custom Sender");
    assert_eq!(options.message, Some("Custom message".to_string()));
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::infrastructure::networking::{NetworkingCore, protocols::{FileTransferProtocolHandler, ProtocolHandler}};
    
    /// Test that demonstrates the complete integration flow
    /// This test shows how the pieces work together but doesn't perform actual network operations
    #[tokio::test]
    async fn test_complete_integration_flow() {
        // 1. Initialize core components
        let device_manager = Arc::new(DeviceManager::init().unwrap());
        
        // 2. Create file sharing system (without networking for this test)
        let file_sharing = FileSharing::new(None, device_manager.clone());
        
        // 3. Create a test file
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("integration_test.txt");
        let test_content = b"This is a test file for cross-device transfer integration";
        tokio::fs::write(&test_file, test_content).await.unwrap();
        
        // 4. Create file transfer protocol handler
        let file_transfer_handler = FileTransferProtocolHandler::new_default();
        assert_eq!(file_transfer_handler.protocol_name(), "file_transfer");
        
        // 5. Simulate cross-device copy job creation
        let source = SdPath::local(test_file.clone());
        let target_device = Uuid::new_v4();
        let destination = SdPath::new(target_device, PathBuf::from("/tmp/received_file.txt"));
        
        let copy_job = FileCopyJob::from_paths(vec![source], destination);
        
        // 6. Verify job configuration
        assert_eq!(copy_job.sources.paths.len(), 1);
        assert_eq!(copy_job.destination.device_id, target_device);
        assert_eq!(copy_job.destination.path, PathBuf::from("/tmp/received_file.txt"));
        
        // 7. Test high-level file sharing API
        let sharing_options = SharingOptions {
            sender_name: "Integration Test".to_string(),
            message: Some("Testing complete flow".to_string()),
            ..Default::default()
        };
        
        let result = file_sharing.share_files(
            vec![test_file],
            SharingTarget::PairedDevice(target_device),
            sharing_options,
        ).await;
        
        // 8. Verify the result
        match &result {
            Ok(_) => println!("✅ File sharing succeeded"),
            Err(e) => println!("❌ File sharing failed: {:?}", e),
        }
        assert!(result.is_ok());
        let transfer_ids = result.unwrap();
        assert_eq!(transfer_ids.len(), 1);
        
        // 9. Test transfer management
        let status = file_sharing.get_transfer_status(&transfer_ids[0]).await;
        assert!(status.is_ok());
        
        let cancel_result = file_sharing.cancel_transfer(&transfer_ids[0]).await;
        assert!(cancel_result.is_ok());
        
        println!("✅ Complete integration flow test passed");
        println!("   - File sharing API: ✓");
        println!("   - Cross-device copy job: ✓");
        println!("   - File transfer protocol: ✓");
        println!("   - Transfer management: ✓");
    }
}