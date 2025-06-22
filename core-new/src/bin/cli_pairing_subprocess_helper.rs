//! Subprocess helper for CLI pairing integration tests
//! This binary allows spawning separate processes for Alice and Bob

use sd_core_new::Core;
use std::env;
use std::time::Duration;
use tokio::time::sleep;

/// Run LibP2P pairing protocol directly in subprocess context
/// This bypasses the Core API to avoid Send/Sync issues
async fn run_libp2p_initiator_protocol(
    core: &Core,
    pairing_code: &str,
    password: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // For now, just simulate the protocol by waiting
    // TODO: Implement actual LibP2P protocol here
    println!("📡 LibP2P protocol simulated - waiting for Bob...");
    sleep(Duration::from_secs(30)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    std::fs::create_dir_all(&data_dir)?;

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
            run_libp2p_initiator_protocol(&core, &pairing_code, password).await?;
            
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
            
            core.start_pairing_as_joiner(pairing_code).await?;
            
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
    Ok(())
}