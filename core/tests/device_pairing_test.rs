//! Core pairing test using the new cargo test subprocess framework
//!
//! This test demonstrates the new approach where ALL test logic remains in the test file
//! while still supporting subprocess-based testing for multi-device scenarios.

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Alice's pairing scenario - ALL logic stays in this test file!
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_pairing_scenario() {
	// Exit early if not running as Alice
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-pairing-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-pairing-test/alice");
	let device_name = "Alice's Test Device";

	println!("Alice: Starting Core pairing test");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized successfully");

	// Set device name
	println!("🏷️ Alice: Setting device name for testing...");
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Wait longer for networking to fully initialize and detect external addresses
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Alice: Networking initialized successfully");

	// Start pairing as initiator
	println!("Alice: Starting pairing as initiator...");
	let (pairing_code, expires_in) = if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_initiator(),
		)
		.await
		.unwrap()
		.unwrap()
	} else {
		panic!("Networking not initialized");
	};

	let short_code = pairing_code
		.split_whitespace()
		.take(3)
		.collect::<Vec<_>>()
		.join(" ");
	println!(
		"Alice: Pairing code generated: {}... (expires in {}s)",
		short_code, expires_in
	);

	// Write pairing code to shared location for Bob to read
	std::fs::create_dir_all("/tmp/spacedrive-pairing-test").unwrap();
	std::fs::write(
		"/tmp/spacedrive-pairing-test/pairing_code.txt",
		&pairing_code,
	)
	.unwrap();
	println!("Alice: Pairing code written to /tmp/spacedrive-pairing-test/pairing_code.txt");

	// Wait for pairing completion (Alice waits for Bob to connect)
	println!("⏳ Alice: Waiting for pairing to complete...");
	let mut attempts = 0;
	let max_attempts = 45; // 45 seconds

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core.get_connected_devices().await.unwrap();
		if !connected_devices.is_empty() {
			println!("Alice: Pairing completed successfully!");
			println!("Alice: Checking connected devices...");
			println!("Alice: Connected {} devices", connected_devices.len());

			// Get detailed device info
			let device_info = core.get_connected_devices_info().await.unwrap();
			for device in &device_info {
				println!(
					"Alice sees: {} (ID: {}, OS: {}, App: {})",
					device.device_name, device.device_id, device.os_version, device.app_version
				);
			}

			println!("PAIRING_SUCCESS: Alice's Test Device connected to Bob successfully");

			// Write success marker for orchestrator to detect
			std::fs::write("/tmp/spacedrive-pairing-test/alice_success.txt", "success").unwrap();

			// Wait a bit longer to give Bob time to detect the connection before Alice exits
			println!("⏳ Alice: Waiting for Bob to also detect the connection...");
			tokio::time::sleep(Duration::from_secs(5)).await;
			break;
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Alice: Pairing timeout - no devices connected");
		}

		if attempts % 5 == 0 {
			println!("Alice: Pairing status check {} - waiting", attempts / 5);
		}
	}

	println!("Alice: Test completed");
}

/// Bob's pairing scenario - ALL logic stays in this test file!
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_pairing_scenario() {
	// Exit early if not running as Bob
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-pairing-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-pairing-test/bob");
	let device_name = "Bob's Test Device";

	println!("Bob: Starting Core pairing test");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized successfully");

	// Set device name
	println!("🏷️ Bob: Setting device name for testing...");
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Wait longer for networking to fully initialize and detect external addresses
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized successfully");

	// Wait for initiator to create pairing code
	println!("Bob: Looking for pairing code...");
	let pairing_code = loop {
		if let Ok(code) = std::fs::read_to_string("/tmp/spacedrive-pairing-test/pairing_code.txt") {
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Found pairing code");

	// Join pairing session
	println!("Bob: Joining pairing with code...");
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code),
		)
		.await
		.unwrap()
		.unwrap();
	} else {
		panic!("Networking not initialized");
	}
	println!("Bob: Successfully joined pairing");

	// Wait for pairing completion
	println!("⏳ Bob: Waiting for pairing to complete...");
	let mut attempts = 0;
	let max_attempts = 30; // 30 seconds

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		// Check pairing status by looking at connected devices
		let connected_devices = core.get_connected_devices().await.unwrap();
		if !connected_devices.is_empty() {
			println!("Bob: Pairing completed successfully!");
			println!("Bob: Checking connected devices...");
			println!("Bob: Connected {} devices", connected_devices.len());

			// Get detailed device info
			let device_info = core.get_connected_devices_info().await.unwrap();
			for device in &device_info {
				println!(
					"Bob sees: {} (ID: {}, OS: {}, App: {})",
					device.device_name, device.device_id, device.os_version, device.app_version
				);
			}

			println!("PAIRING_SUCCESS: Bob's Test Device connected to Alice successfully");

			// Write success marker for orchestrator to detect
			std::fs::write("/tmp/spacedrive-pairing-test/bob_success.txt", "success").unwrap();
			break;
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Bob: Pairing timeout - no devices connected");
		}

		if attempts % 5 == 0 {
			println!("Bob: Pairing status check {} - waiting", attempts / 5);
		}
	}

	println!("Bob: Test completed");
}

/// Main test orchestrator - spawns cargo test subprocesses
#[tokio::test]
async fn test_device_pairing() {
	const PAIRING_CODE_PATH: &str = "/tmp/spacedrive-pairing-test/pairing_code.txt";

	// Clean up stale pairing code file from previous test runs
	// This prevents Bob from reading old data and fixes the file I/O race condition
	if std::path::Path::new(PAIRING_CODE_PATH).exists() {
		let _ = std::fs::remove_file(PAIRING_CODE_PATH);
		println!("Cleaned up stale pairing code file");
	}
	println!("Testing Core pairing with cargo test subprocess framework");

	// Clean up any old pairing files to avoid race conditions
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-pairing-test");
	std::fs::create_dir_all("/tmp/spacedrive-pairing-test").unwrap();

	let mut runner = CargoTestRunner::for_test_file("device_pairing_test")
		.with_timeout(Duration::from_secs(180))
		.add_subprocess("alice", "alice_pairing_scenario")
		.add_subprocess("bob", "bob_pairing_scenario");

	// Spawn Alice first
	println!("Starting Alice as initiator...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Wait for Alice to initialize and generate pairing code
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Start Bob as joiner
	println!("Starting Bob as joiner...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Run until both devices successfully pair using file markers
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success =
				std::fs::read_to_string("/tmp/spacedrive-pairing-test/alice_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_success =
				std::fs::read_to_string("/tmp/spacedrive-pairing-test/bob_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!(
				"Cargo test subprocess pairing test successful with mutual device recognition!"
			);
		}
		Err(e) => {
			println!("❌ Cargo test subprocess pairing test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\\n{} output:\\n{}", name, output);
			}
			panic!("Cargo test subprocess pairing test failed - devices did not properly recognize each other");
		}
	}
}
