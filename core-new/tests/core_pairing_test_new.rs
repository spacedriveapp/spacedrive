//! Core pairing test using the new cargo test subprocess framework
//!
//! This test demonstrates the new approach where ALL test logic remains in the test file
//! while still supporting subprocess-based testing for multi-device scenarios.

use sd_core_new::test_framework_new::CargoTestRunner;
use sd_core_new::Core;
use std::path::PathBuf;
use std::env;
use std::time::Duration;
use tokio::time::timeout;
use tokio::process::Command;
use tempfile;

/// Alice's pairing scenario - ALL logic stays in this test file!
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_pairing_scenario() {
    // Exit early if not running as Alice
    if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
        return;
    }
    
    let data_dir = PathBuf::from("/tmp/spacedrive-pairing-test/alice");
    let device_name = "Alice's Test Device";
    
    println!("🟦 Alice: Starting Core pairing test");
    println!("📁 Alice: Data dir: {:?}", data_dir);
    
    // Initialize Core
    println!("🔧 Alice: Initializing Core...");
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir),
    ).await.unwrap().unwrap();
    println!("✅ Alice: Core initialized successfully");
    
    // Set device name
    println!("🏷️ Alice: Setting device name for testing...");
    core.device.set_name(device_name.to_string()).unwrap();
    
    // Initialize networking
    println!("🌐 Alice: Initializing networking...");
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await.unwrap().unwrap();
    
    // Wait longer for networking to fully initialize and detect external addresses
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("✅ Alice: Networking initialized successfully");
    
    // Start pairing as initiator
    println!("🔑 Alice: Starting pairing as initiator...");
    let (pairing_code, expires_in) = timeout(
        Duration::from_secs(15),
        core.start_pairing_as_initiator(),
    ).await.unwrap().unwrap();
    
    let short_code = pairing_code.split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    println!("✅ Alice: Pairing code generated: {}... (expires in {}s)", short_code, expires_in);
    
    // Write pairing code to shared location for Bob to read
    std::fs::create_dir_all("/tmp/spacedrive-pairing-test").unwrap();
    std::fs::write("/tmp/spacedrive-pairing-test/pairing_code.txt", &pairing_code).unwrap();
    println!("📝 Alice: Pairing code written to /tmp/spacedrive-pairing-test/pairing_code.txt");
    
    // Wait for pairing completion (Alice waits for Bob to connect)
    println!("⏳ Alice: Waiting for pairing to complete...");
    let mut attempts = 0;
    let max_attempts = 45; // 45 seconds
    
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        let connected_devices = core.get_connected_devices().await.unwrap();
        if !connected_devices.is_empty() {
            println!("🎉 Alice: Pairing completed successfully!");
            println!("🔗 Alice: Checking connected devices...");
            println!("✅ Alice: Connected {} devices", connected_devices.len());
            
            // Get detailed device info
            let device_info = core.get_connected_devices_info().await.unwrap();
            for device in &device_info {
                println!("📱 Alice sees: {} (ID: {}, OS: {}, App: {})", 
                        device.device_name, device.device_id, device.os_version, device.app_version);
            }
            
            println!("PAIRING_SUCCESS: Alice's Test Device connected to Bob successfully");
            
            // Write success marker for orchestrator to detect
            std::fs::write("/tmp/spacedrive-pairing-test/alice_success.txt", "success").unwrap();
            break;
        }
        
        attempts += 1;
        if attempts >= max_attempts {
            panic!("Alice: Pairing timeout - no devices connected");
        }
        
        if attempts % 5 == 0 {
            println!("🔍 Alice: Pairing status check {} - waiting", attempts / 5);
        }
    }
    
    println!("🧹 Alice: Test completed");
}

/// Bob's pairing scenario - ALL logic stays in this test file!
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_pairing_scenario() {
    // Exit early if not running as Bob
    if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
        return;
    }
    
    let data_dir = PathBuf::from("/tmp/spacedrive-pairing-test/bob");
    let device_name = "Bob's Test Device";
    
    println!("🟦 Bob: Starting Core pairing test");
    println!("📁 Bob: Data dir: {:?}", data_dir);
    
    // Initialize Core
    println!("🔧 Bob: Initializing Core...");
    let mut core = timeout(
        Duration::from_secs(10),
        Core::new_with_config(data_dir),
    ).await.unwrap().unwrap();
    println!("✅ Bob: Core initialized successfully");
    
    // Set device name
    println!("🏷️ Bob: Setting device name for testing...");
    core.device.set_name(device_name.to_string()).unwrap();
    
    // Initialize networking
    println!("🌐 Bob: Initializing networking...");
    timeout(
        Duration::from_secs(10),
        core.init_networking("test-password"),
    ).await.unwrap().unwrap();
    
    // Wait longer for networking to fully initialize and detect external addresses
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("✅ Bob: Networking initialized successfully");
    
    // Wait for initiator to create pairing code
    println!("🔍 Bob: Looking for pairing code...");
    let pairing_code = loop {
        if let Ok(code) = std::fs::read_to_string("/tmp/spacedrive-pairing-test/pairing_code.txt") {
            break code.trim().to_string();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    println!("📋 Bob: Found pairing code");
    
    // Join pairing session
    println!("🤝 Bob: Joining pairing with code...");
    timeout(
        Duration::from_secs(15),
        core.start_pairing_as_joiner(&pairing_code),
    ).await.unwrap().unwrap();
    println!("✅ Bob: Successfully joined pairing");
    
    // Wait for pairing completion
    println!("⏳ Bob: Waiting for pairing to complete...");
    let mut attempts = 0;
    let max_attempts = 30; // 30 seconds
    
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Check pairing status by looking at connected devices
        let connected_devices = core.get_connected_devices().await.unwrap();
        if !connected_devices.is_empty() {
            println!("🎉 Bob: Pairing completed successfully!");
            println!("🔗 Bob: Checking connected devices...");
            println!("✅ Bob: Connected {} devices", connected_devices.len());
            
            // Get detailed device info
            let device_info = core.get_connected_devices_info().await.unwrap();
            for device in &device_info {
                println!("📱 Bob sees: {} (ID: {}, OS: {}, App: {})", 
                        device.device_name, device.device_id, device.os_version, device.app_version);
            }
            
            println!("PAIRING_SUCCESS: Bob's Test Device connected to Alice successfully");
            
            // Write success marker for orchestrator to detect
            std::fs::write("/tmp/spacedrive-pairing-test/bob_success.txt", "success").unwrap();
            break;
        }
        
        attempts += 1;
        if attempts >= max_attempts {
            panic!("Bob: Pairing timeout - no devices connected");
        }
        
        if attempts % 5 == 0 {
            println!("🔍 Bob: Pairing status check {} - waiting", attempts / 5);
        }
    }
    
    println!("🧹 Bob: Test completed");
}

/// Main test orchestrator - spawns cargo test subprocesses
#[tokio::test]
async fn test_core_pairing_cargo_subprocess() {
    println!("🧪 Testing Core pairing with cargo test subprocess framework");
    
    // Clean up any old pairing files to avoid race conditions
    let _ = std::fs::remove_dir_all("/tmp/spacedrive-pairing-test");
    std::fs::create_dir_all("/tmp/spacedrive-pairing-test").unwrap();
    
    let mut runner = CargoTestRunner::new()
        .with_timeout(Duration::from_secs(180))
        .add_subprocess("alice", "alice_pairing_scenario")
        .add_subprocess("bob", "bob_pairing_scenario");
    
    // Spawn Alice first
    println!("🚀 Starting Alice as initiator...");
    runner.spawn_single_process("alice").await.expect("Failed to spawn Alice");
    
    // Wait for Alice to initialize and generate pairing code
    tokio::time::sleep(Duration::from_secs(8)).await;
    
    // Start Bob as joiner
    println!("🚀 Starting Bob as joiner...");
    runner.spawn_single_process("bob").await.expect("Failed to spawn Bob");
    
    // Run until both devices successfully pair using file markers
    let result = runner.wait_for_success(|_outputs| {
        let alice_success = std::fs::read_to_string("/tmp/spacedrive-pairing-test/alice_success.txt")
            .map(|content| content.trim() == "success")
            .unwrap_or(false);
        let bob_success = std::fs::read_to_string("/tmp/spacedrive-pairing-test/bob_success.txt")
            .map(|content| content.trim() == "success")
            .unwrap_or(false);
        
        alice_success && bob_success
    }).await;
    
    match result {
        Ok(_) => {
            println!("🎉 Cargo test subprocess pairing test successful with mutual device recognition!");
        }
        Err(e) => {
            println!("❌ Cargo test subprocess pairing test failed: {}", e);
            for (name, output) in runner.get_all_outputs() {
                println!("\\n{} output:\\n{}", name, output);
            }
            panic!("Cargo test subprocess pairing test failed - devices did not properly recognize each other");
        }
    }
}

/// Binary subprocess test - uses separate binary processes for proper isolation
#[tokio::test]
async fn test_core_pairing_binary_subprocess() {
    println!("🧪 Testing Core pairing with binary subprocess framework");
    
    // Clean up any old pairing files to avoid race conditions
    let _ = std::fs::remove_dir_all("/tmp/spacedrive-pairing-test");
    std::fs::create_dir_all("/tmp/spacedrive-pairing-test").unwrap();
    
    // Create temp directories for Alice and Bob
    let alice_dir = tempfile::tempdir().unwrap();
    let bob_dir = tempfile::tempdir().unwrap();
    
    // Start Alice as initiator using binary
    println!("🚀 Starting Alice as initiator...");
    let alice_process = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "test_core",
            "--",
            "--mode",
            "initiator",
            "--data-dir",
            alice_dir.path().to_str().unwrap(),
            "--device-name",
            "Alice's Test Device",
        ])
        .spawn()
        .expect("Failed to spawn Alice process");
    
    // Wait for Alice to initialize and generate pairing code
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Start Bob as joiner using binary
    println!("🚀 Starting Bob as joiner...");
    let bob_process = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "test_core",
            "--",
            "--mode",
            "joiner",
            "--data-dir",
            bob_dir.path().to_str().unwrap(),
            "--device-name",
            "Bob's Test Device",
        ])
        .spawn()
        .expect("Failed to spawn Bob process");
    
    // Wait for both processes to complete pairing
    let mut alice_handle = alice_process;
    let mut bob_handle = bob_process;
    
    // Wait for both to complete or timeout
    let timeout_duration = Duration::from_secs(60);
    let start_time = std::time::Instant::now();
    
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Check if both processes have completed successfully
        let alice_done = alice_handle.try_wait().unwrap_or(None);
        let bob_done = bob_handle.try_wait().unwrap_or(None);
        
        if let (Some(alice_status), Some(bob_status)) = (alice_done, bob_done) {
            if alice_status.success() && bob_status.success() {
                println!("🎉 Binary subprocess pairing test successful!");
                return;
            } else {
                panic!("Binary subprocess pairing test failed - one or both processes exited with failure");
            }
        }
        
        // Check for timeout
        if start_time.elapsed() > timeout_duration {
            // Kill both processes
            let _ = alice_handle.kill().await;
            let _ = bob_handle.kill().await;
            panic!("Binary subprocess pairing test failed - timeout");
        }
    }
}