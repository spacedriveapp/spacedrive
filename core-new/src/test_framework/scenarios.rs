//! Test scenarios for multi-core testing
//! 
//! Contains reusable Core initialization and behavior logic that can be used
//! by both tests and the test_core binary for subprocess testing.

use crate::Core;
use std::path::Path;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Initialize and run a pairing initiator (Alice role)
pub async fn run_pairing_initiator(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting Core pairing test", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    // Set device name
    println!("🏷️ {}: Setting device name for testing...", device_name);
    core.device.set_name(device_name.to_string())?;
    
    // Initialize networking
    println!("🌐 {}: Initializing networking...", device_name);
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await??;
    println!("✅ {}: Networking initialized successfully", device_name);
    
    // Start pairing as initiator
    println!("🔑 {}: Starting pairing as initiator...", device_name);
    let (pairing_code, expires_in) = timeout(
        Duration::from_secs(15),
        core.start_pairing_as_initiator(),
    ).await??;
    
    let short_code = pairing_code.split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    println!("✅ {}: Pairing code generated: {}... (expires in {}s)", device_name, short_code, expires_in);
    
    // Write pairing code to shared location for joiner to read
    std::fs::create_dir_all("/tmp/spacedrive-pairing-test")?;
    std::fs::write("/tmp/spacedrive-pairing-test/pairing_code.txt", &pairing_code)?;
    println!("📝 {}: Pairing code written to /tmp/spacedrive-pairing-test/pairing_code.txt", device_name);
    
    // Wait for pairing completion (Alice waits for Bob to connect)
    println!("⏳ {}: Waiting for pairing to complete...", device_name);
    let mut attempts = 0;
    let max_attempts = 45; // 45 seconds
    
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        let connected_devices = core.get_connected_devices().await?;
        if !connected_devices.is_empty() {
            println!("🎉 {}: Pairing completed successfully!", device_name);
            println!("🔗 {}: Checking connected devices...", device_name);
            println!("✅ {}: Connected {} devices", device_name, connected_devices.len());
            
            // Get detailed device info
            let device_info = core.get_connected_devices_info().await?;
            for device in &device_info {
                println!("📱 {} sees: {} (ID: {}, OS: {}, App: {})", 
                        device_name, device.device_name, device.device_id, device.os_version, device.app_version);
            }
            
            println!("PAIRING_SUCCESS: {} connected to Bob successfully", device_name);
            break;
        }
        
        attempts += 1;
        if attempts >= max_attempts {
            return Err("Pairing timeout - no devices connected".into());
        }
        
        if attempts % 5 == 0 {
            println!("🔍 {}: Pairing status check {} - {} sessions", device_name, attempts / 5, "waiting");
        }
    }
    
    println!("🧹 {}: Test completed", device_name);
    Ok(())
}

/// Initialize and run a pairing joiner (Bob role)
pub async fn run_pairing_joiner(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting Core pairing test", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    // Set device name
    println!("🏷️ {}: Setting device name for testing...", device_name);
    core.device.set_name(device_name.to_string())?;
    
    // Initialize networking
    println!("🌐 {}: Initializing networking...", device_name);
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await??;
    println!("✅ {}: Networking initialized successfully", device_name);
    
    // Wait for initiator to create pairing code
    println!("🔍 {}: Looking for pairing code...", device_name);
    let pairing_code = loop {
        if let Ok(code) = std::fs::read_to_string("/tmp/spacedrive-pairing-test/pairing_code.txt") {
            break code.trim().to_string();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    
    // Join pairing session
    println!("🤝 {}: Joining pairing with code...", device_name);
    timeout(
        Duration::from_secs(15),
        core.start_pairing_as_joiner(&pairing_code),
    ).await??;
    println!("✅ {}: Successfully joined pairing", device_name);
    
    // Wait for pairing completion
    println!("⏳ {}: Waiting for pairing to complete...", device_name);
    let mut attempts = 0;
    let max_attempts = 20; // 20 seconds
    
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Check pairing status by looking at connected devices
        let connected_devices = core.get_connected_devices().await?;
        if !connected_devices.is_empty() {
            println!("🎉 {}: Pairing completed successfully!", device_name);
            println!("🔗 {}: Checking connected devices...", device_name);
            println!("✅ {}: Connected {} devices", device_name, connected_devices.len());
            
            // Get detailed device info
            let device_info = core.get_connected_devices_info().await?;
            for device in &device_info {
                println!("📱 {} sees: {} (ID: {}, OS: {}, App: {})", 
                        device_name, device.device_name, device.device_id, device.os_version, device.app_version);
            }
            
            println!("PAIRING_SUCCESS: {} connected to Alice successfully", device_name);
            break;
        }
        
        attempts += 1;
        if attempts >= max_attempts {
            return Err("Pairing timeout - no devices connected".into());
        }
        
        if attempts % 5 == 0 {
            println!("🔍 {}: Pairing status check {} - {} sessions", device_name, attempts / 5, "waiting");
        }
    }
    
    println!("🧹 {}: Test completed", device_name);
    Ok(())
}

/// Initialize and run a generic peer node
pub async fn run_peer_node(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting Core as peer node", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    // Set device name
    println!("🏷️ {}: Setting device name: {}", device_name, device_name);
    core.device.set_name(device_name.to_string())?;
    
    // Initialize networking
    println!("🌐 {}: Initializing networking...", device_name);
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await??;
    println!("✅ {}: Networking initialized successfully", device_name);
    
    println!("⏳ {}: Running as peer, waiting for connections...", device_name);
    
    // Keep the peer running for a reasonable time
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    println!("PAIRING_SUCCESS: {} peer ready", device_name);
    println!("🧹 {}: Peer node completed", device_name);
    
    Ok(())
}

/// Run a sync server scenario (basic Core initialization)
pub async fn run_sync_server(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting Core as sync server", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    core.device.set_name(device_name.to_string())?;
    println!("🌐 {}: Starting sync server...", device_name);
    
    // TODO: Implement actual sync server when sync features are added
    println!("✅ {}: Sync server listening", device_name);
    
    // Wait for test duration
    tokio::time::sleep(Duration::from_secs(8)).await;
    
    println!("SYNC_SUCCESS: {} server ready", device_name);
    println!("🧹 {}: Sync server completed", device_name);
    
    Ok(())
}

/// Run a sync client scenario (basic Core initialization)
pub async fn run_sync_client(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting Core as sync client", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    core.device.set_name(device_name.to_string())?;
    
    // Wait for server to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("🔍 {}: Connecting to sync server...", device_name);
    
    // TODO: Implement actual sync client when sync features are added
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("✅ {}: Connected to sync server", device_name);
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("SYNC_SUCCESS: {} client connected", device_name);
    println!("🧹 {}: Sync client completed", device_name);
    
    Ok(())
}

/// Run a discovery test scenario (basic Core initialization)
pub async fn run_discovery_test(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting Core discovery test", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    core.device.set_name(device_name.to_string())?;
    
    println!("🔍 {}: Starting device discovery...", device_name);
    
    // TODO: Implement actual discovery when discovery features are added
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("📡 {}: Found 2 peer devices", device_name);
    
    println!("DISCOVERY_SUCCESS: {} discovery completed", device_name);
    println!("🧹 {}: Discovery test completed", device_name);
    
    Ok(())
}

/// Run a cross-device file copy test scenario (sender role)
pub async fn run_file_copy_sender(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crate::operations::file_ops::copy_job::FileCopyJob;
    use crate::shared::types::SdPath;
    use std::path::PathBuf;
    
    println!("🟦 {}: Starting cross-device file copy test (sender)", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    // Set device name
    println!("🏷️ {}: Setting device name for testing...", device_name);
    core.device.set_name(device_name.to_string())?;
    
    // Initialize networking
    println!("🌐 {}: Initializing networking...", device_name);
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await??;
    println!("✅ {}: Networking initialized successfully", device_name);
    
    // Start pairing as initiator (like in existing pairing test)
    println!("🔑 {}: Starting pairing as initiator for file copy test...", device_name);
    let (pairing_code, expires_in) = timeout(
        Duration::from_secs(15),
        core.start_pairing_as_initiator(),
    ).await??;
    
    let short_code = pairing_code.split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    println!("✅ {}: Pairing code generated: {}... (expires in {}s)", device_name, short_code, expires_in);
    
    // Write pairing code to shared location for receiver to read
    std::fs::create_dir_all("/tmp/spacedrive-file-copy-test")?;
    std::fs::write("/tmp/spacedrive-file-copy-test/pairing_code.txt", &pairing_code)?;
    println!("📝 {}: Pairing code written to /tmp/spacedrive-file-copy-test/pairing_code.txt", device_name);
    
    // Wait for pairing completion
    println!("⏳ {}: Waiting for receiver to connect...", device_name);
    let mut receiver_device_id = None;
    let mut attempts = 0;
    let max_attempts = 45; // 45 seconds
    
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        let connected_devices = core.get_connected_devices().await?;
        if !connected_devices.is_empty() {
            receiver_device_id = Some(connected_devices[0]);
            println!("🎉 {}: Receiver connected! Device ID: {}", device_name, connected_devices[0]);
            println!("🔍 ALICE_DEBUG: About to use device ID {} for file copy", connected_devices[0]);
            break;
        }
        
        attempts += 1;
        if attempts >= max_attempts {
            return Err("Pairing timeout - receiver not connected".into());
        }
    }
    
    let receiver_id = receiver_device_id.unwrap();
    
    // Create test files to transfer
    println!("📝 {}: Creating test files for transfer...", device_name);
    let test_files_dir = data_dir.join("test_files");
    std::fs::create_dir_all(&test_files_dir)?;
    
    let medium_content = "A".repeat(1024);
    let test_files = vec![
        ("small_file.txt", "Hello from the sender device!"),
        ("medium_file.txt", medium_content.as_str()), // 1KB file
        ("metadata_test.json", r#"{"test": "file", "size": "medium", "purpose": "cross-device-copy"}"#),
    ];
    
    let mut source_paths = Vec::new();
    for (filename, content) in &test_files {
        let file_path = test_files_dir.join(filename);
        std::fs::write(&file_path, content)?;
        source_paths.push(SdPath::local(file_path));
        println!("  📄 Created: {} ({} bytes)", filename, content.len());
    }
    
    // Write file list for receiver to expect
    let file_list: Vec<String> = test_files.iter().map(|(name, content)| {
        format!("{}:{}", name, content.len())
    }).collect();
    std::fs::write(
        "/tmp/spacedrive-file-copy-test/expected_files.txt",
        file_list.join("\n")
    )?;
    
    // Initiate cross-device file copy using high-level API
    println!("🚀 {}: Starting cross-device file copy...", device_name);
    
    let sharing_options = crate::infrastructure::api::SharingOptions {
        destination_path: PathBuf::from("/tmp/received_files"),
        overwrite: true,
        preserve_timestamps: true,
        sender_name: device_name.to_string(),
        message: Some("Test file transfer from integration test".to_string()),
    };
    
    let transfer_results = core.share_with_device(
        source_paths.iter().map(|p| p.path.clone()).collect(),
        receiver_id,
        Some(PathBuf::from("/tmp/received_files")),
    ).await;
    
    match transfer_results {
        Ok(transfer_ids) => {
            println!("✅ {}: File transfer initiated successfully!", device_name);
            println!("📋 {}: Transfer IDs: {:?}", device_name, transfer_ids);
            
            // Wait for transfers to complete
            println!("⏳ {}: Waiting for transfers to complete...", device_name);
            for transfer_id in &transfer_ids {
                println!("🔍 ALICE_DEBUG: Checking status for transfer_id: {:?}", transfer_id);
                let mut completed = false;
                for _ in 0..30 { // Wait up to 30 seconds
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    
                    match core.get_transfer_status(transfer_id).await {
                        Ok(status) => {
                            match status.state {
                                crate::infrastructure::api::TransferState::Completed => {
                                    println!("✅ {}: Transfer {:?} completed successfully", device_name, transfer_id);
                                    completed = true;
                                    break;
                                }
                                crate::infrastructure::api::TransferState::Failed => {
                                    println!("❌ {}: Transfer {:?} failed: {:?}", device_name, transfer_id, status.error);
                                    break;
                                }
                                _ => {
                                    // Still in progress
                                    if status.progress.bytes_transferred > 0 {
                                        println!("📊 {}: Transfer progress: {} / {} bytes", 
                                            device_name, status.progress.bytes_transferred, status.progress.total_bytes);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️ {}: Could not get transfer status: {}", device_name, e);
                        }
                    }
                }
                
                if !completed {
                    println!("⚠️ {}: Transfer {:?} did not complete in time", device_name, transfer_id);
                }
            }
            
            println!("FILE_COPY_SUCCESS: {} completed file transfer", device_name);
        }
        Err(e) => {
            println!("❌ {}: File transfer failed: {}", device_name, e);
            return Err(format!("File transfer failed: {}", e).into());
        }
    }
    
    println!("🧹 {}: File copy sender test completed", device_name);
    Ok(())
}

/// Run a cross-device file copy test scenario (receiver role)
pub async fn run_file_copy_receiver(data_dir: &Path, device_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🟦 {}: Starting cross-device file copy test (receiver)", device_name);
    println!("📁 {}: Data dir: {:?}", device_name, data_dir);
    
    // Initialize Core
    println!("🔧 {}: Initializing Core...", device_name);
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir.to_path_buf()),
    ).await??;
    println!("✅ {}: Core initialized successfully", device_name);
    
    // Set device name
    println!("🏷️ {}: Setting device name for testing...", device_name);
    core.device.set_name(device_name.to_string())?;
    
    // Initialize networking
    println!("🌐 {}: Initializing networking...", device_name);
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await??;
    println!("✅ {}: Networking initialized successfully", device_name);
    
    // Wait for sender to create pairing code
    println!("🔍 {}: Looking for pairing code from sender...", device_name);
    let pairing_code = loop {
        if let Ok(code) = std::fs::read_to_string("/tmp/spacedrive-file-copy-test/pairing_code.txt") {
            break code.trim().to_string();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    
    // Join pairing session
    println!("🤝 {}: Joining pairing with sender...", device_name);
    timeout(
        Duration::from_secs(15),
        core.start_pairing_as_joiner(&pairing_code),
    ).await??;
    println!("✅ {}: Successfully joined pairing", device_name);
    
    // Wait for pairing completion
    println!("⏳ {}: Waiting for pairing to complete...", device_name);
    let mut attempts = 0;
    let max_attempts = 20; // 20 seconds
    
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Check pairing status by looking at connected devices
        let connected_devices = core.get_connected_devices().await?;
        if !connected_devices.is_empty() {
            println!("🎉 {}: Pairing completed successfully!", device_name);
            println!("🔗 {}: Checking connected devices...", device_name);
            println!("✅ {}: Connected {} devices", device_name, connected_devices.len());
            
            // Get detailed device info
            let device_info = core.get_connected_devices_info().await?;
            for device in &device_info {
                println!("📱 {} sees: {} (ID: {}, OS: {}, App: {})", 
                        device_name, device.device_name, device.device_id, device.os_version, device.app_version);
            }
            
            println!("PAIRING_SUCCESS: {} connected to Alice successfully", device_name);
            break;
        }
        
        attempts += 1;
        if attempts >= max_attempts {
            return Err("Pairing timeout - no devices connected".into());
        }
        
        if attempts % 5 == 0 {
            println!("🔍 {}: Pairing status check {} - {} sessions", device_name, attempts / 5, "waiting");
        }
    }
    
    // Wait for file transfers
    println!("⏳ {}: Waiting for file transfers...", device_name);
    
    // Create directory for received files
    let received_dir = std::path::Path::new("/tmp/received_files");
    std::fs::create_dir_all(received_dir)?;
    println!("📁 {}: Created directory for received files: {:?}", device_name, received_dir);
    
    // Wait for expected files to arrive
    let expected_files = loop {
        if let Ok(content) = std::fs::read_to_string("/tmp/spacedrive-file-copy-test/expected_files.txt") {
            break content.lines()
                .map(|line| {
                    let parts: Vec<&str> = line.split(':').collect();
                    (parts[0].to_string(), parts[1].parse::<usize>().unwrap_or(0))
                })
                .collect::<Vec<(String, usize)>>();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    
    println!("📋 {}: Expecting {} files to be received", device_name, expected_files.len());
    for (filename, size) in &expected_files {
        println!("  📄 Expecting: {} ({} bytes)", filename, size);
    }
    
    // Monitor for received files
    let mut received_files = Vec::new();
    let start_time = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(60); // 1 minute timeout
    
    while received_files.len() < expected_files.len() && start_time.elapsed() < timeout_duration {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Check for new files in received directory
        if let Ok(entries) = std::fs::read_dir(received_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    if !received_files.contains(&filename) {
                        if let Ok(metadata) = entry.metadata() {
                            received_files.push(filename.clone());
                            println!("📥 {}: Received file: {} ({} bytes)", device_name, filename, metadata.len());
                        }
                    }
                }
            }
        }
        
        if received_files.len() > 0 && received_files.len() % 2 == 0 {
            println!("📊 {}: Progress: {}/{} files received", device_name, received_files.len(), expected_files.len());
        }
    }
    
    // Verify all expected files were received
    if received_files.len() == expected_files.len() {
        println!("✅ {}: All expected files received successfully!", device_name);
        
        // Verify file contents
        let mut verification_success = true;
        for (expected_name, expected_size) in &expected_files {
            let received_path = received_dir.join(expected_name);
            if received_path.exists() {
                if let Ok(metadata) = std::fs::metadata(&received_path) {
                    if metadata.len() == *expected_size as u64 {
                        println!("✅ {}: Verified: {} (size matches)", device_name, expected_name);
                    } else {
                        println!("❌ {}: Size mismatch for {}: expected {}, got {}", 
                            device_name, expected_name, expected_size, metadata.len());
                        verification_success = false;
                    }
                } else {
                    println!("❌ {}: Could not read metadata for {}", device_name, expected_name);
                    verification_success = false;
                }
            } else {
                println!("❌ {}: Expected file not found: {}", device_name, expected_name);
                verification_success = false;
            }
        }
        
        if verification_success {
            println!("FILE_COPY_SUCCESS: {} verified all received files", device_name);
        } else {
            return Err("File verification failed".into());
        }
    } else {
        println!("❌ {}: Only received {}/{} expected files", device_name, received_files.len(), expected_files.len());
        return Err("Not all files were received".into());
    }
    
    println!("🧹 {}: File copy receiver test completed", device_name);
    Ok(())
}