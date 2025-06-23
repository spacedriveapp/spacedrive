//! Bob Core pairing test binary
//! Directly tests Core networking methods without CLI layer

use clap::Parser;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
	#[arg(long)]
	data_dir: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();

	println!("🟨 Bob: Starting Core pairing test");
	println!("📁 Bob: Data dir: {}", args.data_dir);

	// Initialize tracing for debug output
	// tracing_subscriber::fmt()
	//     .with_max_level(tracing::Level::DEBUG)
	//     .init();

	// Create Core instance
	println!("🔧 Bob: Initializing Core...");
	let mut core = match timeout(
		Duration::from_secs(10),
		sd_core_new::Core::new_with_config(std::path::PathBuf::from(&args.data_dir)),
	)
	.await
	{
		Ok(Ok(core)) => {
			println!("✅ Bob: Core initialized successfully");
			core
		}
		Ok(Err(e)) => {
			println!("❌ Bob: Core initialization failed: {}", e);
			return Err(e);
		}
		Err(_) => {
			println!("❌ Bob: Core initialization timed out");
			return Err("Core initialization timeout".into());
		}
	};

	// Initialize networking
	println!("🌐 Bob: Initializing networking...");
	match timeout(
		Duration::from_secs(10),
		core.init_networking("bob-password"),
	)
	.await
	{
		Ok(Ok(_)) => {
			println!("✅ Bob: Networking initialized successfully");
		}
		Ok(Err(e)) => {
			println!("❌ Bob: Networking initialization failed: {}", e);
			return Err(e);
		}
		Err(_) => {
			println!("❌ Bob: Networking initialization timed out");
			return Err("Networking initialization timeout".into());
		}
	}

	// Wait for Alice's pairing code
	println!("⏳ Bob: Waiting for Alice's pairing code...");
	let shared_dir = "/tmp/spacedrive-pairing-test";
	let code_file = format!("{}/pairing_code.txt", shared_dir);
	let pairing_code = loop {
		match std::fs::read_to_string(&code_file) {
			Ok(code) => {
				if !code.trim().is_empty() {
					println!(
						"✅ Bob: Found pairing code: {}...",
						code.trim()
							.split_whitespace()
							.take(3)
							.collect::<Vec<_>>()
							.join(" ")
					);
					break code.trim().to_string();
				}
			}
			Err(_) => {
				// File doesn't exist yet, keep waiting
			}
		}

		tokio::time::sleep(Duration::from_millis(100)).await;
	};

	// Join pairing using the code
	println!("🤝 Bob: Joining pairing with code...");
	match timeout(
		Duration::from_secs(15),
		core.start_pairing_as_joiner(&pairing_code),
	)
	.await
	{
		Ok(Ok(_)) => {
			println!("✅ Bob: Successfully joined pairing");
		}
		Ok(Err(e)) => {
			println!("❌ Bob: Failed to join pairing: {}", e);
			return Err(e);
		}
		Err(_) => {
			println!("❌ Bob: Pairing join timed out");
			return Err("Pairing join timeout".into());
		}
	}

	// Wait for pairing to complete
	println!("⏳ Bob: Waiting for pairing to complete...");
	let mut attempts = 0;
	let max_attempts = 20; // 20 seconds

	loop {
		if attempts >= max_attempts {
			println!("❌ Bob: Pairing timed out after {} seconds", max_attempts);
			return Err("Pairing timeout".into());
		}

		// Check pairing status
		match timeout(Duration::from_secs(3), core.get_pairing_status()).await {
			Ok(Ok(status)) => {
				println!(
					"🔍 Bob: Pairing status check {} - {} sessions",
					attempts + 1,
					status.len()
				);

				// Check if we have any completed pairings
				if !status.is_empty() {
					for session in &status {
						println!("📊 Bob: Session state: {:?}", session);
					}

					// Look for successful pairing
					if status.iter().any(|s| {
						matches!(
							s.state,
							sd_core_new::networking::PairingState::Completed { .. }
						)
					}) {
						println!("🎉 Bob: Pairing completed successfully!");
						break;
					}
				}
			}
			Ok(Err(e)) => {
				println!("⚠️ Bob: Pairing status check failed: {}", e);
			}
			Err(_) => {
				println!("⚠️ Bob: Pairing status check timed out");
			}
		}

		attempts += 1;
		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	// Check connected devices
	println!("🔗 Bob: Checking connected devices...");
	match timeout(Duration::from_secs(5), core.get_connected_devices()).await {
		Ok(Ok(devices)) => {
			println!("✅ Bob: Connected devices: {:?}", devices);
			if !devices.is_empty() {
				println!(
					"PAIRING_SUCCESS: Bob has {} connected devices",
					devices.len()
				);
			} else {
				println!("⚠️ Bob: No devices connected after pairing");
			}
		}
		Ok(Err(e)) => {
			println!("❌ Bob: Failed to get connected devices: {}", e);
		}
		Err(_) => {
			println!("❌ Bob: Get connected devices timed out");
		}
	}

	println!("🧹 Bob: Test completed");
	Ok(())
}
