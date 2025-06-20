//! Production-ready device pairing demonstration with mDNS discovery
//! 
//! This demo shows the complete pairing protocol with two connection methods:
//! - Automatic mDNS discovery (recommended)
//! - Direct IP connection (fallback)
//!
//! Features demonstrated:
//! - Real TCP/TLS connections
//! - Automatic device discovery via mDNS
//! - Full challenge-response authentication
//! - Session key establishment
//! - Complete error handling

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use sd_core_new::networking::{
    identity::{DeviceInfo, PrivateKey},
    pairing::{
        PairingCode, PairingConnection, PairingServer, PairingTarget, PairingDiscovery,
        PairingProtocolHandler, PairingUserInterface, PairingState,
        DiscoveryEvent
    },
    NetworkError, Result,
};

/// Enhanced UI that shows discovery results
struct DiscoveryUI {
    device_name: String,
    auto_accept: bool,
}

#[async_trait::async_trait]
impl PairingUserInterface for DiscoveryUI {
    async fn show_pairing_error(&self, error: &NetworkError) {
        println!("❌ Pairing error: {}", error);
    }
    
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
        println!("\n📋 Your Pairing Code");
        println!("Share this code with the other device:");
        println!();
        println!("    {}", code);
        println!();
        println!("⏰ Expires in {} seconds", expires_in_seconds);
        println!("💡 The other device should enter these 12 words or find you via network discovery");
        println!();
    }
    
    async fn prompt_pairing_code(&self) -> Result<[String; 12]> {
        println!("\n📥 Enter Pairing Code");
        println!("Please enter the 12-word pairing code from the other device:");
        println!("(Format: word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12)");
        print!("> ");
        
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let words: Vec<String> = input
                .trim()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            
            if words.len() == 12 {
                return Ok([
                    words[0].clone(),
                    words[1].clone(),
                    words[2].clone(),
                    words[3].clone(),
                    words[4].clone(),
                    words[5].clone(),
                    words[6].clone(),
                    words[7].clone(),
                    words[8].clone(),
                    words[9].clone(),
                    words[10].clone(),
                    words[11].clone(),
                ]);
            }
        }
        
        Err(NetworkError::AuthenticationFailed("Invalid pairing code format".to_string()))
    }
    
    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool> {
        if self.auto_accept {
            println!("🤖 Auto-accepting pairing with {}", remote_device.device_name);
            return Ok(true);
        }
        
        println!("\n🔐 Pairing Request");
        println!("Device '{}' wants to pair with this device.", remote_device.device_name);
        println!("Device ID: {}", remote_device.device_id);
        println!("Network Fingerprint: {}", remote_device.network_fingerprint);
        println!("Last seen: {}", remote_device.last_seen);
        
        loop {
            print!("Accept this pairing? [y/N]: ");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                match input.trim().to_lowercase().as_str() {
                    "y" | "yes" => return Ok(true),
                    "n" | "no" | "" => return Ok(false),
                    _ => println!("Please enter 'y' for yes or 'n' for no."),
                }
            }
        }
    }
    
    async fn show_pairing_progress(&self, state: PairingState) {
        match state {
            PairingState::GeneratingCode => println!("🔐 Generating pairing code..."),
            PairingState::Broadcasting => println!("📡 Broadcasting availability via mDNS..."),
            PairingState::Scanning => println!("🔍 Scanning for pairing devices via mDNS..."),
            PairingState::Connecting => println!("🔗 Establishing secure connection..."),
            PairingState::Authenticating => println!("🔐 Performing mutual authentication..."),
            PairingState::ExchangingKeys => println!("🔄 Exchanging device information..."),
            PairingState::AwaitingConfirmation => println!("⏳ Awaiting user confirmation..."),
            PairingState::EstablishingSession => println!("🔑 Establishing session keys..."),
            PairingState::Completed => println!("✅ Pairing completed successfully!"),
            PairingState::Failed(err) => println!("❌ Pairing failed: {}", err),
            _ => {}
        }
    }
}

impl DiscoveryUI {
    /// Display discovered devices and let user choose
    async fn choose_discovered_device(&self, devices: Vec<(PairingTarget, [u8; 16])>) -> Result<Option<PairingTarget>> {
        if devices.is_empty() {
            println!("🔍 No pairing devices found on the network");
            return Ok(None);
        }
        
        println!("\n📱 Discovered Pairing Devices:");
        println!("=====================================");
        
        for (i, (target, fingerprint)) in devices.iter().enumerate() {
            println!("{}. {} ({}:{})", 
                i + 1, 
                target.device_name, 
                target.address, 
                target.port
            );
            println!("   🔐 Fingerprint: {}", hex::encode(fingerprint));
            if let Some(expires) = target.expires_at {
                println!("   ⏰ Expires: {}", expires);
            }
            println!();
        }
        
        println!("{}. Enter pairing code manually", devices.len() + 1);
        println!("{}. Connect to IP address directly", devices.len() + 2);
        println!();
        
        loop {
            print!("Choose device [1-{}]: ", devices.len() + 2);
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                if let Ok(choice) = input.trim().parse::<usize>() {
                    if choice >= 1 && choice <= devices.len() {
                        return Ok(Some(devices[choice - 1].0.clone()));
                    } else if choice == devices.len() + 1 {
                        // Manual pairing code entry
                        return Ok(None);
                    } else if choice == devices.len() + 2 {
                        // Direct IP connection
                        return self.prompt_direct_ip_connection().await;
                    }
                }
                println!("❌ Invalid choice. Please enter a number between 1 and {}", devices.len() + 2);
            }
        }
    }
    
    /// Prompt for direct IP connection
    async fn prompt_direct_ip_connection(&self) -> Result<Option<PairingTarget>> {
        println!("\n🌐 Direct IP Connection");
        println!("Enter the IP address and port of the device to connect to:");
        print!("Address (format: IP:PORT): ");
        
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            if let Ok(addr) = input.trim().parse::<SocketAddr>() {
                println!("📥 Now enter the pairing code from that device:");
                let words = self.prompt_pairing_code().await?;
                let code = PairingCode::from_words(&words)?;
                
                let target = PairingTarget {
                    address: addr.ip(),
                    port: addr.port(),
                    device_name: "Direct Connection".to_string(),
                    expires_at: Some(code.expires_at),
                };
                
                return Ok(Some(target));
            } else {
                println!("❌ Invalid address format. Please use IP:PORT (e.g., 192.168.1.100:12345)");
            }
        }
        
        Ok(None)
    }
}

/// Create a test device identity
async fn create_test_device(name: &str) -> Result<(DeviceInfo, PrivateKey)> {
    let device_id = Uuid::new_v4();
    let private_key = PrivateKey::generate()?;
    let public_key = private_key.public_key();
    
    let device_info = DeviceInfo::new(device_id, name.to_string(), public_key);
    
    println!("📱 Created device: {} (ID: {})", name, device_id);
    println!("   🔐 Network Fingerprint: {}", device_info.network_fingerprint);
    
    Ok((device_info, private_key))
}

/// Production pairing initiator with mDNS broadcasting
async fn run_pairing_initiator() -> Result<()> {
    println!("🚀 Starting Production Pairing Demo - Initiator");
    println!("==============================================");
    
    // Create device identity
    let (local_device, local_private_key) = create_test_device("Alice's MacBook Pro").await?;
    
    // Create UI
    let ui = Arc::new(DiscoveryUI {
        device_name: local_device.device_name.clone(),
        auto_accept: false,
    });
    
    // Generate pairing code
    let pairing_code = PairingCode::generate()?;
    ui.show_pairing_code(&pairing_code.as_string(), 
        pairing_code.time_remaining().unwrap_or_default().num_seconds() as u32).await;
    
    // Start pairing server on random port
    let server_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let server = PairingServer::bind(server_addr, local_device.clone()).await?;
    let actual_addr = server.local_addr()?;
    
    println!("🎧 Pairing server listening on {}", actual_addr);
    
    // Start mDNS discovery and broadcasting
    let mut discovery = PairingDiscovery::new(local_device.clone())?;
    ui.show_pairing_progress(PairingState::Broadcasting).await;
    discovery.start_broadcast(&pairing_code, actual_addr.port()).await?;
    
    println!("📡 Broadcasting pairing availability via mDNS");
    println!("💡 Other devices can now discover this device automatically");
    println!("💡 Or they can connect directly to: {}", actual_addr);
    println!("⏳ Waiting for connection...");
    
    // Accept incoming pairing connection
    let timeout_duration = Duration::from_secs(300); // 5 minutes
    let connection_result = timeout(timeout_duration, server.accept()).await;
    
    // Stop broadcasting when someone connects
    discovery.stop_broadcast().await?;
    
    match connection_result {
        Ok(Ok(mut connection)) => {
            println!("✅ Incoming connection accepted from {}", connection.peer_addr()?);
            
            // Perform authentication as initiator
            ui.show_pairing_progress(PairingState::Authenticating).await;
            PairingProtocolHandler::authenticate_as_initiator(&mut connection, &pairing_code).await?;
            println!("✅ Authentication successful");
            
            // Exchange device information (initiator sends first)
            ui.show_pairing_progress(PairingState::ExchangingKeys).await;
            let remote_device = PairingProtocolHandler::exchange_device_information_as_initiator(
                &mut connection, 
                &local_private_key
            ).await?;
            println!("✅ Device information exchange successful");
            println!("   📱 Remote device: {}", remote_device.device_name);
            
            // User confirmation
            ui.show_pairing_progress(PairingState::AwaitingConfirmation).await;
            let confirmed = ui.confirm_pairing(&remote_device).await?;
            if !confirmed {
                println!("❌ User rejected pairing");
                return Ok(());
            }
            
            // Establish session keys (initiator sends first)
            ui.show_pairing_progress(PairingState::EstablishingSession).await;
            let session_keys = PairingProtocolHandler::establish_session_keys_as_initiator(&mut connection).await?;
            println!("✅ Session keys established");
            println!("   🔑 Send key: {}", hex::encode(session_keys.send_key));
            println!("   🔑 Receive key: {}", hex::encode(session_keys.receive_key));
            println!("   🔑 MAC key: {}", hex::encode(session_keys.mac_key));
            
            ui.show_pairing_progress(PairingState::Completed).await;
            println!("\n🎉 Production pairing completed successfully!");
            println!("✅ {} is now paired with {}", 
                local_device.device_name, remote_device.device_name);
        }
        Ok(Err(e)) => {
            println!("❌ Failed to accept connection: {}", e);
            return Err(e);
        }
        Err(_) => {
            println!("⏰ Timeout waiting for connection");
            return Err(NetworkError::ConnectionTimeout);
        }
    }
    
    Ok(())
}

/// Production pairing joiner with automatic discovery
async fn run_pairing_joiner() -> Result<()> {
    println!("🚀 Starting Production Pairing Demo - Joiner");
    println!("===========================================");
    
    // Create device identity
    let (local_device, local_private_key) = create_test_device("Bob's iPhone").await?;
    
    // Create UI
    let ui = Arc::new(DiscoveryUI {
        device_name: local_device.device_name.clone(),
        auto_accept: false,
    });
    
    // Start mDNS discovery
    let mut discovery = PairingDiscovery::new(local_device.clone())?;
    ui.show_pairing_progress(PairingState::Scanning).await;
    
    println!("🔍 Scanning for pairing devices on the network...");
    println!("⏳ Please wait while we search for available devices...");
    
    // Start continuous scanning
    let mut event_receiver = discovery.start_continuous_scan().await?;
    let mut discovered_devices = Vec::new();
    
    // Collect devices for a few seconds
    let scan_timeout = Duration::from_secs(10);
    let _scan_result = timeout(scan_timeout, async {
        loop {
            match event_receiver.recv().await {
                Some(DiscoveryEvent::DeviceFound { target, fingerprint }) => {
                    println!("📱 Found device: {} at {}:{}", 
                        target.device_name, target.address, target.port);
                    discovered_devices.push((target, fingerprint));
                }
                Some(DiscoveryEvent::DeviceLost { address }) => {
                    println!("📤 Device lost: {}", address);
                    discovered_devices.retain(|(target, _)| target.address != address);
                }
                Some(DiscoveryEvent::Error { error }) => {
                    println!("⚠️  Discovery error: {}", error);
                }
                Some(DiscoveryEvent::BroadcastStarted { .. }) => {
                    // Ignore broadcast events in joiner mode
                }
                Some(DiscoveryEvent::BroadcastStopped) => {
                    // Ignore broadcast events in joiner mode
                }
                None => break,
            }
        }
    }).await;
    
    // Present discovery results to user
    let target_option = ui.choose_discovered_device(discovered_devices).await?;
    
    let (target, pairing_code) = if let Some(target) = target_option {
        // User selected a discovered device, now get the pairing code
        println!("📥 Selected device: {} at {}:{}", target.device_name, target.address, target.port);
        let words = ui.prompt_pairing_code().await?;
        let code = PairingCode::from_words(&words)?;
        (target, code)
    } else {
        // User chose manual entry, get both target and code
        println!("📥 Manual pairing code entry selected");
        let words = ui.prompt_pairing_code().await?;
        let code = PairingCode::from_words(&words)?;
        
        // Try to discover the device with this code
        println!("🔍 Searching for device with pairing code...");
        let discovered_target = discovery.scan_for_pairing_device(&code, Duration::from_secs(30)).await?;
        (discovered_target, code)
    };
    
    println!("✅ Pairing code accepted");
    println!("   🔍 Discovery fingerprint: {}", hex::encode(pairing_code.discovery_fingerprint));
    
    // Connect to the target
    println!("🔗 Connecting to {}:{}...", target.address, target.port);
    ui.show_pairing_progress(PairingState::Connecting).await;
    
    let mut connection = PairingConnection::connect_to_target(target.clone(), local_device.clone()).await?;
    println!("✅ Connection established");
    
    // Perform authentication as joiner
    ui.show_pairing_progress(PairingState::Authenticating).await;
    PairingProtocolHandler::authenticate_as_joiner(&mut connection, &pairing_code).await?;
    println!("✅ Authentication successful");
    
    // Exchange device information (joiner receives first)
    ui.show_pairing_progress(PairingState::ExchangingKeys).await;
    let remote_device = PairingProtocolHandler::exchange_device_information_as_joiner(
        &mut connection, 
        &local_private_key
    ).await?;
    println!("✅ Device information exchange successful");
    println!("   📱 Remote device: {}", remote_device.device_name);
    
    // User confirmation
    ui.show_pairing_progress(PairingState::AwaitingConfirmation).await;
    let confirmed = ui.confirm_pairing(&remote_device).await?;
    if !confirmed {
        println!("❌ User rejected pairing");
        return Ok(());
    }
    
    // Establish session keys (joiner receives first)
    ui.show_pairing_progress(PairingState::EstablishingSession).await;
    let session_keys = PairingProtocolHandler::establish_session_keys_as_joiner(&mut connection).await?;
    println!("✅ Session keys established");
    println!("   🔑 Send key: {}", hex::encode(session_keys.send_key));
    println!("   🔑 Receive key: {}", hex::encode(session_keys.receive_key));
    println!("   🔑 MAC key: {}", hex::encode(session_keys.mac_key));
    
    ui.show_pairing_progress(PairingState::Completed).await;
    println!("\n🎉 Production pairing completed successfully!");
    println!("✅ {} is now paired with {}", 
        local_device.device_name, remote_device.device_name);
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize crypto provider for rustls 0.23
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| NetworkError::EncryptionError("Failed to install crypto provider".to_string()))?;
    
    // Initialize tracing for debugging
    tracing_subscriber::fmt::init();
    
    println!("🔗 Spacedrive Production Pairing Protocol Demo");
    println!("===============================================");
    println!("This demo performs REAL network pairing with automatic device discovery!");
    println!();
    println!("🌟 Features:");
    println!("  • Automatic mDNS device discovery");
    println!("  • Manual pairing code entry");
    println!("  • Direct IP connection fallback");
    println!("  • Real TLS encryption");
    println!("  • Challenge-response authentication");
    println!();
    println!("Choose your role:");
    println!("1. Initiator (generates pairing code and broadcasts availability)");
    println!("2. Joiner (discovers and connects to pairing devices)");
    println!();
    
    loop {
        print!("Enter your choice [1/2]: ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            match input.trim() {
                "1" => {
                    println!("\n🎯 You chose: Initiator");
                    return run_pairing_initiator().await;
                }
                "2" => {
                    println!("\n🎯 You chose: Joiner");
                    return run_pairing_joiner().await;
                }
                _ => {
                    println!("❌ Invalid choice. Please enter 1 or 2.");
                }
            }
        }
    }
}