//! Networking module demonstration
//!
//! This example shows how to use the networking module for device communication.

use sd_core_new::networking::{
    identity::NetworkIdentity,
    manager::{Network, NetworkConfig},
};
use sd_core_new::device::DeviceManager;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🌐 Spacedrive Networking Demo");
    println!("==============================\n");
    
    // Initialize device manager first
    println!("📱 Initializing device manager...");
    let device_manager = DeviceManager::init()?;
    let device_config = device_manager.config()?;
    println!("✅ Device manager initialized: {} (ID: {})", device_config.name, device_config.id);
    
    // Create network identity from device manager
    println!("🔐 Creating network identity...");
    let password = "secure_password_123";
    let identity = NetworkIdentity::from_device_manager(&device_manager, password).await?;
    println!("✅ Network identity created for device: {}", identity.device_name);
    
    // Create network instance
    println!("\n🔧 Initializing network...");
    let config = NetworkConfig::default();
    let network = Network::new(identity, config).await?;
    
    println!("✅ Network initialized successfully");
    
    // Show network capabilities
    println!("\n📊 Network Information:");
    println!("- Device ID: {}", network.identity().device_id);
    println!("- Device Name: {}", network.identity().device_name);
    println!("- Network Fingerprint: {}", network.identity().network_fingerprint);
    println!("- Configuration: {:?}", network.config());
    
    // Generate pairing code
    println!("\n🔐 Generating pairing code...");
    match network.initiate_pairing().await {
        Ok(pairing_code) => {
            println!("✅ Pairing code generated: {}", pairing_code.as_string());
            println!("⏰ Expires at: {}", pairing_code.expires_at);
        }
        Err(e) => {
            println!("❌ Failed to generate pairing code: {}", e);
        }
    }
    
    // Show connection statistics
    println!("\n📈 Connection Statistics:");
    let stats = network.connection_stats().await;
    println!("- Total connections: {}", stats.total_connections);
    println!("- Active connections: {}", stats.active_connections);
    println!("- Max connections: {}", stats.max_connections);
    
    // List known devices
    println!("\n👥 Known Devices:");
    let devices = network.known_devices().await;
    if devices.is_empty() {
        println!("- No devices paired yet");
    } else {
        for device in devices {
            println!("- {} ({})", device.device_name, device.device_id);
        }
    }
    
    println!("\n✨ Demo completed successfully!");
    
    Ok(())
}