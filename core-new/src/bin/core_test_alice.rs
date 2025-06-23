//! Alice Core pairing test binary
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

	println!("🟦 Alice: Starting Core pairing test");
	println!("📁 Alice: Data dir: {}", args.data_dir);

	// Initialize tracing for debug output
	// tracing_subscriber::fmt()
	//     .with_max_level(tracing::Level::DEBUG)
	//     .init();

	// Create Core instance
	println!("🔧 Alice: Initializing Core...");
	let mut core = match timeout(
		Duration::from_secs(10),
		sd_core_new::Core::new_with_config(std::path::PathBuf::from(&args.data_dir)),
	)
	.await
	{
		Ok(Ok(core)) => {
			println!("✅ Alice: Core initialized successfully");
			core
		}
		Ok(Err(e)) => {
			println!("❌ Alice: Core initialization failed: {}", e);
			return Err(e);
		}
		Err(_) => {
			println!("❌ Alice: Core initialization timed out");
			return Err("Core initialization timeout".into());
		}
	};

	// Initialize networking
	println!("🌐 Alice: Initializing networking...");
	match timeout(
		Duration::from_secs(10),
		core.init_networking("alice-password"),
	)
	.await
	{
		Ok(Ok(_)) => {
			println!("✅ Alice: Networking initialized successfully");
		}
		Ok(Err(e)) => {
			println!("❌ Alice: Networking initialization failed: {}", e);
			return Err(e);
		}
		Err(_) => {
			println!("❌ Alice: Networking initialization timed out");
			return Err("Networking initialization timeout".into());
		}
	}

	// Start pairing as initiator
	println!("🔑 Alice: Starting pairing as initiator...");
	let (pairing_code, expires_in) = match timeout(
		Duration::from_secs(15),
		core.start_pairing_as_initiator(true),
	)
	.await
	{
		Ok(Ok((code, expires))) => {
			println!(
				"✅ Alice: Pairing code generated: {}... (expires in {}s)",
				code.split_whitespace()
					.take(3)
					.collect::<Vec<_>>()
					.join(" "),
				expires
			);
			(code, expires)
		}
		Ok(Err(e)) => {
			println!("❌ Alice: Pairing code generation failed: {}", e);
			return Err(e);
		}
		Err(_) => {
			println!("❌ Alice: Pairing code generation timed out");
			return Err("Pairing code generation timeout".into());
		}
	};

	// Write pairing code to shared file for Bob to read
	let shared_dir = "/tmp/spacedrive-pairing-test";
	std::fs::create_dir_all(shared_dir).expect("Failed to create shared directory");
	let code_file = format!("{}/pairing_code.txt", shared_dir);
	match std::fs::write(&code_file, &pairing_code) {
		Ok(_) => {
			println!("📝 Alice: Pairing code written to {}", code_file);
		}
		Err(e) => {
			println!("❌ Alice: Failed to write pairing code: {}", e);
			return Err(e.into());
		}
	}

	// Wait for pairing to complete (Bob should join)
	println!("⏳ Alice: Waiting for pairing to complete...");
	let mut attempts = 0;
	let max_attempts = 30; // 30 seconds

	loop {
		if attempts >= max_attempts {
			println!("❌ Alice: Pairing timed out after {} seconds", max_attempts);
			return Err("Pairing timeout".into());
		}

		// Check pairing status
		match timeout(Duration::from_secs(3), core.get_pairing_status()).await {
			Ok(Ok(status)) => {
				println!(
					"🔍 Alice: Pairing status check {} - {} sessions",
					attempts + 1,
					status.len()
				);

				// Check if we have any completed pairings
				if !status.is_empty() {
					for session in &status {
						println!("📊 Alice: Session state: {:?}", session);
					}

					// Look for successful pairing
					if status.iter().any(|s| {
						matches!(
							s.state,
							sd_core_new::networking::PairingState::Completed { .. }
						)
					}) {
						println!("🎉 Alice: Pairing completed successfully!");
						break;
					}
				}
			}
			Ok(Err(e)) => {
				println!("⚠️ Alice: Pairing status check failed: {}", e);
			}
			Err(_) => {
				println!("⚠️ Alice: Pairing status check timed out");
			}
		}

		attempts += 1;
		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	// Check connected devices
	println!("🔗 Alice: Checking connected devices...");
	match timeout(Duration::from_secs(5), core.get_connected_devices()).await {
		Ok(Ok(devices)) => {
			println!("✅ Alice: Connected devices: {:?}", devices);
			if !devices.is_empty() {
				println!(
					"PAIRING_SUCCESS: Alice has {} connected devices",
					devices.len()
				);
			} else {
				println!("⚠️ Alice: No devices connected after pairing");
			}
		}
		Ok(Err(e)) => {
			println!("❌ Alice: Failed to get connected devices: {}", e);
		}
		Err(_) => {
			println!("❌ Alice: Get connected devices timed out");
		}
	}

	println!("🧹 Alice: Test completed");
	Ok(())
}
