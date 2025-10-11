//! Relay-only pairing test - verifies pairing works exclusively through Iroh relay
//!
//! This test forces both devices to connect only via relay (no direct/mDNS connections)
//! to ensure the relay fallback mechanism works correctly for cross-network pairing.

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Alice's relay-only pairing scenario - initiator
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_relay_only_pairing() {
	// Exit early if not running as Alice
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	// Set test directory
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-relay-only-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-relay-only-test/alice");
	let device_name = "Alice's Relay Test Device";

	println!("Alice: Starting RELAY-ONLY pairing test");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized successfully");

	// Set device name
	println!("Alice: Setting device name...");
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Wait for relay connection
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Alice: Networking initialized successfully");

	// Start pairing as initiator with FORCE_RELAY = true
	println!("Alice: Starting pairing as initiator (FORCE RELAY MODE)...");
	let (pairing_code, expires_in) = if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_initiator(true), // FORCE RELAY!
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
	println!("Alice: RELAY-ONLY MODE - Direct connections disabled");

	// Get the full PairingCode object (with NodeId and relay URL) from the networking service
	// This is the same object that was created internally with the correct session_id
	let networking = core.networking().expect("Networking not initialized");
	let pairing_code_obj = networking
		.get_pairing_code_for_current_session()
		.await
		.unwrap()
		.expect("Pairing code should exist");

	// Generate QR code JSON which preserves NodeId and relay URL for cross-network pairing
	let qr_json = pairing_code_obj.to_qr_json();
	println!(
		"Alice: Generated QR JSON with relay info (Session: {})",
		pairing_code_obj.session_id()
	);
	if let Some(node_id) = pairing_code_obj.node_id() {
		println!("Alice: NodeId in QR: {}", node_id.fmt_short());
	}
	if let Some(relay_url) = pairing_code_obj.relay_url() {
		println!("Alice: Relay URL in QR: {}", relay_url);
	}

	// Write QR JSON to shared location for Bob (contains NodeId + relay URL)
	std::fs::create_dir_all("/tmp/spacedrive-relay-only-test").unwrap();
	std::fs::write("/tmp/spacedrive-relay-only-test/pairing_qr.json", &qr_json).unwrap();
	println!("Alice: QR JSON written (includes NodeId and relay URL for relay discovery)");

	// Wait for pairing completion
	println!("Alice: Waiting for relay connection from Bob...");

	// Give the pairing protocol time to complete
	tokio::time::sleep(Duration::from_secs(2)).await;

	let mut attempts = 0;
	let max_attempts = 60; // 60 seconds for relay connection

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		// Check for paired devices (they may not be actively connected yet)
		if let Some(networking) = core.networking() {
			let device_registry = networking.device_registry();
			let registry = device_registry.read().await;
			let paired_devices = registry.get_paired_devices();

			if !paired_devices.is_empty() {
				println!("Alice: Pairing completed via RELAY!");
				println!("Alice: Paired with {} devices", paired_devices.len());

				// Verify pairing details
				for device in &paired_devices {
					println!("Alice:   - Device: {}", device.device_name);
					println!("Alice:   - ID: {}", device.device_id);
				}

				println!("Alice: Pairing test PASSED - relay connection successful!");

				// Write success marker for orchestrator
				std::fs::write(
					"/tmp/spacedrive-relay-only-test/alice_success.txt",
					"success",
				)
				.unwrap();
				println!("Alice: Success marker written");

				// Keep process alive longer to ensure test orchestrator sees success
				tokio::time::sleep(Duration::from_secs(15)).await;
				println!("Alice: Test complete, exiting");
				return;
			}
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Alice: Timeout waiting for relay pairing to complete");
		}

		if attempts % 10 == 0 {
			println!(
				"Alice: Still waiting for relay connection... ({}/{})",
				attempts, max_attempts
			);
		}
	}
}

/// Bob's relay-only pairing scenario - joiner
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_relay_only_pairing() {
	// Exit early if not running as Bob
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	// Set test directory
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-relay-only-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-relay-only-test/bob");
	let device_name = "Bob's Relay Test Device";

	println!("Bob: Starting RELAY-ONLY pairing test");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized successfully");

	// Set device name
	println!("Bob: Setting device name...");
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Wait for relay connection
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized successfully");

	// Wait for Alice's QR JSON with relay info
	println!("Bob: Looking for QR code JSON with relay info...");
	let qr_json = loop {
		if let Ok(json) = std::fs::read_to_string("/tmp/spacedrive-relay-only-test/pairing_qr.json")
		{
			break json.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Found QR code JSON");
	println!("Bob: QR JSON content: {}", qr_json);

	// Parse QR JSON to get PairingCode with NodeId and relay URL
	use sd_core::service::network::protocol::pairing::PairingCode;
	let pairing_code = PairingCode::from_qr_json(&qr_json).unwrap();
	println!("Bob: Parsed session_id: {}", pairing_code.session_id());
	println!(
		"Bob: Parsed QR code - has NodeId: {:?}",
		pairing_code.node_id().is_some()
	);
	if let Some(node_id) = pairing_code.node_id() {
		println!("Bob: Target NodeId: {}", node_id.fmt_short());
	}
	if let Some(relay_url) = pairing_code.relay_url() {
		println!("Bob: Target relay URL: {}", relay_url);
	}

	// Join pairing with FORCE_RELAY = true using the parsed PairingCode
	println!("Bob: Joining pairing session (FORCE RELAY MODE)...");
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(30), // Longer timeout for relay
			networking.start_pairing_as_joiner_with_code(pairing_code, true), // FORCE RELAY!
		)
		.await
		.unwrap()
		.unwrap();
	} else {
		panic!("Networking not initialized");
	}
	println!("Bob: Successfully joined pairing via RELAY!");
	println!("Bob: RELAY-ONLY MODE - Direct connections disabled");

	// Wait for pairing completion
	println!("Bob: Waiting for pairing to complete...");

	// Give the pairing protocol time to complete
	tokio::time::sleep(Duration::from_secs(2)).await;

	let mut attempts = 0;
	let max_attempts = 30; // 30 seconds

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		// Check for paired devices (they may not be actively connected yet)
		if let Some(networking) = core.networking() {
			let device_registry = networking.device_registry();
			let registry = device_registry.read().await;
			let paired_devices = registry.get_paired_devices();

			if !paired_devices.is_empty() {
				println!("Bob: Pairing completed successfully!");
				println!("Bob: Paired with {} devices", paired_devices.len());

				// Verify pairing details
				for device in &paired_devices {
					println!("Bob:   - Device: {}", device.device_name);
					println!("Bob:   - ID: {}", device.device_id);
				}

				println!("Bob: Pairing test PASSED - relay connection successful!");

				// Write success marker for orchestrator
				std::fs::write("/tmp/spacedrive-relay-only-test/bob_success.txt", "success")
					.unwrap();
				println!("Bob: Success marker written");

				// Keep the process running for a bit longer to let Alice detect the pairing
				println!("Bob: Keeping alive for Alice to detect pairing...");
				tokio::time::sleep(Duration::from_secs(15)).await;
				println!("Bob: Test complete, exiting");
				return;
			}
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Bob: Timeout waiting for pairing to complete");
		}

		if attempts % 5 == 0 {
			println!("Bob: Still waiting... ({}/{})", attempts, max_attempts);
		}
	}
}

/// Main test orchestrator - runs Alice and Bob in separate subprocesses
#[tokio::test]
async fn test_relay_only_pairing() {
	// Clean up any previous test data
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-relay-only-test");
	std::fs::create_dir_all("/tmp/spacedrive-relay-only-test").unwrap();

	println!("Starting RELAY-ONLY pairing integration test");
	println!("This test verifies pairing works exclusively through Iroh relay");
	println!("Direct and mDNS connections are disabled\n");

	let mut runner = CargoTestRunner::for_test_file("relay_only_pairing_test")
		.with_timeout(Duration::from_secs(90))
		.add_subprocess("alice", "alice_relay_only_pairing")
		.add_subprocess("bob", "bob_relay_only_pairing");

	// Spawn Alice first
	println!("Starting Alice as initiator...");
	runner.spawn_single_process("alice").await.unwrap();

	// Wait for Alice to initialize and generate QR code
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Start Bob as joiner
	println!("Starting Bob as joiner...");
	runner.spawn_single_process("bob").await.unwrap();

	// Wait for both to complete by checking success markers (don't call run_until_success as it spawns again!)
	let result = runner
		.wait_for_success(|_outputs| {
			// Check for success marker files
			let alice_done =
				std::path::Path::new("/tmp/spacedrive-relay-only-test/alice_success.txt").exists();
			let bob_done =
				std::path::Path::new("/tmp/spacedrive-relay-only-test/bob_success.txt").exists();

			println!("Pairing status: Alice={}, Bob={}", alice_done, bob_done);
			alice_done && bob_done
		})
		.await;

	assert!(
		result.is_ok(),
		"Relay-only pairing test failed: {:?}",
		result.err()
	);

	println!("\nRELAY-ONLY PAIRING TEST PASSED");
	println!("Both devices successfully paired using only the Iroh relay");
}
