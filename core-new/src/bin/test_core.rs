//! Generic test core binary
//! 
//! Single configurable binary that can run different Core scenarios
//! for multi-process testing. Replaces separate Alice/Bob binaries.

use clap::Parser;
use sd_core_new::test_framework::scenarios;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "test_core")]
#[command(about = "Generic Spacedrive core test runner")]
struct Args {
    /// Test scenario to run
    #[arg(long)]
    mode: String,
    
    /// Data directory for this instance
    #[arg(long)]
    data_dir: PathBuf,
    
    /// Device name for this instance
    #[arg(long)]
    device_name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Ensure data directory exists
    std::fs::create_dir_all(&args.data_dir)?;
    
    // Run the appropriate scenario
    match args.mode.as_str() {
        "initiator" => {
            scenarios::run_pairing_initiator(&args.data_dir, &args.device_name).await?;
        }
        "joiner" => {
            scenarios::run_pairing_joiner(&args.data_dir, &args.device_name).await?;
        }
        "peer" => {
            scenarios::run_peer_node(&args.data_dir, &args.device_name).await?;
        }
        "sync_server" => {
            scenarios::run_sync_server(&args.data_dir, &args.device_name).await?;
        }
        "sync_client" => {
            scenarios::run_sync_client(&args.data_dir, &args.device_name).await?;
        }
        "discovery" => {
            scenarios::run_discovery_test(&args.data_dir, &args.device_name).await?;
        }
        _ => {
            eprintln!("❌ Unknown mode: {}", args.mode);
            eprintln!("Available modes: initiator, joiner, peer, sync_server, sync_client, discovery");
            std::process::exit(1);
        }
    }
    
    Ok(())
}