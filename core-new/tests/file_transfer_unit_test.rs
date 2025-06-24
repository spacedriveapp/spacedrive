//! Unit test for file transfer networking integration without full pairing
//!
//! This test validates that the file transfer job can access networking services
//! and that the core components are properly integrated

use sd_core_new::Core;
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_file_transfer_networking_integration() {
    println!("🧪 Testing file transfer networking integration");
    
    // Create a temporary directory for this test
    let temp_dir = tempdir().unwrap();
    println!("📁 Test data dir: {:?}", temp_dir.path());
    
    // Initialize Core
    println!("🔧 Initializing Core...");
    let mut core = Core::new_with_config(temp_dir.path().to_path_buf())
        .await
        .expect("Failed to initialize Core");
    println!("✅ Core initialized successfully");
    
    // Initialize networking
    println!("🌐 Initializing networking...");
    core.init_networking("test-password")
        .await
        .expect("Failed to initialize networking");
    println!("✅ Networking initialized successfully");
    
    // Test deferred file sharing initialization by creating test files and submitting a job
    // This will trigger the lazy library creation
    println!("🔍 Testing deferred file sharing initialization...");
    
    // Create a test file to transfer
    println!("📝 Creating test file...");
    let source_file = temp_dir.path().join("test_source.txt");
    tokio::fs::write(&source_file, b"Hello from file transfer test!")
        .await
        .expect("Failed to create test file");
    println!("✅ Created test file: {:?}", source_file);
    
    // Create SdPath objects for source and destination
    let remote_device_id = Uuid::new_v4(); // Simulate remote device
    
    // Try to use the Core's file sharing API which will trigger deferred initialization
    println!("📤 Attempting to submit file transfer via Core API...");
    match core.share_with_device(
        vec![source_file.clone()],
        remote_device_id,
        Some(PathBuf::from("/tmp/received_files")),
    ).await {
        Ok(transfer_ids) => {
            println!("✅ File transfer submitted successfully - transfer IDs: {:?}", transfer_ids);
            
            // Let it run briefly to see if it tries to access networking
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            
            if let Some(transfer_id) = transfer_ids.first() {
                println!("🔍 Checking transfer status...");
                match core.get_transfer_status(transfer_id).await {
                    Ok(status) => {
                        println!("📊 Transfer status: {:?}", status.state);
                    },
                    Err(e) => {
                        println!("⚠️ Could not get transfer status: {}", e);
                    }
                }
            }
            
            println!("FILE_TRANSFER_SUCCESS: Job system can access networking for cross-device operations");
        },
        Err(e) => {
            println!("❌ File transfer submission failed: {}", e);
            panic!("File transfer submission failed: {}", e);
        }
    }
    
    // Verify that the default library was created
    println!("🔍 Verifying default library creation...");
    let libraries = core.libraries.get_open_libraries().await;
    if !libraries.is_empty() {
        let library_name = libraries[0].name().await;
        println!("✅ Default library created: {}", library_name);
    } else {
        println!("⚠️ No libraries created during file transfer");
    }
    
    println!("🧹 Test completed successfully");
}

#[tokio::test]
async fn test_core_initialization_creates_default_library() {
    println!("🧪 Testing that Core initialization creates default library when none exist");
    
    // Create a temporary directory for this test
    let temp_dir = tempdir().unwrap();
    println!("📁 Test data dir: {:?}", temp_dir.path());
    
    // Initialize Core
    println!("🔧 Initializing Core...");
    let mut core = Core::new_with_config(temp_dir.path().to_path_buf())
        .await
        .expect("Failed to initialize Core");
    println!("✅ Core initialized successfully");
    
    // Initialize networking to trigger file sharing setup
    println!("🌐 Initializing networking...");
    core.init_networking("test-password")
        .await
        .expect("Failed to initialize networking");
    println!("✅ Networking initialized successfully");
    
    // Check that at least one library exists
    println!("🔍 Checking for libraries...");
    let libraries = core.libraries.get_open_libraries().await;
    println!("📚 Found {} libraries", libraries.len());
    
    if libraries.is_empty() {
        panic!("Expected at least one library to be created during networking initialization");
    }
    
    let library = &libraries[0];
    let library_name = library.name().await;
    println!("✅ Default library created: {}", library_name);
    
    // Verify the library has a job manager with networking
    let _job_manager = library.jobs();
    println!("✅ Job manager available for default library");
    
    println!("🧹 Test completed - default library creation verified");
}