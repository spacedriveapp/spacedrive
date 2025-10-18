//! End-to-end sync backfill integration test
//!
//! This test validates the full sync stack by:
//! 1. Device A (Alice) indexes the Spacedrive source code
//! 2. Device B (Bob) pairs with Alice and triggers backfill
//! 3. Validates that Bob receives all indexed data via sync

use sd_core::infra::db::entities::{entry, entry_closure};
use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use sea_orm::{EntityTrait, PaginatorTrait};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

const TEST_DIR: &str = "/tmp/spacedrive-sync-backfill-test";

/// Alice indexes the Spacedrive source code
#[tokio::test]
#[ignore]
async fn alice_indexes_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);

	let data_dir = PathBuf::from(format!("{}/alice", TEST_DIR));
	let device_name = "Alice's Device";

	println!("Alice: Starting sync backfill test");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	core.device.set_name(device_name.to_string()).unwrap();
	println!("Alice: Core initialized");

	// Subscribe to events to monitor job completion
	let mut event_subscriber = core.events.subscribe();

	// Get the Spacedrive project root (current working directory during test)
	let project_root = env::current_dir().expect("Failed to get current directory");
	println!("Alice: Project root: {:?}", project_root);

	// Create library
	println!("Alice: Creating library...");
	let library = core
		.libraries
		.create_library("Test Sync Library", None, core.context.clone())
		.await
		.unwrap();

	// Register device in database
	let db = library.db();
	let device = core.device.to_device().unwrap();

	use sd_core::infra::db::entities;
	use sea_orm::{ActiveModelTrait, ColumnTrait, QueryFilter};

	let device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await
		.unwrap()
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(db.conn()).await.unwrap()
		}
	};

	// Create a location pointing to the Spacedrive source code
	println!("Alice: Creating location for source code...");

	let location_args = sd_core::location::LocationCreateArgs {
		path: project_root.clone(),
		name: Some("Spacedrive Source".to_string()),
		index_mode: sd_core::location::IndexMode::Shallow,
	};

	let location_id = sd_core::location::create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await
	.expect("Failed to create location");

	println!("Alice: Location created with ID: {}", location_id);

	// The indexing job is automatically started by add_location, so we just monitor events
	println!("Alice: Monitoring indexer job progress...");

	let mut job_completed = false;
	let mut attempts = 0;
	let max_attempts = 300; // 5 minutes max

	tokio::spawn(async move {
		while let Ok(event) = event_subscriber.recv().await {
			match event {
				sd_core::infra::event::Event::JobProgress {
					job_id,
					progress,
					message,
					..
				} => {
					println!(
						"Alice: Job {} progress: {}% - {}",
						job_id,
						progress,
						message.unwrap_or_else(|| "".to_string())
					);
				}
				sd_core::infra::event::Event::JobCompleted { job_id, .. } => {
					println!("Alice: Job {} completed!", job_id);
				}
				sd_core::infra::event::Event::JobFailed { error, .. } => {
					panic!("Alice: Job failed: {}", error);
				}
				_ => {}
			}
		}
	});

	// Poll for indexing completion by checking if files have been indexed
	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let entry_count = entry::Entity::find()
			.count(library.db().conn())
			.await
			.unwrap();

		if entry_count > 0 && attempts > 10 {
			// Give it some time to ensure indexing is done
			tokio::time::sleep(Duration::from_secs(3)).await;

			let final_entry_count = entry::Entity::find()
				.count(library.db().conn())
				.await
				.unwrap();

			if final_entry_count == entry_count {
				println!("Alice: Indexing complete! Found {} entry entries", final_entry_count);
				job_completed = true;
				break;
			}
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Alice: Indexing timeout - job did not complete");
		}

		if attempts % 10 == 0 {
			println!("Alice: Still waiting for indexing... (current count: {})", entry_count);
		}
	}

	if !job_completed {
		panic!("Alice: Failed to complete indexing");
	}

	// Count total entries via entry_closure table
	let entry_count = entry_closure::Entity::find()
		.count(library.db().conn())
		.await
		.unwrap();

	println!("Alice: Total entry_closure count: {}", entry_count);

	// Write entry count to shared file for Bob to validate against
	std::fs::create_dir_all(TEST_DIR).unwrap();
	std::fs::write(
		format!("{}/alice_entry_count.txt", TEST_DIR),
		entry_count.to_string(),
	)
	.unwrap();
	println!("Alice: Entry count written to shared file");

	// Initialize networking for device pairing
	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Alice: Networking initialized");

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
		"Alice: Pairing code: {}... (expires in {}s)",
		short_code, expires_in
	);

	// Write pairing code for Bob
	std::fs::write(format!("{}/pairing_code.txt", TEST_DIR), &pairing_code).unwrap();
	println!("Alice: Pairing code written");

	// Wait for Bob to connect
	println!("Alice: Waiting for Bob to pair...");
	let mut pair_attempts = 0;
	let max_pair_attempts = 60;

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core
			.services
			.device
			.get_connected_devices()
			.await
			.unwrap();
		if !connected_devices.is_empty() {
			println!("Alice: Bob paired successfully!");
			break;
		}

		pair_attempts += 1;
		if pair_attempts >= max_pair_attempts {
			panic!("Alice: Pairing timeout");
		}

		if pair_attempts % 5 == 0 {
			println!("Alice: Still waiting for Bob...");
		}
	}

	// Give time for sync to initialize and begin backfill
	println!("Alice: Waiting for sync to begin...");
	tokio::time::sleep(Duration::from_secs(10)).await;

	// Write success marker
	std::fs::write(format!("{}/alice_success.txt", TEST_DIR), "success").unwrap();
	println!("Alice: Test completed successfully");

	// Keep alive for Bob to complete backfill
	tokio::time::sleep(Duration::from_secs(60)).await;
}

/// Bob pairs with Alice and backfills the indexed data
#[tokio::test]
#[ignore]
async fn bob_backfills_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);

	let data_dir = PathBuf::from(format!("{}/bob", TEST_DIR));
	let device_name = "Bob's Device";

	println!("Bob: Starting sync backfill test");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	core.device.set_name(device_name.to_string()).unwrap();
	println!("Bob: Core initialized");

	// Initialize networking
	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized");

	// Wait for Alice's pairing code
	println!("Bob: Looking for pairing code...");
	let pairing_code = loop {
		if let Ok(code) = std::fs::read_to_string(format!("{}/pairing_code.txt", TEST_DIR)) {
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Found pairing code");

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
	} else {
		panic!("Networking not initialized");
	}
	println!("Bob: Successfully paired with Alice");

	// Wait for devices to be connected
	println!("Bob: Waiting for connection...");
	let mut pair_attempts = 0;
	let max_pair_attempts = 30;

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core
			.services
			.device
			.get_connected_devices()
			.await
			.unwrap();
		if !connected_devices.is_empty() {
			println!("Bob: Connected to Alice!");
			break;
		}

		pair_attempts += 1;
		if pair_attempts >= max_pair_attempts {
			panic!("Bob: Connection timeout");
		}
	}

	// Read Alice's entry count for validation
	let alice_entry_count: u64 = loop {
		if let Ok(content) = std::fs::read_to_string(format!("{}/alice_entry_count.txt", TEST_DIR))
		{
			if let Ok(count) = content.trim().parse() {
				break count;
			}
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Alice indexed {} entries", alice_entry_count);

	// Wait for library to be available after pairing
	println!("Bob: Waiting for shared library...");
	let library = loop {
		let libs = core.libraries.list().await;
		if !libs.is_empty() {
			break libs[0].clone();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Got shared library: {}", library.id());

	let mut backfill_attempts = 0;
	let max_backfill_attempts = 120; // 2 minutes

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let bob_entry_count = entry_closure::Entity::find()
			.count(library.db().conn())
			.await
			.unwrap();

		if backfill_attempts % 10 == 0 {
			println!(
				"Bob: Current entry count: {} / {}",
				bob_entry_count, alice_entry_count
			);
		}

		// Check if Bob has received most of the data (within 5% tolerance)
		let min_expected = (alice_entry_count as f64 * 0.95) as u64;
		if bob_entry_count >= min_expected {
			println!(
				"Bob: Backfill complete! Received {} entries (expected ~{})",
				bob_entry_count, alice_entry_count
			);

			// Validate the count is reasonable
			if bob_entry_count < alice_entry_count / 2 {
				panic!(
					"Bob: Backfill validation failed - received {} but expected ~{}",
					bob_entry_count, alice_entry_count
				);
			}

			break;
		}

		backfill_attempts += 1;
		if backfill_attempts >= max_backfill_attempts {
			panic!(
				"Bob: Backfill timeout - only received {} / {} entries",
				bob_entry_count, alice_entry_count
			);
		}
	}

	// Write success marker
	std::fs::write(format!("{}/bob_success.txt", TEST_DIR), "success").unwrap();
	println!("Bob: Test completed successfully");

	// Give time for graceful shutdown
	tokio::time::sleep(Duration::from_secs(5)).await;
}

/// Main test orchestrator
#[tokio::test]
async fn test_sync_backfill_end_to_end() {
	println!("Starting end-to-end sync backfill integration test");

	// Clean up from previous runs
	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	let mut runner = CargoTestRunner::for_test_file("sync_backfill_integration_test")
		.with_timeout(Duration::from_secs(600)) // 10 minutes for full test
		.add_subprocess("alice", "alice_indexes_scenario")
		.add_subprocess("bob", "bob_backfills_scenario");

	// Start Alice first to index the source code
	println!("Starting Alice (indexer)...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Wait for Alice to complete indexing and initialize networking
	// Indexing can take 30-60 seconds depending on system
	println!("Waiting for Alice to complete indexing...");
	tokio::time::sleep(Duration::from_secs(90)).await;

	// Start Bob to trigger backfill
	println!("Starting Bob (backfill receiver)...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Wait for both devices to complete
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success = std::fs::read_to_string(format!("{}/alice_success.txt", TEST_DIR))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);
			let bob_success = std::fs::read_to_string(format!("{}/bob_success.txt", TEST_DIR))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!("End-to-end sync backfill test successful!");
		}
		Err(e) => {
			println!("End-to-end sync backfill test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Sync backfill test failed - see output above");
		}
	}
}
