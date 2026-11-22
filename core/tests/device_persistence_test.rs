//! Test device persistence and automatic reconnection after core restart
//!
//! This test verifies that:
//! 1. Devices can pair successfully
//! 2. Pairing information is persisted to disk
//! 3. After both devices restart, they automatically reconnect
//! 4. The reconnection happens without manual intervention
//!
//! The roles coordinate through marker files. In the first phase each role
//! writes `<role>_paired.txt` once it sees the other as a connected device
//! and stays up until the other's marker exists, so neither exit can cut a
//! pairing exchange the other side is still finishing. The orchestrator
//! waits for both processes to exit on their own before it restarts them
//! on the same data directories.

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

#[path = "helpers/wait.rs"]
mod wait;
use wait::{wait_for_connected_device, wait_for_file};

const TEST_DIR: &str = "/tmp/spacedrive-persistence-test";

fn marker(name: &str) -> String {
	format!("{TEST_DIR}/{name}")
}

/// Alice's device persistence scenario - handles both initial pairing and restart
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_persistence_scenario() {
	let role = env::var("TEST_ROLE").unwrap_or_default();
	if !role.starts_with("alice") {
		return;
	}

	let data_dir = PathBuf::from(marker("alice"));
	let device_name = "Alice's Persistent Device";

	if role == "alice_restart" {
		println!("Alice: RESTART PHASE - Testing automatic reconnection");
		println!("Alice: Data dir: {:?}", data_dir);

		println!("Alice: Initializing Core after restart...");
		let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
			.await
			.unwrap()
			.unwrap();
		println!("Alice: Core initialized successfully");

		let current_name = core.device.config().unwrap().name;
		println!("Alice: Device name after restart: {}", current_name);
		assert_eq!(current_name, device_name, "Device name not persisted");

		println!("Alice: Initializing networking (should auto-reconnect)...");
		timeout(Duration::from_secs(10), core.init_networking())
			.await
			.unwrap()
			.unwrap();

		println!("Alice: Waiting for automatic reconnection to Bob...");
		wait_for_connected_device(&core, Duration::from_secs(70))
			.await
			.expect("Alice: Auto-reconnection timeout - Bob did not reconnect automatically");
		let device_info = core
			.services
			.device
			.get_connected_devices_info()
			.await
			.unwrap();
		assert!(
			device_info.iter().any(|d| d.device_name.contains("Bob")),
			"Bob not found in connected devices after restart"
		);
		for device in &device_info {
			println!(
				"Alice sees after restart: {} (ID: {})",
				device.device_name, device.device_id
			);
		}
		println!("Alice: Auto-reconnection successful!");
		std::fs::write(marker("alice_restart_success.txt"), "success").unwrap();

		// Stay up until Bob has seen the reconnection too, so Alice's exit
		// cannot drop it before he checks.
		wait_for_file(marker("bob_restart_success.txt"), Duration::from_secs(60))
			.await
			.expect("Alice: Bob never reported the reconnection");
		println!("Alice: Device persistence test completed successfully");
		return;
	}

	println!("Alice: INITIAL PHASE - Starting pairing");
	println!("Alice: Data dir: {:?}", data_dir);

	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized successfully");

	println!("Alice: Setting device name...");
	core.device.set_name(device_name.to_string()).unwrap();

	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Networking initialized successfully");

	println!("Alice: Starting pairing as initiator...");
	let networking = core.networking().expect("Networking not initialized");
	let (pairing_code, expires_in) = timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_initiator(false),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Alice: Pairing code generated (expires in {}s)", expires_in);

	std::fs::create_dir_all(TEST_DIR).unwrap();
	std::fs::write(marker("pairing_code.txt"), &pairing_code).unwrap();

	println!("Alice: Waiting for Bob to connect...");
	wait_for_connected_device(&core, Duration::from_secs(45))
		.await
		.expect("Alice: Initial pairing timeout");
	println!("Alice: Initial pairing completed!");

	let paired_devices = networking
		.device_registry()
		.read()
		.await
		.get_paired_devices();
	assert!(
		!paired_devices.is_empty(),
		"No paired devices found in registry"
	);
	println!(
		"Alice: {} devices persisted to registry",
		paired_devices.len()
	);
	std::fs::write(marker("alice_paired.txt"), "success").unwrap();

	// Bob's marker says his side completed pairing too; exiting earlier
	// could cut the exchange while he is still persisting.
	wait_for_file(marker("bob_paired.txt"), Duration::from_secs(60))
		.await
		.expect("Alice: Bob never reported pairing");

	println!("Alice: Shutting down to test persistence...");
	core.shutdown().await.expect("shutdown");
	println!("Alice: Initial phase completed");
}

/// Bob's device persistence scenario - handles both initial pairing and restart
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_persistence_scenario() {
	let role = env::var("TEST_ROLE").unwrap_or_default();
	if !role.starts_with("bob") {
		return;
	}

	let data_dir = PathBuf::from(marker("bob"));
	let device_name = "Bob's Persistent Device";

	if role == "bob_restart" {
		println!("Bob: RESTART PHASE - Testing automatic reconnection");
		println!("Bob: Data dir: {:?}", data_dir);

		println!("Bob: Initializing Core after restart...");
		let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
			.await
			.unwrap()
			.unwrap();
		println!("Bob: Core initialized successfully");

		let current_name = core.device.config().unwrap().name;
		println!("Bob: Device name after restart: {}", current_name);
		assert_eq!(current_name, device_name, "Device name not persisted");

		println!("Bob: Initializing networking (should auto-reconnect)...");
		timeout(Duration::from_secs(10), core.init_networking())
			.await
			.unwrap()
			.unwrap();

		println!("Bob: Waiting for automatic reconnection to Alice...");
		wait_for_connected_device(&core, Duration::from_secs(70))
			.await
			.expect("Bob: Auto-reconnection timeout - Alice did not reconnect automatically");
		let device_info = core
			.services
			.device
			.get_connected_devices_info()
			.await
			.unwrap();
		assert!(
			device_info.iter().any(|d| d.device_name.contains("Alice")),
			"Alice not found in connected devices after restart"
		);
		for device in &device_info {
			println!(
				"Bob sees after restart: {} (ID: {})",
				device.device_name, device.device_id
			);
		}
		println!("Bob: Auto-reconnection successful!");
		std::fs::write(marker("bob_restart_success.txt"), "success").unwrap();

		wait_for_file(marker("alice_restart_success.txt"), Duration::from_secs(60))
			.await
			.expect("Bob: Alice never reported the reconnection");
		println!("Bob: Device persistence test completed successfully");
		return;
	}

	println!("Bob: INITIAL PHASE - Starting pairing");
	println!("Bob: Data dir: {:?}", data_dir);

	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized successfully");

	println!("Bob: Setting device name...");
	core.device.set_name(device_name.to_string()).unwrap();

	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Networking initialized successfully");

	println!("Bob: Looking for pairing code...");
	let pairing_code = wait_for_file(marker("pairing_code.txt"), Duration::from_secs(60))
		.await
		.expect("Bob: Alice never wrote a pairing code");
	println!("Bob: Found pairing code");

	println!("Bob: Joining pairing session...");
	let networking = core.networking().expect("Networking not initialized");
	timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_joiner(&pairing_code, false),
	)
	.await
	.unwrap()
	.unwrap();

	println!("Bob: Waiting for connection to Alice...");
	wait_for_connected_device(&core, Duration::from_secs(30))
		.await
		.expect("Bob: Initial pairing timeout");
	println!("Bob: Initial pairing completed!");

	let paired_devices = networking
		.device_registry()
		.read()
		.await
		.get_paired_devices();
	assert!(
		!paired_devices.is_empty(),
		"No paired devices found in registry"
	);
	println!(
		"Bob: {} devices persisted to registry",
		paired_devices.len()
	);
	std::fs::write(marker("bob_paired.txt"), "success").unwrap();

	wait_for_file(marker("alice_paired.txt"), Duration::from_secs(60))
		.await
		.expect("Bob: Alice never reported pairing");

	println!("Bob: Shutting down to test persistence...");
	core.shutdown().await.expect("shutdown");
	println!("Bob: Initial phase completed");
}

/// Main test orchestrator - tests device persistence and auto-reconnection
#[tokio::test]
async fn test_device_persistence() {
	println!("Testing device persistence and automatic reconnection");

	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	let mut runner = CargoTestRunner::for_test_file("device_persistence_test")
		.with_timeout(Duration::from_secs(240))
		.add_subprocess("alice", "alice_persistence_scenario")
		.add_subprocess("alice_restart", "alice_persistence_scenario")
		.add_subprocess("bob", "bob_persistence_scenario")
		.add_subprocess("bob_restart", "bob_persistence_scenario");

	println!("\nPHASE 1: Initial pairing");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");
	// Bob waits for the pairing code file himself.
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Both roles exit on their own once each has seen the other's marker;
	// their exit status is the phase's verdict and guarantees the data
	// directories are free before the restart.
	for name in ["alice", "bob"] {
		if let Err(e) = runner.wait_for_exit(name, Duration::from_secs(120)).await {
			runner.kill_all().await;
			panic!("Initial pairing failed: {e}");
		}
	}
	for name in ["alice_paired.txt", "bob_paired.txt"] {
		assert_eq!(
			std::fs::read_to_string(marker(name)).unwrap().trim(),
			"success",
			"{name} missing after phase 1"
		);
	}
	println!("Phase 1 complete: Devices paired successfully");

	println!("\nPHASE 2: Testing automatic reconnection after restart");

	// Clear the pairing code so the restarted devices cannot re-pair.
	let _ = std::fs::remove_file(marker("pairing_code.txt"));

	runner
		.spawn_single_process("alice_restart")
		.await
		.expect("Failed to spawn Alice restart");
	runner
		.spawn_single_process("bob_restart")
		.await
		.expect("Failed to spawn Bob restart");

	let reconnection_result = runner
		.wait_for_success(|_| {
			let alice_reconnected = std::fs::read_to_string(marker("alice_restart_success.txt"))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);
			let bob_reconnected = std::fs::read_to_string(marker("bob_restart_success.txt"))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);
			alice_reconnected && bob_reconnected
		})
		.await;

	match reconnection_result {
		Ok(_) => {
			println!("\nDevice persistence test successful!");
			println!("Devices automatically reconnected after restart");
		}
		Err(e) => {
			println!("\nDevice persistence test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Devices did not automatically reconnect after restart");
		}
	}
}
