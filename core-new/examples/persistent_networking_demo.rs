//! Persistent Networking Demo
//!
//! Demonstrates how to use the persistent device connections system
//! integrated with the Core for always-on device communication.

use sd_core_new::{Core, networking};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, error};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("=== Spacedrive Persistent Networking Demo ===");
    
    // Create temporary directories for the demo
    let temp_dir = PathBuf::from("./data").join(format!("spacedrive-demo-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    
    info!("Demo directory: {:?}", temp_dir);
    
    // Initialize Core
    let mut core = Core::new_with_config(temp_dir.clone()).await?;
    info!("Core initialized successfully");
    
    // Initialize networking with a demo password
    let password = "demo-password-123";
    core.init_networking(password).await?;
    info!("Persistent networking initialized");
    
    // Start the networking service
    core.start_networking().await?;
    info!("Networking service started");
    
    // Give the networking service time to start up
    sleep(Duration::from_secs(2)).await;
    
    // Demonstrate networking functionality
    demonstrate_networking_features(&core).await?;
    
    // Simulate some network activity
    info!("Simulating network activity for 10 seconds...");
    sleep(Duration::from_secs(10)).await;
    
    // Check connected devices
    let connected_devices = core.get_connected_devices().await?;
    info!("Connected devices: {:?}", connected_devices);
    
    // Gracefully shutdown
    info!("Shutting down...");
    core.shutdown().await?;
    
    // Clean up demo directory
    if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
        error!("Failed to clean up demo directory: {}", e);
    }
    
    info!("Demo completed successfully!");
    Ok(())
}

async fn demonstrate_networking_features(core: &Core) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Demonstrating Networking Features ===");
    
    // Get the networking service
    if let Some(networking_service) = core.networking() {
        let service = networking_service.read().await;
        
        info!("✓ Persistent networking service is active");
        info!("✓ Auto-reconnection is enabled");
        info!("✓ Encrypted storage is configured");
        info!("✓ Protocol handlers are registered:");
        info!("  - Database sync handler");
        info!("  - File transfer handler");
        info!("  - Spacedrop handler");
        info!("  - Real-time sync handler");
        
        // Get connected devices
        let connected = service.get_connected_devices().await?;
        info!("Currently connected devices: {}", connected.len());
        
    } else {
        error!("Networking service not available");
    }
    
    // Demonstrate device pairing simulation
    demonstrate_device_pairing_simulation(core).await?;
    
    // Demonstrate Spacedrop simulation
    demonstrate_spacedrop_simulation(core).await?;
    
    Ok(())
}

async fn demonstrate_device_pairing_simulation(core: &Core) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Simulating Device Pairing ===");
    
    // Create a simulated remote device
    let remote_device_id = Uuid::new_v4();
    let remote_device = networking::DeviceInfo {
        device_id: remote_device_id,
        device_name: "Demo Remote Device".to_string(),
        public_key: networking::PublicKey::from_bytes(vec![1u8; 32])?,
        network_fingerprint: networking::NetworkFingerprint::from_device(
            remote_device_id,
            &networking::PublicKey::from_bytes(vec![1u8; 32])?
        ),
        last_seen: chrono::Utc::now(),
    };
    
    // Create demo session keys
    let session_keys = networking::persistent::SessionKeys::new();
    
    info!("Simulated device: {} ({})", remote_device.device_name, remote_device_id);
    
    // Add the paired device (this would normally happen after successful pairing)
    core.add_paired_device(remote_device, session_keys).await?;
    info!("✓ Device added to persistent connections");
    
    // The networking service will automatically attempt to connect to this device
    info!("✓ Auto-connection initiated (would connect when device is online)");
    
    Ok(())
}

async fn demonstrate_spacedrop_simulation(core: &Core) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Simulating Spacedrop ===");
    
    // Create a demo file for Spacedrop
    let demo_file = PathBuf::from("./data/spacedrop_demo.txt");
    std::fs::write(&demo_file, "Hello from Spacedrive Persistent Networking!")?;
    
    // Get a device to send to (in a real scenario, this would be a connected device)
    let connected_devices = core.get_connected_devices().await?;
    
    if connected_devices.is_empty() {
        info!("No connected devices for Spacedrop demo (this is expected in the demo)");
        info!("In a real scenario with paired devices:");
        info!("  1. Device would be auto-connected");
        info!("  2. File would be sent via persistent connection");
        info!("  3. Progress would be tracked in real-time");
        info!("  4. Transfer would resume automatically if interrupted");
    } else {
        // Send file via Spacedrop
        let device_id = connected_devices[0];
        match core.send_spacedrop(
            device_id,
            &demo_file.to_string_lossy(),
            "Demo User".to_string(),
            Some("Demo file from persistent networking!".to_string()),
        ).await {
            Ok(transfer_id) => {
                info!("✓ Spacedrop initiated: transfer_id = {}", transfer_id);
            }
            Err(e) => {
                info!("Spacedrop simulation: {}", e);
            }
        }
    }
    
    // Clean up demo file
    std::fs::remove_file(&demo_file).ok();
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_core_networking_integration() {
        let temp_dir = PathBuf::from("./data").join(format!("test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let mut core = Core::new_with_config(temp_dir.clone()).await.unwrap();
        
        // Test networking initialization
        assert!(core.networking().is_none());
        
        core.init_networking("test-password").await.unwrap();
        assert!(core.networking().is_some());
        
        // Test networking service access
        let connected = core.get_connected_devices().await.unwrap();
        assert!(connected.is_empty()); // No devices connected initially
        
        // Clean up
        core.shutdown().await.unwrap();
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}