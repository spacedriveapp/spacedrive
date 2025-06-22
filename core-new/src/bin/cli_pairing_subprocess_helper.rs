//! Subprocess helper for CLI pairing integration tests
//! This binary allows spawning separate processes for Alice and Bob

use sd_core_new::Core;
use sd_core_new::networking::{
    identity::{DeviceInfo, NetworkIdentity, PrivateKey},
    pairing::{PairingCode, PairingState, PairingUserInterface},
    LibP2PPairingProtocol, NetworkError, Result,
};
use async_trait::async_trait;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// UI implementation for subprocess pairing
struct SubprocessPairingUI {
    device_name: String,
}

#[async_trait]
impl PairingUserInterface for SubprocessPairingUI {
    async fn show_pairing_error(&self, error: &NetworkError) {
        println!("❌ Pairing error: {}", error);
    }

    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
        println!("PAIRING_CODE:{}", code);
        println!("EXPIRES_IN:{}", expires_in_seconds);
        println!("📋 Pairing code generated for {}", self.device_name);
    }

    async fn prompt_pairing_code(&self) -> Result<[String; 12]> {
        Err(NetworkError::AuthenticationFailed(
            "Not supported in subprocess mode".to_string(),
        ))
    }

    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool> {
        println!("CONFIRM_PAIRING:{}", remote_device.device_name);
        println!("✅ Auto-accepting pairing with {}", remote_device.device_name);
        Ok(true) // Auto-accept for testing
    }

    async fn show_pairing_progress(&self, state: PairingState) {
        match state {
            PairingState::GeneratingCode => println!("🔐 Generating pairing code..."),
            PairingState::Broadcasting => println!("📡 Broadcasting on LibP2P DHT..."),
            PairingState::Scanning => println!("🔍 Scanning LibP2P DHT for devices..."),
            PairingState::Connecting => println!("🔗 Establishing LibP2P connection..."),
            PairingState::Authenticating => println!("🔐 Authenticating via LibP2P..."),
            PairingState::ExchangingKeys => println!("🔄 Exchanging device information..."),
            PairingState::AwaitingConfirmation => println!("⏳ Awaiting confirmation..."),
            PairingState::EstablishingSession => println!("🔑 Establishing session keys..."),
            PairingState::Completed => println!("✅ LibP2P pairing completed!"),
            PairingState::Failed(err) => println!("❌ Pairing failed: {}", err),
            _ => {}
        }
    }
}

/// Run REAL LibP2P pairing protocol as initiator
async fn run_libp2p_initiator_protocol(
    core: &Core,
    pairing_code: &str,
    password: &str,
) -> Result<()> {
    println!("🔗 Starting REAL LibP2P pairing protocol as initiator...");
    
    // Get network identity from Core
    let networking = core.networking().ok_or_else(|| 
        NetworkError::NotInitialized("Networking not available".to_string()))?;
    let service = networking.read().await;
    
    // Get device info from device manager
    let device_id = core.device.device_id()
        .map_err(|e| NetworkError::AuthenticationFailed(format!("Device ID error: {}", e)))?;
    
    // Create a placeholder device info for now
    let device_name = format!("Initiator-{}", device_id.to_string().chars().take(8).collect::<String>());
    let private_key = PrivateKey::generate()?;
    let public_key = private_key.public_key();
    
    let device_info = DeviceInfo::new(device_id, device_name.clone(), public_key);

    // Create network identity for LibP2P
    let network_identity = NetworkIdentity::new_temporary(
        device_info.device_id,
        device_info.device_name.clone(),
        password,
    )?;

    // Use the private key we already created
    
    // Create UI interface
    let ui = Arc::new(SubprocessPairingUI {
        device_name: device_info.device_name.clone(),
    });
    
    println!("🔧 Initializing LibP2P pairing protocol...");
    
    // Create LibP2P pairing protocol
    let mut protocol = LibP2PPairingProtocol::new(
        &network_identity,
        device_info.clone(),
        private_key,
        password,
    ).await?;
    
    println!("🌐 Peer ID: {}", protocol.local_peer_id());
    
    // Start listening on LibP2P transports
    println!("📡 Starting LibP2P listeners...");
    let listening_addrs = protocol.start_listening().await?;
    println!("📡 Listening on addresses: {:?}", listening_addrs);
    
    // RUN THE ACTUAL PAIRING PROTOCOL
    println!("🤝 Running LibP2P pairing event loop as initiator...");
    match protocol.start_as_initiator(&*ui).await {
        Ok((remote_device, session_keys)) => {
            println!("✅ PAIRING SUCCESS!");
            println!("REMOTE_DEVICE:{}", remote_device.device_name);
            println!("SESSION_ESTABLISHED:true");
            
            // Register pairing with Core for persistence
            if let Err(e) = core.add_paired_device(remote_device, session_keys.into()).await {
                println!("⚠️ Warning: Failed to persist pairing: {}", e);
            }
            
            Ok(())
        }
        Err(e) => {
            println!("❌ LibP2P pairing failed: {}", e);
            Err(e)
        }
    }
}

/// Run REAL LibP2P pairing protocol as joiner
async fn run_libp2p_joiner_protocol(
    core: &Core,
    pairing_code: &str,
    password: &str,
) -> Result<()> {
    println!("🤝 Starting REAL LibP2P pairing protocol as joiner...");
    
    // Parse the pairing code
    let code_words: Vec<String> = pairing_code
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    
    if code_words.len() != 12 {
        return Err(NetworkError::AuthenticationFailed(
            "Invalid pairing code format - expected 12 words".to_string()
        ));
    }
    
    let words: [String; 12] = [
        code_words[0].clone(), code_words[1].clone(), code_words[2].clone(),
        code_words[3].clone(), code_words[4].clone(), code_words[5].clone(),
        code_words[6].clone(), code_words[7].clone(), code_words[8].clone(),
        code_words[9].clone(), code_words[10].clone(), code_words[11].clone(),
    ];
    
    let pairing_code_obj = PairingCode::from_words(&words)?;
    println!("✅ Parsed pairing code successfully");
    
    // Get device info from device manager
    let device_id = core.device.device_id()
        .map_err(|e| NetworkError::AuthenticationFailed(format!("Device ID error: {}", e)))?;
    
    // Create a placeholder device info for now
    let device_name = format!("Joiner-{}", device_id.to_string().chars().take(8).collect::<String>());
    let private_key = PrivateKey::generate()?;
    let public_key = private_key.public_key();
    
    let device_info = DeviceInfo::new(device_id, device_name.clone(), public_key);

    // Create network identity for LibP2P
    let network_identity = NetworkIdentity::new_temporary(
        device_info.device_id,
        device_info.device_name.clone(),
        password,
    )?;

    // Use the private key we already created
    
    // Create UI interface
    let ui = Arc::new(SubprocessPairingUI {
        device_name: device_info.device_name.clone(),
    });
    
    println!("🔧 Initializing LibP2P pairing protocol...");
    
    // Create LibP2P pairing protocol
    let mut protocol = LibP2PPairingProtocol::new(
        &network_identity,
        device_info.clone(),
        private_key,
        password,
    ).await?;
    
    println!("🌐 Peer ID: {}", protocol.local_peer_id());
    
    // Start listening on LibP2P transports
    println!("📡 Starting LibP2P listeners...");
    let listening_addrs = protocol.start_listening().await?;
    println!("📡 Listening on addresses: {:?}", listening_addrs);
    
    // RUN THE ACTUAL JOINER PROTOCOL
    println!("🔍 Discovering Alice via LibP2P DHT...");
    match protocol.start_as_joiner(&*ui, pairing_code_obj).await {
        Ok((remote_device, session_keys)) => {
            println!("✅ PAIRING SUCCESS!");
            println!("REMOTE_DEVICE:{}", remote_device.device_name);
            println!("SESSION_ESTABLISHED:true");
            
            // Register pairing with Core
            if let Err(e) = core.add_paired_device(remote_device, session_keys.into()).await {
                println!("⚠️ Warning: Failed to persist pairing: {}", e);
            }
            
            Ok(())
        }
        Err(e) => {
            println!("❌ LibP2P pairing failed: {}", e);
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see mDNS discovery
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_env_filter("sd_core_new::infrastructure::networking=debug,libp2p_mdns=debug")
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <role> <data_dir> <password> [pairing_code]", args[0]);
        eprintln!("Roles: initiator, joiner");
        std::process::exit(1);
    }

    let role = &args[1];
    let data_dir = std::path::PathBuf::from(&args[2]);
    let password = &args[3];

    // Create data directory
    std::fs::create_dir_all(&data_dir).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Initialize Core with minimal configuration
    let mut core = Core::new_with_config(data_dir.clone()).await?;
    core.init_networking(password).await?;
    core.start_networking().await?;

    // Give networking and mDNS time to start and discover peers
    println!("⏳ Waiting for networking and mDNS to initialize...");
    sleep(Duration::from_millis(3000)).await;

    match role.as_str() {
        "initiator" => {
            println!("🚀 Starting as pairing initiator...");
            
            // Step 1: Generate pairing code using Core API (this should be fast)
            let (pairing_code, expires_in) = core.start_pairing_as_initiator(true).await?;
            
            // Output in format expected by test
            println!("PAIRING_CODE:{}", pairing_code);
            println!("EXPIRES_IN:{}", expires_in);
            
            // Step 2: Now run the LibP2P protocol to actually listen for connections
            println!("🔗 Starting LibP2P protocol to listen for connections...");
            if let Err(e) = run_libp2p_initiator_protocol(&core, &pairing_code, password).await {
                eprintln!("❌ LibP2P pairing failed: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
            
            println!("STATUS:SUCCESS");
            println!("✅ Pairing completed as initiator");
        }
        "joiner" => {
            if args.len() < 5 {
                eprintln!("Joiner role requires pairing code");
                std::process::exit(1);
            }
            
            let pairing_code = &args[4];
            println!("🤝 Starting as pairing joiner with code: {}...", 
                     pairing_code.split_whitespace().take(3).collect::<Vec<_>>().join(" "));
            
            // Run the REAL LibP2P joiner protocol
            if let Err(e) = run_libp2p_joiner_protocol(&core, pairing_code, password).await {
                eprintln!("❌ LibP2P pairing failed: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
            
            println!("STATUS:SUCCESS");
            println!("✅ Pairing completed as joiner");
        }
        _ => {
            eprintln!("Invalid role: {}. Use 'initiator' or 'joiner'", role);
            std::process::exit(1);
        }
    }

    // Keep process alive briefly to ensure pairing completes
    sleep(Duration::from_secs(2)).await;
    
    core.shutdown().await?;
    std::result::Result::Ok(())
}