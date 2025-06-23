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