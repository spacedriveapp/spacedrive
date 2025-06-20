//! Production-ready device pairing demonstration
//! 
//! This demo shows the complete pairing protocol in action:
//! - Real TCP/TLS connections
//! - Full challenge-response authentication
//! - Actual session key establishment
//! - Complete error handling

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;
use rustls::crypto::aws_lc_rs;

use sd_core_new::networking::{
    identity::{DeviceInfo, NetworkIdentity, PrivateKey, PublicKey},
    pairing::{
        PairingCode, PairingConnection, PairingServer, PairingTarget,
        PairingProtocolHandler, PairingUserInterface, PairingState,
        PairingManager, PairingSession
    },
    NetworkError, Result,
};

/// Production UI that actually prompts the user
struct ProductionUI {
    device_name: String,
    auto_accept: bool,
}

#[async_trait::async_trait]
impl PairingUserInterface for ProductionUI {
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
        println!("💡 The other device should enter these 6 words");
        println!();
    }
    
    async fn prompt_pairing_code(&self) -> Result<[String; 6]> {
        println!("\n📥 Enter Pairing Code");
        println!("Please enter the 6-word pairing code from the other device:");
        println!("(Format: word1 word2 word3 word4 word5 word6)");
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
            
            if words.len() == 6 {
                return Ok([
                    words[0].clone(),
                    words[1].clone(),
                    words[2].clone(),
                    words[3].clone(),
                    words[4].clone(),
                    words[5].clone(),
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
            PairingState::Broadcasting => println!("📡 Broadcasting availability for pairing..."),
            PairingState::Scanning => println!("🔍 Scanning for pairing devices..."),
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

/// Production pairing initiator (Alice)
async fn run_pairing_initiator() -> Result<()> {
    println!("🚀 Starting Production Pairing Demo - Initiator");
    println!("==============================================");
    
    // Create device identity
    let (local_device, local_private_key) = create_test_device("Alice's MacBook Pro").await?;
    
    // Create UI
    let ui = Arc::new(ProductionUI {
        device_name: local_device.device_name.clone(),
        auto_accept: false,
    });
    
    // Generate pairing code
    let pairing_code = PairingCode::generate()?;
    ui.display_pairing_code(&pairing_code).await?;
    
    // Start pairing server
    let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = PairingServer::bind(server_addr, local_device.clone()).await?;
    let actual_addr = server.local_addr()?;
    
    println!("🎧 Pairing server listening on {}", actual_addr);
    println!("💡 Waiting for the other device to connect...");
    
    // Accept incoming pairing connection
    let timeout_duration = Duration::from_secs(300); // 5 minutes
    let connection_result = timeout(timeout_duration, server.accept()).await;
    
    match connection_result {
        Ok(Ok(mut connection)) => {
            println!("✅ Incoming connection accepted");
            
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

/// Production pairing joiner (Bob)
async fn run_pairing_joiner(server_addr: SocketAddr) -> Result<()> {
    println!("🚀 Starting Production Pairing Demo - Joiner");
    println!("===========================================");
    
    // Create device identity
    let (local_device, local_private_key) = create_test_device("Bob's iPhone").await?;
    
    // Create UI with auto-accept for demo
    let ui = Arc::new(ProductionUI {
        device_name: local_device.device_name.clone(),
        auto_accept: true, // Auto-accept for demo purposes
    });
    
    // Get pairing code from user
    let words = ui.get_pairing_code_from_user().await?;
    let pairing_code = PairingCode::from_words(&words.try_into().unwrap())?;
    
    println!("✅ Pairing code accepted");
    println!("   🔍 Discovery fingerprint: {}", hex::encode(pairing_code.discovery_fingerprint));
    
    // Connect to pairing server
    let target = PairingTarget {
        address: server_addr.ip(),
        port: server_addr.port(),
        device_name: "Unknown Device".to_string(),
        expires_at: None,
    };
    
    println!("🔗 Connecting to pairing server at {}...", server_addr);
    ui.show_pairing_progress(PairingState::Connecting).await;
    
    let mut connection = PairingConnection::connect_to_target(target, local_device.clone()).await?;
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
    
    // User confirmation (auto-accepted in demo)
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
    println!("This demo performs REAL network pairing between two devices.");
    println!("Choose your role:");
    println!("1. Initiator (generates pairing code and waits for connections)");
    println!("2. Joiner (enters pairing code and connects to initiator)");
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
                    println!("Enter the server address (format: IP:PORT):");
                    print!("> ");
                    io::stdout().flush().unwrap();
                    
                    let mut addr_input = String::new();
                    if io::stdin().read_line(&mut addr_input).is_ok() {
                        if let Ok(addr) = addr_input.trim().parse::<SocketAddr>() {
                            return run_pairing_joiner(addr).await;
                        } else {
                            println!("❌ Invalid address format. Please use IP:PORT (e.g., 127.0.0.1:8080)");
                        }
                    }
                }
                _ => {
                    println!("❌ Invalid choice. Please enter 1 or 2.");
                }
            }
        }
    }
}