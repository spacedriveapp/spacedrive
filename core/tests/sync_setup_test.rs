//! Sync setup test using subprocess framework
//!
//! Tests that sync setup works without UNIQUE constraint errors when both devices
//! have the same deterministic default spaces.

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Alice's sync setup scenario
#[tokio::test]
#[ignore]
async fn alice_sync_setup_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-sync-setup-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-sync-setup-test/alice");

	println!("Alice: Starting sync setup test");

	// Initialize Core
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();

	core.device.set_name("Alice Device".to_string()).unwrap();

	// Initialize networking
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	tokio::time::sleep(Duration::from_secs(2)).await;
	println!("Alice: Core initialized");

	// Create library
	let library = core
		.libraries
		.create_library("Test Library".to_string(), None, core.context.clone())
		.await
		.unwrap();

	println!("Alice: Library created with ID: {}", library.id());

	// Write library ID for Bob
	std::fs::write(
		"/tmp/spacedrive-sync-setup-test/library_id.txt",
		library.id().to_string(),
	)
	.unwrap();

	// Write Alice's device ID for Bob
	std::fs::write(
		"/tmp/spacedrive-sync-setup-test/alice_device_id.txt",
		core.device.device_id().unwrap().to_string(),
	)
	.unwrap();

	// Start pairing
	let (pairing_code, _) = if let Some(networking) = core.networking() {
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

	println!("Alice: Pairing code generated");
	std::fs::write(
		"/tmp/spacedrive-sync-setup-test/pairing_code.txt",
		&pairing_code,
	)
	.unwrap();

	// Wait for pairing
	println!("Alice: Waiting for Bob to pair...");
	let mut attempts = 0;
	while attempts < 45 {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected = core.services.device.get_connected_devices().await.unwrap();
		if !connected.is_empty() {
			println!("Alice: Pairing successful!");

			// Share library with Bob - THIS IS THE CRITICAL TEST
			let bob_device_id = connected.first().unwrap().clone();
			println!(
				"Alice: Sharing library with Bob (device: {})...",
				bob_device_id
			);

			use sd_core::infra::action::CoreAction;
			use sd_core::ops::network::sync_setup::{
				LibrarySyncAction, LibrarySyncSetupAction, LibrarySyncSetupInput,
			};

			let input = LibrarySyncSetupInput {
				local_device_id: core.device.device_id().unwrap(),
				remote_device_id: bob_device_id,
				local_library_id: library.id(),
				remote_library_id: Some(library.id()),
				action: LibrarySyncAction::ShareLocalLibrary {
					library_name: "Test Library".to_string(),
				},
				leader_device_id: core.device.device_id().unwrap(),
			};

			let action = LibrarySyncSetupAction::from_input(input).unwrap();
			let result = action.execute(core.context.clone()).await;

			match result {
				Ok(_) => {
					println!("Alice: ✅ Share library SUCCEEDED!");
					std::fs::write(
						"/tmp/spacedrive-sync-setup-test/alice_success.txt",
						"success",
					)
					.unwrap();
				}
				Err(e) => {
					println!("Alice: ❌ Share library FAILED: {:?}", e);
					std::fs::write(
						"/tmp/spacedrive-sync-setup-test/alice_error.txt",
						format!("{:?}", e),
					)
					.unwrap();
					panic!("Alice: Share library failed: {:?}", e);
				}
			}

			std::fs::write(
				"/tmp/spacedrive-sync-setup-test/alice_paired.txt",
				"success",
			)
			.unwrap();

			// Give Bob time to process
			tokio::time::sleep(Duration::from_secs(5)).await;
			break;
		}

		attempts += 1;
	}

	if attempts >= 45 {
		panic!("Alice: Pairing timeout");
	}

	println!("Alice: Test completed");
}

/// Bob's sync setup scenario
#[tokio::test]
#[ignore]
async fn bob_sync_setup_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-sync-setup-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-sync-setup-test/bob");

	println!("Bob: Starting sync setup test");

	// Initialize Core
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();

	core.device.set_name("Bob Device".to_string()).unwrap();

	// Initialize networking
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	tokio::time::sleep(Duration::from_secs(2)).await;
	println!("Bob: Core initialized");

	// Wait for Alice's library ID
	println!("Bob: Waiting for Alice's library ID...");
	let library_id = loop {
		if let Ok(id) = std::fs::read_to_string("/tmp/spacedrive-sync-setup-test/library_id.txt") {
			break id.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Found library ID: {}", library_id);

	// Wait for pairing code
	println!("Bob: Waiting for pairing code...");
	let pairing_code = loop {
		if let Ok(code) =
			std::fs::read_to_string("/tmp/spacedrive-sync-setup-test/pairing_code.txt")
		{
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};

	// Join pairing
	println!("Bob: Joining pairing...");
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
	}

	// Wait for pairing completion
	println!("Bob: Waiting for pairing to complete...");
	let mut attempts = 0;
	while attempts < 30 {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected = core.services.device.get_connected_devices().await.unwrap();
		if !connected.is_empty() {
			println!("Bob: Pairing successful!");

			// Wait for Alice to share her library (ShareLocalLibrary creates it on Bob's side)
			println!("Bob: Waiting for Alice's ShareLocalLibrary to create library...");

			let alice_lib_uuid = uuid::Uuid::parse_str(&library_id).unwrap();
			let mut lib_wait_attempts = 0;

			while lib_wait_attempts < 30 {
				tokio::time::sleep(Duration::from_secs(1)).await;

				// Check if library was created by Alice's ShareLocalLibrary action
				if let Some(lib) = core.libraries.get_library(alice_lib_uuid).await {
					println!("Bob: ✅ Library received from Alice! ID: {}", lib.id());
					std::fs::write("/tmp/spacedrive-sync-setup-test/bob_success.txt", "success")
						.unwrap();

					// Verify sync initialized
					tokio::time::sleep(Duration::from_secs(2)).await;
					break;
				}

				lib_wait_attempts += 1;
			}

			if lib_wait_attempts >= 30 {
				println!("Bob: ❌ Library was never created - UNIQUE constraint may have failed");
				std::fs::write(
					"/tmp/spacedrive-sync-setup-test/bob_error.txt",
					"Timeout waiting for library from Alice - ShareLocalLibrary may have failed with UNIQUE constraint",
				)
				.unwrap();
				panic!("Bob: Timeout waiting for library");
			}

			break;
		}

		attempts += 1;
	}

	if attempts >= 30 {
		panic!("Bob: Pairing timeout");
	}

	println!("Bob: Test completed");
}

/// Main test orchestrator
#[tokio::test]
async fn test_sync_setup_no_constraint_error() {
	println!("Testing sync setup with deterministic spaces...");

	// Clean up
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-sync-setup-test");
	std::fs::create_dir_all("/tmp/spacedrive-sync-setup-test").unwrap();

	let mut runner = CargoTestRunner::for_test_file("sync_setup_test")
		.with_timeout(Duration::from_secs(120))
		.add_subprocess("alice", "alice_sync_setup_scenario")
		.add_subprocess("bob", "bob_sync_setup_scenario");

	// Spawn Alice first
	println!("Starting Alice...");
	runner.spawn_single_process("alice").await.unwrap();

	// Wait for Alice to initialize
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Start Bob
	println!("Starting Bob...");
	runner.spawn_single_process("bob").await.unwrap();

	// Wait for success markers
	let result = runner
		.wait_for_success(|_| {
			let alice_paired =
				std::fs::read_to_string("/tmp/spacedrive-sync-setup-test/alice_paired.txt")
					.map(|c| c.trim() == "success")
					.unwrap_or(false);

			let bob_success =
				std::fs::read_to_string("/tmp/spacedrive-sync-setup-test/bob_success.txt")
					.map(|c| c.trim() == "success")
					.unwrap_or(false);

			// Check if Bob had an error
			if std::path::Path::new("/tmp/spacedrive-sync-setup-test/bob_error.txt").exists() {
				let error =
					std::fs::read_to_string("/tmp/spacedrive-sync-setup-test/bob_error.txt")
						.unwrap();
				println!("Bob encountered error: {}", error);
				return false;
			}

			alice_paired && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!("✅ Sync setup test PASSED - no UNIQUE constraint errors!");
		}
		Err(e) => {
			println!("❌ Sync setup test FAILED: {}", e);

			// Print error if it exists
			if let Ok(error) =
				std::fs::read_to_string("/tmp/spacedrive-sync-setup-test/bob_error.txt")
			{
				println!("Bob's error: {}", error);
			}

			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Sync setup test failed");
		}
	}
}
