//! Test device persistence and automatic reconnection after core restart
//!
//! This test verifies that:
//! 1. Devices can pair successfully
//! 2. Pairing information is persisted to disk
//! 3. After both devices restart, they automatically reconnect
//! 4. The reconnection happens without manual intervention

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Alice's device persistence scenario - handles both initial pairing and restart
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_persistence_scenario() {
	let role = env::var("TEST_ROLE").unwrap_or_default();
	if !role.starts_with("alice") {
		return;
	}

	let data_dir = PathBuf::from("/tmp/spacedrive-persistence-test/alice");
	let device_name = "Alice's Persistent Device";

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-persistence-test");

	// Determine which phase we're in
	let is_restart = role == "alice_restart";

	if is_restart {
		println!("Alice: RESTART PHASE - Testing automatic reconnection");
		println!("Alice: Data dir: {:?}", data_dir);

		// Initialize Core - this should load persisted devices
		println!("Alice: Initializing Core after restart...");
		let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
			.await
			.unwrap()
			.unwrap();
		println!("Alice: Core initialized successfully");

		// Device name should be persisted
		let device_config = core.device.config().unwrap();
		let current_name = device_config.name;
		println!("Alice: Device name after restart: {}", current_name);
		assert_eq!(current_name, device_name, "Device name not persisted");

		// Initialize networking - this should trigger auto-reconnection
		println!("Alice: Initializing networking (should auto-reconnect)...");
		timeout(Duration::from_secs(10), core.init_networking())
			.await
			.unwrap()
			.unwrap();

		// Give time for auto-reconnection to happen - discovery takes time
		tokio::time::sleep(Duration::from_secs(10)).await;
		println!("Alice: Networking initialized, checking for auto-reconnection...");

		// Check if Bob reconnected automatically
		println!("Alice: Waiting for automatic reconnection to Bob...");
		let mut attempts = 0;
		let max_attempts = 60; // 60 seconds - give more time for discovery

		loop {
			tokio::time::sleep(Duration::from_secs(1)).await;

			let connected_devices = core.get_connected_devices().await.unwrap();
			if !connected_devices.is_empty() {
				println!("Alice: Auto-reconnection successful!");
				println!(
					"Alice: Connected {} devices after restart",
					connected_devices.len()
				);

				// Verify it's Bob
				let device_info = core.get_connected_devices_info().await.unwrap();
				let bob_found = device_info.iter().any(|d| d.device_name.contains("Bob"));
				assert!(
					bob_found,
					"Bob not found in connected devices after restart"
				);

				for device in &device_info {
					println!(
						"Alice sees after restart: {} (ID: {})",
						device.device_name, device.device_id
					);
				}

				// Write success marker
				std::fs::write(
					"/tmp/spacedrive-persistence-test/alice_restart_success.txt",
					"success",
				)
				.unwrap();
				println!("Alice: Device persistence test completed successfully");
				break;
			}

			attempts += 1;
			if attempts >= max_attempts {
				panic!("Alice: Auto-reconnection timeout - Bob did not reconnect automatically");
			}

			if attempts % 5 == 0 {
				println!(
					"Alice: Auto-reconnection check {} - waiting for Bob",
					attempts / 5
				);
			}
		}
	} else {
		// Initial pairing phase
		println!("Alice: INITIAL PHASE - Starting pairing");
		println!("Alice: Data dir: {:?}", data_dir);

		// Initialize Core
		println!("Alice: Initializing Core...");
		let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
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

		println!(
			"Alice: Pairing code generated (expires in {}s)",
			expires_in
		);

		// Write pairing code for Bob
		std::fs::create_dir_all("/tmp/spacedrive-persistence-test").unwrap();
		std::fs::write(
			"/tmp/spacedrive-persistence-test/pairing_code.txt",
			&pairing_code,
		)
		.unwrap();

		// Wait for Bob to connect
		println!("Alice: Waiting for Bob to connect...");
		let mut attempts = 0;
		let max_attempts = 45;

		loop {
			tokio::time::sleep(Duration::from_secs(1)).await;

			let connected_devices = core.get_connected_devices().await.unwrap();
			if !connected_devices.is_empty() {
				println!("Alice: Initial pairing completed!");
				println!("Alice: Connected {} devices", connected_devices.len());

				// Verify devices are properly persisted
				if let Some(networking) = core.networking() {
					let registry = networking.device_registry();
					let paired_devices = registry.read().await.get_paired_devices();
					assert!(
						!paired_devices.is_empty(),
						"No paired devices found in registry"
					);
					println!(
						"Alice: {} devices persisted to registry",
						paired_devices.len()
					);
				}

				// Write success marker
				std::fs::write(
					"/tmp/spacedrive-persistence-test/alice_paired.txt",
					"success",
				)
				.unwrap();

				// Keep running for a bit to ensure persistence completes
				tokio::time::sleep(Duration::from_secs(3)).await;
				break;
			}

			attempts += 1;
			if attempts >= max_attempts {
				panic!("Alice: Initial pairing timeout");
			}
		}

		// Gracefully shutdown to ensure persistence
		println!("Alice: Shutting down gracefully to ensure persistence...");
		drop(core);
		tokio::time::sleep(Duration::from_secs(2)).await;
		println!("Alice: Initial phase completed");
	}
}

/// Bob's device persistence scenario - handles both initial pairing and restart
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_persistence_scenario() {
	let role = env::var("TEST_ROLE").unwrap_or_default();
	if !role.starts_with("bob") {
		return;
	}

	let data_dir = PathBuf::from("/tmp/spacedrive-persistence-test/bob");
	let device_name = "Bob's Persistent Device";

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-persistence-test");

	// Determine which phase we're in
	let is_restart = role == "bob_restart";

	if is_restart {
		println!("Bob: RESTART PHASE - Testing automatic reconnection");
		println!("Bob: Data dir: {:?}", data_dir);

		// Initialize Core - this should load persisted devices
		println!("Bob: Initializing Core after restart...");
		let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
			.await
			.unwrap()
			.unwrap();
		println!("Bob: Core initialized successfully");

		// Device name should be persisted
		let current_name = core.device.config().unwrap().name;
		println!("Bob: Device name after restart: {}", current_name);
		assert_eq!(current_name, device_name, "Device name not persisted");

		// Initialize networking - this should trigger auto-reconnection
		println!("Bob: Initializing networking (should auto-reconnect)...");
		timeout(Duration::from_secs(10), core.init_networking())
			.await
			.unwrap()
			.unwrap();

		// Give time for auto-reconnection to happen - discovery takes time
		tokio::time::sleep(Duration::from_secs(10)).await;
		println!("Bob: Networking initialized, checking for auto-reconnection...");

		// Check if Alice reconnected automatically
		println!("Bob: Waiting for automatic reconnection to Alice...");
		let mut attempts = 0;
		let max_attempts = 60; // 60 seconds - give more time for discovery

		loop {
			tokio::time::sleep(Duration::from_secs(1)).await;

			let connected_devices = core.get_connected_devices().await.unwrap();
			if !connected_devices.is_empty() {
				println!("Bob: Auto-reconnection successful!");
				println!(
					"Bob: Connected {} devices after restart",
					connected_devices.len()
				);

				// Verify it's Alice
				let device_info = core.get_connected_devices_info().await.unwrap();
				let alice_found = device_info.iter().any(|d| d.device_name.contains("Alice"));
				assert!(
					alice_found,
					"Alice not found in connected devices after restart"
				);

				for device in &device_info {
					println!(
						"Bob sees after restart: {} (ID: {})",
						device.device_name, device.device_id
					);
				}

				// Write success marker
				std::fs::write(
					"/tmp/spacedrive-persistence-test/bob_restart_success.txt",
					"success",
				)
				.unwrap();
				println!("Bob: Device persistence test completed successfully");
				break;
			}

			attempts += 1;
			if attempts >= max_attempts {
				panic!("Bob: Auto-reconnection timeout - Alice did not reconnect automatically");
			}

			if attempts % 5 == 0 {
				println!(
					"Bob: Auto-reconnection check {} - waiting for Alice",
					attempts / 5
				);
			}
		}
	} else {
		// Initial pairing phase
		println!("Bob: INITIAL PHASE - Starting pairing");
		println!("Bob: Data dir: {:?}", data_dir);

		// Initialize Core
		println!("Bob: Initializing Core...");
		let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
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

		tokio::time::sleep(Duration::from_secs(3)).await;
		println!("Bob: Networking initialized successfully");

		// Wait for pairing code from Alice
		println!("Bob: Looking for pairing code...");
		let pairing_code = loop {
			if let Ok(code) =
				std::fs::read_to_string("/tmp/spacedrive-persistence-test/pairing_code.txt")
			{
				break code.trim().to_string();
			}
			tokio::time::sleep(Duration::from_millis(500)).await;
		};
		println!("Bob: Found pairing code");

		// Join pairing session
		println!("Bob: Joining pairing session...");
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

		// Wait for connection
		println!("Bob: Waiting for connection to Alice...");
		let mut attempts = 0;
		let max_attempts = 30;

		loop {
			tokio::time::sleep(Duration::from_secs(1)).await;

			let connected_devices = core.get_connected_devices().await.unwrap();
			if !connected_devices.is_empty() {
				println!("Bob: Initial pairing completed!");
				println!("Bob: Connected {} devices", connected_devices.len());

				// Verify devices are properly persisted
				if let Some(networking) = core.networking() {
					let registry = networking.device_registry();
					let paired_devices = registry.read().await.get_paired_devices();
					assert!(
						!paired_devices.is_empty(),
						"No paired devices found in registry"
					);
					println!(
						"Bob: {} devices persisted to registry",
						paired_devices.len()
					);
				}

				// Write success marker
				std::fs::write("/tmp/spacedrive-persistence-test/bob_paired.txt", "success")
					.unwrap();

				// Keep running for a bit to ensure persistence completes
				tokio::time::sleep(Duration::from_secs(3)).await;
				break;
			}

			attempts += 1;
			if attempts >= max_attempts {
				panic!("Bob: Initial pairing timeout");
			}
		}

		// Gracefully shutdown to ensure persistence
		println!("Bob: Shutting down gracefully to ensure persistence...");
		drop(core);
		tokio::time::sleep(Duration::from_secs(2)).await;
		println!("Bob: Initial phase completed");
	}
}

/// Main test orchestrator - tests device persistence and auto-reconnection
#[tokio::test]
async fn test_device_persistence() {
	println!("Testing device persistence and automatic reconnection");

	// Clean up any previous test artifacts
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-persistence-test");
	std::fs::create_dir_all("/tmp/spacedrive-persistence-test").unwrap();

	let mut runner = CargoTestRunner::for_test_file("device_persistence_test")
		.with_timeout(Duration::from_secs(240)) // Longer timeout for restart test
		.add_subprocess("alice", "alice_persistence_scenario")
		.add_subprocess("alice_restart", "alice_persistence_scenario")
		.add_subprocess("bob", "bob_persistence_scenario")
		.add_subprocess("bob_restart", "bob_persistence_scenario");

	// Phase 1: Initial pairing
	println!("\\nPHASE 1: Initial pairing");
	println!("Starting Alice for initial pairing...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Wait for Alice to initialize
	tokio::time::sleep(Duration::from_secs(8)).await;

	println!("Starting Bob for initial pairing...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Wait for initial pairing to complete
	let pairing_result = runner
		.wait_for_success(|_| {
			let alice_paired =
				std::fs::read_to_string("/tmp/spacedrive-persistence-test/alice_paired.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_paired =
				std::fs::read_to_string("/tmp/spacedrive-persistence-test/bob_paired.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			alice_paired && bob_paired
		})
		.await;

	if pairing_result.is_err() {
		println!("Initial pairing failed");
		for (name, output) in runner.get_all_outputs() {
			println!("\\n{} output:\\n{}", name, output);
		}
		panic!("Initial pairing failed - cannot proceed with persistence test");
	}

	println!("Phase 1 complete: Devices paired successfully");

	// Wait a bit to ensure processes have fully shut down
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Phase 2: Restart both devices and verify auto-reconnection
	println!("\\nPHASE 2: Testing automatic reconnection after restart");

	// Clear the pairing code to ensure devices aren't re-pairing
	let _ = std::fs::remove_file("/tmp/spacedrive-persistence-test/pairing_code.txt");

	println!("Restarting Alice...");
	runner
		.spawn_single_process("alice_restart")
		.await
		.expect("Failed to spawn Alice restart");

	// Give Alice just a small head start
	tokio::time::sleep(Duration::from_secs(2)).await;

	println!("Restarting Bob...");
	runner
		.spawn_single_process("bob_restart")
		.await
		.expect("Failed to spawn Bob restart");

	// Give both devices time to fully start up and discover each other
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Wait for auto-reconnection
	let reconnection_result = runner
		.wait_for_success(|_| {
			let alice_reconnected = std::fs::read_to_string(
				"/tmp/spacedrive-persistence-test/alice_restart_success.txt",
			)
			.map(|content| content.trim() == "success")
			.unwrap_or(false);
			let bob_reconnected =
				std::fs::read_to_string("/tmp/spacedrive-persistence-test/bob_restart_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			alice_reconnected && bob_reconnected
		})
		.await;

	match reconnection_result {
		Ok(_) => {
			println!("\\nDevice persistence test successful!");
			println!("Devices automatically reconnected after restart");
		}
		Err(e) => {
			println!("\\nDevice persistence test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\\n{} output:\\n{}", name, output);
			}
			panic!("Devices did not automatically reconnect after restart");
		}
	}
}
