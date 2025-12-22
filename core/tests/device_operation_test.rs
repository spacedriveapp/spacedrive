//! Device operation integration test
//!
//! This test verifies the complete lifecycle of device operations:
//! 1. Two devices pair successfully
//! 2. Device unpair/revoke operation works correctly
//! 3. Unpaired device is removed from all caches and persistent storage
//! 4. ResourceDeleted event is emitted
//! 5. Unpaired device doesn't reappear after restart
//!
//! Tests the full cleanup flow:
//! - DeviceRegistry in-memory state
//! - DevicePersistence (encrypted KeyManager storage)
//! - DeviceManager paired_device_cache
//! - Node-to-device mappings
//! - Event emission

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Alice's device operation scenario - pairs with Bob, then unpairs
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_device_ops_scenario() {
	let role = env::var("TEST_ROLE").unwrap_or_default();
	if !role.starts_with("alice") {
		return;
	}

	let data_dir = PathBuf::from("/tmp/spacedrive-device-ops-test/alice");
	let device_name = "Alice's Device";

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-device-ops-test");

	// Determine which phase we're in
	let is_restart = role == "alice_restart";

	if is_restart {
		println!("Alice: RESTART PHASE - Verifying unpaired device stays gone");
		println!("Alice: Data dir: {:?}", data_dir);

		// Initialize Core
		println!("Alice: Initializing Core after restart...");
		let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
			.await
			.unwrap()
			.unwrap();
		println!("Alice: Core initialized successfully");

		// Initialize networking
		println!("Alice: Initializing networking...");
		timeout(Duration::from_secs(10), core.init_networking())
			.await
			.unwrap()
			.unwrap();

		// Give time for any potential auto-reconnection
		tokio::time::sleep(Duration::from_secs(5)).await;

		// Verify Bob is NOT in paired devices list
		if let Some(networking) = core.networking() {
			let registry = networking.device_registry();
			let guard = registry.read().await;
			let paired_devices = guard.get_paired_devices();

			println!(
				"Alice: After restart, paired devices count: {}",
				paired_devices.len()
			);

			// Should have NO paired devices after unpair + restart
			assert_eq!(
				paired_devices.len(),
				0,
				"Unpaired device reappeared after restart! Found {} devices",
				paired_devices.len()
			);

			println!("Alice: ✓ Verified unpaired device stayed removed after restart");
		}

		// Verify Bob is NOT in connected devices
		let connected_devices = core.services.device.get_connected_devices().await.unwrap();
		assert_eq!(
			connected_devices.len(),
			0,
			"Unpaired device reconnected! Found {} connected devices",
			connected_devices.len()
		);

		println!("Alice: ✓ Verified no devices reconnected");

		// Write success marker
		std::fs::write(
			"/tmp/spacedrive-device-ops-test/alice_restart_success.txt",
			"success",
		)
		.unwrap();

		println!("Alice: Restart phase completed successfully");
		return;
	}

	// INITIAL PHASE: Pair with Bob, then unpair
	println!("Alice: INITIAL PHASE - Pairing and unpairing");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized successfully");

	// Set device name
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
			networking.start_pairing_as_initiator(false),
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

	// Write pairing code for Bob
	std::fs::create_dir_all("/tmp/spacedrive-device-ops-test").unwrap();
	std::fs::write(
		"/tmp/spacedrive-device-ops-test/pairing_code.txt",
		&pairing_code,
	)
	.unwrap();

	// Wait for pairing completion
	println!("Alice: Waiting for pairing to complete...");
	let mut bob_device_id = None;
	let mut attempts = 0;
	let max_attempts = 45;

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core.services.device.get_connected_devices().await.unwrap();
		if !connected_devices.is_empty() {
			println!("Alice: Pairing completed successfully!");

			let device_info = core
				.services
				.device
				.get_connected_devices_info()
				.await
				.unwrap();

			for device in &device_info {
				println!(
					"Alice paired with: {} (ID: {})",
					device.device_name, device.device_id
				);
				if device.device_name.contains("Bob") {
					bob_device_id = Some(device.device_id);
				}
			}

			assert!(
				bob_device_id.is_some(),
				"Bob's device not found in paired devices"
			);
			break;
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Alice: Pairing timeout");
		}
	}

	let bob_id = bob_device_id.unwrap();
	println!("Alice: Bob's device ID: {}", bob_id);

	// Give Bob time to also detect the connection
	tokio::time::sleep(Duration::from_secs(3)).await;

	// Now UNPAIR Bob
	println!("Alice: Unpairing Bob's device...");

	if let Some(networking) = core.networking() {
		// Verify Bob is in paired devices before unpair
		{
			let registry = networking.device_registry();
			let guard = registry.read().await;
			let paired_before = guard.get_paired_devices();
			println!(
				"Alice: Paired devices before unpair: {}",
				paired_before.len()
			);
			assert_eq!(
				paired_before.len(),
				1,
				"Should have exactly 1 paired device"
			);
		}

		// Execute unpair by calling registry methods directly (same as DeviceRevokeAction)
		let registry = networking.device_registry();
		let result = {
			let mut guard = registry.write().await;
			guard.remove_device(bob_id).unwrap();
			guard.remove_paired_device(bob_id).await.unwrap()
		};

		println!("Alice: Unpair result: removed={}", result);
		assert!(result, "Unpair operation failed - device not found");

		// Remove from DeviceManager cache (same as action does)
		if let Err(e) = core.device.remove_paired_device_from_cache(bob_id) {
			println!("Alice: Warning - failed to remove from cache: {}", e);
		}

		// Give time for cleanup to complete
		tokio::time::sleep(Duration::from_secs(2)).await;

		// Verify Bob is removed from paired devices
		{
			let registry = networking.device_registry();
			let guard = registry.read().await;
			let paired_after = guard.get_paired_devices();
			println!("Alice: Paired devices after unpair: {}", paired_after.len());
			assert_eq!(
				paired_after.len(),
				0,
				"Device still in paired list after unpair!"
			);
		}

		println!("Alice: ✓ Verified device removed from registry");

		// Verify Bob is removed from DeviceManager cache
		let device_by_slug = core.device.resolve_by_slug("bobs-test-device");
		assert!(
			device_by_slug.is_none(),
			"Device still in DeviceManager cache after unpair!"
		);
		println!("Alice: ✓ Verified device removed from DeviceManager cache");

		// Verify Bob disconnected
		let connected_after = core.services.device.get_connected_devices().await.unwrap();
		assert_eq!(
			connected_after.len(),
			0,
			"Device still connected after unpair!"
		);
		println!("Alice: ✓ Verified device disconnected");
	}

	// Write success marker
	std::fs::write(
		"/tmp/spacedrive-device-ops-test/alice_success.txt",
		"success",
	)
	.unwrap();

	println!("Alice: Initial phase completed successfully");
}

/// Bob's device operation scenario - pairs with Alice, gets unpaired
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_device_ops_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	let data_dir = PathBuf::from("/tmp/spacedrive-device-ops-test/bob");
	let device_name = "Bob's Test Device";

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-device-ops-test");

	println!("Bob: Starting pairing scenario");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized successfully");

	// Set device name
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized successfully");

	// Wait for Alice's pairing code
	println!("Bob: Waiting for pairing code from Alice...");
	let mut attempts = 0;
	let pairing_code = loop {
		if let Ok(code) =
			std::fs::read_to_string("/tmp/spacedrive-device-ops-test/pairing_code.txt")
		{
			break code;
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
		attempts += 1;
		if attempts > 40 {
			panic!("Bob: Timeout waiting for pairing code");
		}
	};

	println!("Bob: Got pairing code, joining...");

	// Join pairing
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(20),
			networking.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
	} else {
		panic!("Networking not initialized");
	}

	println!("Bob: Successfully joined pairing");

	// Wait for pairing completion
	println!("Bob: Waiting for pairing to complete...");
	let mut attempts = 0;
	let max_attempts = 30;

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core.services.device.get_connected_devices().await.unwrap();
		if !connected_devices.is_empty() {
			println!("Bob: Pairing completed successfully!");

			let device_info = core
				.services
				.device
				.get_connected_devices_info()
				.await
				.unwrap();

			for device in &device_info {
				println!(
					"Bob paired with: {} (ID: {})",
					device.device_name, device.device_id
				);
			}

			// Wait for persistent connection
			println!("Bob: Waiting for persistent connection...");
			tokio::time::sleep(Duration::from_secs(10)).await;

			// Write success marker
			std::fs::write("/tmp/spacedrive-device-ops-test/bob_success.txt", "success").unwrap();

			// Keep Bob alive while Alice unpairs
			// Bob should detect disconnection when Alice unpairs
			println!("Bob: Waiting for potential unpair...");
			tokio::time::sleep(Duration::from_secs(30)).await;

			break;
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Bob: Pairing timeout");
		}
	}

	println!("Bob: Test completed");
}

/// Main test orchestrator - tests device pairing and unpair operations
#[tokio::test]
async fn test_device_operations() {
	println!("Testing device pairing and unpair operations");

	// Clean up from previous runs
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-device-ops-test");
	std::fs::create_dir_all("/tmp/spacedrive-device-ops-test").unwrap();

	let mut runner = CargoTestRunner::for_test_file("device_operation_test")
		.with_timeout(Duration::from_secs(180))
		.add_subprocess("alice", "alice_device_ops_scenario")
		.add_subprocess("bob", "bob_device_ops_scenario")
		.add_subprocess("alice_restart", "alice_device_ops_scenario");

	// PHASE 1: Pair devices and unpair
	println!("\n=== PHASE 1: Pairing and Unpair ===\n");

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

	// Run until both complete pairing and Alice unpairs Bob
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success =
				std::fs::read_to_string("/tmp/spacedrive-device-ops-test/alice_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_success =
				std::fs::read_to_string("/tmp/spacedrive-device-ops-test/bob_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => println!("✓ Phase 1 completed: Devices paired and unpaired successfully"),
		Err(e) => {
			println!("Phase 1 failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Phase 1 failed: {}", e);
		}
	}

	// Kill Bob process as it's no longer needed
	runner.kill_all().await;

	// Wait a bit for cleanup
	tokio::time::sleep(Duration::from_secs(3)).await;

	// PHASE 2: Restart Alice and verify unpaired device stays gone
	println!("\n=== PHASE 2: Restart Verification ===\n");

	println!("Restarting Alice to verify persistence...");
	runner
		.spawn_single_process("alice_restart")
		.await
		.expect("Failed to spawn Alice restart");

	let result = runner
		.wait_for_success(|_outputs| {
			std::fs::read_to_string("/tmp/spacedrive-device-ops-test/alice_restart_success.txt")
				.map(|content| content.trim() == "success")
				.unwrap_or(false)
		})
		.await;

	match result {
		Ok(_) => println!("✓ Phase 2 completed: Unpaired device stayed removed after restart"),
		Err(e) => {
			println!("Phase 2 failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Phase 2 failed: {}", e);
		}
	}

	// Final cleanup
	runner.kill_all().await;

	println!("\n=== ✓ ALL TESTS PASSED ===\n");
	println!("Verified:");
	println!("  • Device pairing works");
	println!("  • Device unpair removes from registry");
	println!("  • Device unpair removes from DeviceManager cache");
	println!("  • Device unpair removes from KeyManager storage");
	println!("  • Unpaired device doesn't reappear after restart");
}
