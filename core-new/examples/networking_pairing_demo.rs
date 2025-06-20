//! Production-ready device pairing demonstration with libp2p networking
//! 
//! This demo shows the complete pairing protocol using the new libp2p stack:
//! - Global DHT-based discovery (replaces mDNS)
//! - Multi-transport support (TCP + QUIC)
//! - NAT traversal and hole punching
//! - Noise Protocol encryption (replaces TLS)
//! - Request-response messaging over libp2p
//!
//! Features demonstrated:
//! - Global device discovery via Kademlia DHT
//! - Full challenge-response authentication
//! - Session key establishment
//! - Production-ready libp2p networking

use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use sd_core_new::infrastructure::networking::{
    identity::{DeviceInfo, PrivateKey, NetworkIdentity},
    pairing::{PairingCode, PairingUserInterface, PairingState},
    LibP2PPairingProtocol,
    NetworkError, Result,
};

/// Enhanced UI for libp2p pairing demo
struct LibP2PUI {
    device_name: String,
    auto_accept: bool,
}

#[async_trait::async_trait]
impl PairingUserInterface for LibP2PUI {
    async fn show_pairing_error(&self, error: &NetworkError) {
        println!("❌ Pairing error: {}", error);
    }
    
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
        println!("\n📋 Your Pairing Code (LibP2P)");
        println!("Share this code with the other device:");
        println!();
        println!("    {}", code);
        println!();
        println!("⏰ Expires in {} seconds", expires_in_seconds);
        println!("💡 The other device will find you via global DHT discovery");
        println!("🌐 No more mDNS limitations - works across networks!");
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
        
        // Use async stdin to avoid blocking the network event loop
        println!();
        print!("Accept this pairing? [y/N]: ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        // Read input in a way that doesn't interfere with debug logging
        let input = tokio::task::spawn_blocking(|| {
            let mut input = String::new();
            io::stdin().read_line(&mut input).ok().map(|_| input)
        }).await.unwrap_or(None);
        
        if let Some(input) = input {
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => return Ok(true),
                "n" | "no" | "" => return Ok(false),
                _ => {
                    println!("Invalid input. Rejecting pairing.");
                    return Ok(false);
                }
            }
        } else {
            println!("Failed to read input. Rejecting pairing.");
            return Ok(false);
        }
    }
    
    async fn show_pairing_progress(&self, state: PairingState) {
        match state {
            PairingState::GeneratingCode => println!("🔐 Generating pairing code..."),
            PairingState::Broadcasting => println!("📡 Providing on Kademlia DHT (global discovery)..."),
            PairingState::Scanning => println!("🔍 Searching Kademlia DHT for pairing devices..."),
            PairingState::Connecting => println!("🔗 Establishing libp2p connection (TCP/QUIC)..."),
            PairingState::Authenticating => println!("🔐 Authenticating via request-response protocol..."),
            PairingState::ExchangingKeys => println!("🔄 Exchanging device information over libp2p..."),
            PairingState::AwaitingConfirmation => println!("⏳ Awaiting user confirmation..."),
            PairingState::EstablishingSession => println!("🔑 Establishing session keys..."),
            PairingState::Completed => println!("✅ LibP2P pairing completed successfully!"),
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

/// Production pairing initiator with libp2p DHT
async fn run_pairing_initiator() -> Result<()> {
    println!("🚀 Starting Production Pairing Demo - Initiator (LibP2P)");
    println!("=======================================================");
    
    // Create device identity
    let (local_device, local_private_key) = create_test_device("Alice's MacBook Pro").await?;
    
    // Create network identity for libp2p
    let network_identity = NetworkIdentity::new_temporary(
        local_device.device_id,
        local_device.device_name.clone(),
        "production_demo_password"
    )?;
    
    // Create UI
    let ui = Arc::new(LibP2PUI {
        device_name: local_device.device_name.clone(),
        auto_accept: false,
    });
    
    println!("🔧 Initializing libp2p networking...");
    
    // Create simple libp2p pairing protocol
    let mut pairing_protocol = LibP2PPairingProtocol::new(
        &network_identity,
        local_device.clone(),
        local_private_key,
        "production_demo_password"
    ).await?;
    
    println!("✅ Production LibP2P pairing protocol initialized");
    println!("🌐 Peer ID: {}", pairing_protocol.local_peer_id());
    
    // Start listening
    let listening_addrs = pairing_protocol.start_listening().await?;
    println!("📡 Listening on addresses: {:?}", listening_addrs);
    println!("📡 Ready for global DHT discovery");
    
    // Start pairing as initiator
    match pairing_protocol.start_as_initiator(&*ui).await {
        Ok((remote_device, session_keys)) => {
            println!("\n🎉 Production pairing completed successfully!");
            println!("✅ {} is now paired with {}", 
                local_device.device_name, remote_device.device_name);
            println!("   🔑 Send key: {}", hex::encode(session_keys.send_key));
            println!("   🔑 Receive key: {}", hex::encode(session_keys.receive_key));
            println!("   🔑 MAC key: {}", hex::encode(session_keys.mac_key));
        }
        Err(e) => {
            println!("❌ Pairing failed: {}", e);
            return Err(e);
        }
    }
    
    Ok(())
}

/// Production pairing joiner with libp2p DHT discovery
async fn run_pairing_joiner() -> Result<()> {
    println!("🚀 Starting Production Pairing Demo - Joiner (LibP2P)");
    println!("====================================================");
    
    // Create device identity
    let (local_device, local_private_key) = create_test_device("Bob's iPhone").await?;
    
    // Create network identity for libp2p
    let network_identity = NetworkIdentity::new_temporary(
        local_device.device_id,
        local_device.device_name.clone(),
        "production_demo_password"
    )?;
    
    // Create UI
    let ui = Arc::new(LibP2PUI {
        device_name: local_device.device_name.clone(),
        auto_accept: false,
    });
    
    // Get pairing code from user
    let words = ui.prompt_pairing_code().await?;
    let pairing_code = PairingCode::from_words(&words)?;
    println!("✅ Pairing code accepted");
    println!("   🔍 Discovery fingerprint: {}", hex::encode(pairing_code.discovery_fingerprint));
    
    println!("🔧 Initializing libp2p networking...");
    
    // Create simple libp2p pairing protocol
    let mut pairing_protocol = LibP2PPairingProtocol::new(
        &network_identity,
        local_device.clone(),
        local_private_key,
        "production_demo_password"
    ).await?;
    
    println!("✅ Production LibP2P pairing protocol initialized");
    println!("🌐 Peer ID: {}", pairing_protocol.local_peer_id());
    
    // Start listening
    let listening_addrs = pairing_protocol.start_listening().await?;
    println!("📡 Listening on addresses: {:?}", listening_addrs);
    println!("🔍 Starting DHT discovery...");
    
    // Start pairing as joiner
    match pairing_protocol.start_as_joiner(&*ui, pairing_code).await {
        Ok((remote_device, session_keys)) => {
            println!("\n🎉 Production pairing completed successfully!");
            println!("✅ {} is now paired with {}", 
                local_device.device_name, remote_device.device_name);
            println!("   🔑 Send key: {}", hex::encode(session_keys.send_key));
            println!("   🔑 Receive key: {}", hex::encode(session_keys.receive_key));
            println!("   🔑 MAC key: {}", hex::encode(session_keys.mac_key));
        }
        Err(e) => {
            println!("❌ Pairing failed: {}", e);
            return Err(e);
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for debugging
    tracing_subscriber::fmt::init();
    
    println!("🔗 Spacedrive Production Pairing Protocol Demo (LibP2P)");
    println!("========================================================");
    println!("This demo performs REAL network pairing with the NEW libp2p stack!");
    println!();
    println!("🌟 LibP2P Features:");
    println!("  • Global DHT-based discovery (no more mDNS limitations!)");
    println!("  • Multi-transport: TCP + QUIC");
    println!("  • NAT traversal and hole punching");
    println!("  • Noise Protocol encryption (replaces TLS)");
    println!("  • Production-ready (used by IPFS, Polkadot, etc.)");
    println!("  • Works across networks and the internet");
    println!();
    println!("Choose your role:");
    println!("1. Initiator (generates pairing code and provides on DHT)");
    println!("2. Joiner (searches DHT and connects to pairing devices)");
    println!();
    
    loop {
        print!("Enter your choice [1/2]: ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            match input.trim() {
                "1" => {
                    println!("\n🎯 You chose: LibP2P Initiator");
                    if let Err(e) = run_pairing_initiator().await {
                        println!("❌ Initiator failed: {}", e);
                        return Err(e);
                    }
                    println!("\n🔄 Pairing completed! Keeping connection alive...");
                    println!("Press Ctrl+C to exit.");
                    
                    // Keep the demo running to maintain the connection
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
                "2" => {
                    println!("\n🎯 You chose: LibP2P Joiner");
                    if let Err(e) = run_pairing_joiner().await {
                        println!("❌ Joiner failed: {}", e);
                        return Err(e);
                    }
                    println!("\n🔄 Pairing completed! Keeping connection alive...");
                    println!("Press Ctrl+C to exit.");
                    
                    // Keep the demo running to maintain the connection
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
                _ => {
                    println!("❌ Invalid choice. Please enter 1 or 2.");
                }
            }
        }
    }
}