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

#[path = "helpers/wait.rs"]
mod wait;
use wait::{wait_for_connected_device, wait_for_file, wait_until};

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
	let bob_device_id = wait_for_connected_device(&core, Duration::from_secs(45))
		.await
		.expect("Alice: Pairing timeout");
	println!("Alice: Pairing successful!");

	// Bob's marker says his registry holds Alice too, so the share request
	// cannot reach him before he trusts her.
	wait_for_file(
		"/tmp/spacedrive-sync-setup-test/bob_paired.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Alice: Bob never confirmed pairing");

	// Share library with Bob - THIS IS THE CRITICAL TEST
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
			println!("Alice: Share library SUCCEEDED!");
			std::fs::write(
				"/tmp/spacedrive-sync-setup-test/alice_success.txt",
				"success",
			)
			.unwrap();
		}
		Err(e) => {
			println!("Alice: Share library FAILED: {:?}", e);
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

	// Stay up until Bob has the library, so Alice's exit cannot cut the
	// share short.
	wait_for_file(
		"/tmp/spacedrive-sync-setup-test/bob_success.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Alice: Bob never received the library");

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

	println!("Bob: Core initialized");

	// Wait for Alice's library ID
	println!("Bob: Waiting for Alice's library ID...");
	let library_id = wait_for_file(
		"/tmp/spacedrive-sync-setup-test/library_id.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Bob: Alice never wrote a library id");
	println!("Bob: Found library ID: {}", library_id);

	// Wait for pairing code
	println!("Bob: Waiting for pairing code...");
	let pairing_code = wait_for_file(
		"/tmp/spacedrive-sync-setup-test/pairing_code.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Bob: Alice never wrote a pairing code");

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
	wait_for_connected_device(&core, Duration::from_secs(30))
		.await
		.expect("Bob: Pairing timeout");
	println!("Bob: Pairing successful!");
	std::fs::write("/tmp/spacedrive-sync-setup-test/bob_paired.txt", "paired").unwrap();

	// Wait for Alice to share her library (ShareLocalLibrary creates it on Bob's side)
	println!("Bob: Waiting for Alice's ShareLocalLibrary to create library...");
	let alice_lib_uuid = uuid::Uuid::parse_str(&library_id).unwrap();
	match wait_until("the shared library", Duration::from_secs(30), || async {
		core.libraries.get_library(alice_lib_uuid).await
	})
	.await
	{
		Ok(lib) => {
			println!("Bob: Library received from Alice! ID: {}", lib.id());
			std::fs::write("/tmp/spacedrive-sync-setup-test/bob_success.txt", "success").unwrap();
			// Alice's reply travels back over this connection; stay up until
			// her action has returned, or the reply dies with the process.
			wait_for_file(
				"/tmp/spacedrive-sync-setup-test/alice_success.txt",
				Duration::from_secs(60),
			)
			.await
			.expect("Bob: Alice's share never returned");
		}
		Err(_) => {
			println!("Bob: Library was never created - UNIQUE constraint may have failed");
			std::fs::write(
				"/tmp/spacedrive-sync-setup-test/bob_error.txt",
				"Timeout waiting for library from Alice - ShareLocalLibrary may have failed with UNIQUE constraint",
			)
			.unwrap();
			panic!("Bob: Timeout waiting for library");
		}
	}

	println!("Bob: Test completed");
}

/// Carol's sync setup scenario for three-device test
#[tokio::test]
#[ignore]
async fn carol_three_device_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "carol" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-three-device-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-three-device-test/carol");

	println!("Carol: Starting three-device test");

	// Initialize Core
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();

	core.device.set_name("Carol Device".to_string()).unwrap();

	// Initialize networking
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	tokio::time::sleep(Duration::from_secs(2)).await;
	println!("Carol: Core initialized");

	// Wait for Alice's library ID
	println!("Carol: Waiting for Alice's library ID...");
	let library_id = loop {
		if let Ok(id) = std::fs::read_to_string("/tmp/spacedrive-three-device-test/library_id.txt")
		{
			break id.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Carol: Found library ID: {}", library_id);

	// Wait for pairing code
	println!("Carol: Waiting for Alice's second pairing code...");
	let pairing_code = loop {
		if let Ok(code) =
			std::fs::read_to_string("/tmp/spacedrive-three-device-test/pairing_code_carol.txt")
		{
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};

	// Join pairing
	println!("Carol: Joining pairing...");
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
	println!("Carol: Waiting for pairing to complete...");
	let mut attempts = 0;
	while attempts < 30 {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected = core.services.device.get_connected_devices().await.unwrap();
		if !connected.is_empty() {
			println!("Carol: Pairing successful!");

			// Wait for Alice's ShareLocalLibrary to create library
			println!("Carol: Waiting for library from Alice...");
			let alice_lib_uuid = uuid::Uuid::parse_str(&library_id).unwrap();
			let mut lib_wait_attempts = 0;

			while lib_wait_attempts < 30 {
				tokio::time::sleep(Duration::from_secs(1)).await;

				if let Some(lib) = core.libraries.get_library(alice_lib_uuid).await {
					println!("Carol: ✅ Library received! ID: {}", lib.id());

					// Wait a bit for device sync to propagate
					tokio::time::sleep(Duration::from_secs(3)).await;

					// Check if Bob's device is in the library (via shared sync)
					use sd_core::infra::db::entities;
					use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

					// Read Bob's device ID
					let bob_device_id = if let Ok(id) = std::fs::read_to_string(
						"/tmp/spacedrive-three-device-test/bob_device_id.txt",
					) {
						uuid::Uuid::parse_str(id.trim()).ok()
					} else {
						None
					};

					if let Some(bob_id) = bob_device_id {
						let bob_device = entities::device::Entity::find()
							.filter(entities::device::Column::Uuid.eq(bob_id))
							.one(lib.db().conn())
							.await
							.unwrap();

						if bob_device.is_some() {
							println!(
								"Carol: ✅ Bob's device automatically discovered via shared sync!"
							);
							std::fs::write(
								"/tmp/spacedrive-three-device-test/carol_success.txt",
								"success",
							)
							.unwrap();
						} else {
							println!("Carol: ❌ Bob's device NOT found in library");
							std::fs::write(
								"/tmp/spacedrive-three-device-test/carol_error.txt",
								"Bob device not found - shared sync failed",
							)
							.unwrap();
						}
					} else {
						println!("Carol: ⚠️ Could not read Bob's device ID");
					}

					break;
				}

				lib_wait_attempts += 1;
			}

			break;
		}

		attempts += 1;
	}

	println!("Carol: Test completed");
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

	// Bob waits for the marker files himself, so he can start at once.
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

/// Three-device discovery test - verify shared resource sync enables automatic device discovery
#[tokio::test]
async fn test_three_device_discovery() {
	println!("Testing three-device automatic discovery via shared sync...");

	// Clean up
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-three-device-test");
	std::fs::create_dir_all("/tmp/spacedrive-three-device-test").unwrap();

	// This test verifies that:
	// 1. Alice pairs with Bob and runs sync setup
	// 2. Alice pairs with Carol and runs sync setup
	// 3. Bob automatically discovers Carol's device via shared sync
	// 4. Carol automatically discovers Bob's device via shared sync
	// No direct pairing between Bob and Carol needed!

	println!("✅ Three-device discovery test placeholder");
	println!("This test requires modifications to alice and bob scenarios");
	println!("to add Carol pairing and device ID writing");

	// TODO: Implement full three-device test with:
	// - Alice pairs with Bob (existing flow)
	// - Alice pairs with Carol (new flow)
	// - Bob verifies Carol's device in library
	// - Carol verifies Bob's device in library
}
