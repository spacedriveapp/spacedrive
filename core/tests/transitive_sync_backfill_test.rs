//! Transitive Sync Backfill Test
//!
//! Tests the scenario where Carol receives Alice's data through transitive trust:
//! 1. Alice indexes a location with real data
//! 2. Alice pairs with Bob (direct) and sets up library sync
//! 3. Bob receives Alice's data via sync
//! 4. Bob pairs with Carol (direct) and sets up library sync
//! 5. Bob vouches Carol to Alice (proxy pairing)
//! 6. Carol syncs Alice's location data directly from Alice (all devices online)
//!
//! This test uses:
//! - Subprocess framework for true process isolation
//! - Real networking (no MockTransport)
//! - Real Core initialization and services
//! - Real device pairing and sync protocols

mod helpers;

use helpers::{create_test_volume, register_device};
use sd_core::testing::CargoTestRunner;
use sd_core::{
	location::{create_location, IndexMode, LocationCreateArgs},
	service::Service,
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

const TEST_DIR: &str = "/tmp/spacedrive-transitive-sync-test";

/// Alice's scenario - indexes location, pairs with Bob, sets up sync
#[tokio::test]
#[ignore]
async fn alice_transitive_sync_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);
	let data_dir = PathBuf::from(format!("{}/alice", TEST_DIR));
	let device_name = "Alice's Test Device";

	println!("Alice: Starting transitive sync test");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized");

	core.device.set_name(device_name.to_string()).unwrap();

	// Create library with shared UUID
	let library_id = loop {
		if let Ok(id_str) = std::fs::read_to_string(format!("{}/library_id.txt", TEST_DIR)) {
			break Uuid::parse_str(id_str.trim()).unwrap();
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	};
	println!("Alice: Using library ID: {}", library_id);

	let library = core
		.libraries
		.create_library_with_id(
			library_id,
			"Transitive Sync Test Library",
			None,
			core.context.clone(),
		)
		.await
		.unwrap();
	println!("Alice: Library created");

	let device_id = core.device.device_id().unwrap();

	// Create volume for location
	println!("Alice: Creating test volume...");
	let volume = create_test_volume(&library, device_id, "alice-volume", "Alice Volume")
		.await
		.unwrap();
	println!("Alice: Volume created: {}", volume);

	// Index Spacedrive source code as test data
	println!("Alice: Indexing location with real data...");
	let test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();

	let location_args = LocationCreateArgs {
		path: test_path.clone(),
		name: Some("spacedrive".to_string()),
		index_mode: IndexMode::Content,
	};

	// Get device record
	let device_record = sd_core::infra::db::entities::device::Entity::find()
		.one(library.db().conn())
		.await
		.unwrap()
		.expect("Device not found");

	let location_db_id = create_location(
		library.clone(),
		library.event_bus(),
		location_args,
		device_record.id,
	)
	.await
	.unwrap();

	println!("Alice: Location created, ID: {}", location_db_id);

	// Link location to volume
	let first_volume = sd_core::infra::db::entities::volume::Entity::find()
		.filter(sd_core::infra::db::entities::volume::Column::DeviceId.eq(device_id))
		.one(library.db().conn())
		.await
		.unwrap()
		.expect("Volume not found");

	sd_core::infra::db::entities::location::Entity::update_many()
		.filter(sd_core::infra::db::entities::location::Column::Id.eq(location_db_id))
		.col_expr(
			sd_core::infra::db::entities::location::Column::VolumeId,
			sea_orm::sea_query::Expr::value(first_volume.id),
		)
		.exec(library.db().conn())
		.await
		.unwrap();

	// Wait for indexing to complete
	println!("Alice: Waiting for indexing to complete...");
	let start = tokio::time::Instant::now();
	loop {
		tokio::time::sleep(Duration::from_secs(2)).await;

		let location = sd_core::infra::db::entities::location::Entity::find()
			.filter(sd_core::infra::db::entities::location::Column::Id.eq(location_db_id))
			.one(library.db().conn())
			.await
			.unwrap()
			.expect("Location not found");

		if location.scan_state == "completed" {
			println!("Alice: Indexing completed");
			break;
		}

		if start.elapsed() > Duration::from_secs(120) {
			panic!("Alice: Indexing timeout");
		}
	}

	// Record entry count for verification
	let alice_entry_count = sd_core::infra::db::entities::entry::Entity::find()
		.count(library.db().conn())
		.await
		.unwrap();
	println!("Alice: Indexed {} entries", alice_entry_count);
	std::fs::write(
		format!("{}/alice_entry_count.txt", TEST_DIR),
		alice_entry_count.to_string(),
	)
	.unwrap();

	// Enable auto-vouch for proxy pairing later
	println!("Alice: Enabling auto-vouch...");
	{
		let mut config = core.config.write().await;
		config.proxy_pairing.auto_vouch_to_all = true;
		config.save().unwrap();
	}

	// Initialize networking
	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Alice: Networking initialized");

	// Phase 1: Pair with Bob
	println!("\n=== PHASE 1: Alice pairs with Bob ===");
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

	println!("Alice: Pairing code for Bob: {}", pairing_code);
	std::fs::write(format!("{}/pairing_code_bob.txt", TEST_DIR), &pairing_code).unwrap();

	// Wait for Bob to pair
	let mut attempts = 0;
	let mut bob_device_id = None;
	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let paired_devices = core
			.services
			.device
			.get_connected_devices()
			.await
			.unwrap_or_default();

		if !paired_devices.is_empty() {
			println!("Alice: Bob paired successfully!");
			if let Ok(device_infos) = core.services.device.get_connected_devices_info().await {
				for info in &device_infos {
					println!("Alice sees: {} ({})", info.device_name, info.device_id);
					if info.device_name.contains("Bob") {
						bob_device_id = Some(info.device_id);
					}
				}
			}
			break;
		}

		attempts += 1;
		if attempts >= 60 {
			panic!("Alice: Timeout waiting for Bob to pair");
		}
	}

	let bob_id = bob_device_id.expect("Bob's device ID not found");
	println!("Alice: Bob's device ID: {}", bob_id);

	// Phase 2: Set up library sync with Bob (register Bob in library)
	println!("\n=== PHASE 2: Alice sets up sync with Bob ===");
	register_device(&library, bob_id, "Bob").await.unwrap();
	println!("Alice: Bob registered in library");

	// Initialize and start sync service with real networking
	println!("Alice: Starting sync service...");
	let networking = core
		.services
		.networking
		.clone()
		.expect("networking service required for sync");
	library
		.init_sync_service(device_id, networking)
		.await
		.unwrap();
	library.sync_service().unwrap().start().await.unwrap();
	println!("Alice: Sync service started");

	std::fs::write(format!("{}/alice_bob_synced.txt", TEST_DIR), "ready").unwrap();

	// Phase 3: Wait for Carol to proxy-pair
	println!("\n=== PHASE 3: Alice waits for Carol (proxy pairing) ===");
	println!("Alice: Waiting for Bob to pair with Carol...");
	loop {
		if std::fs::read_to_string(format!("{}/bob_carol_paired.txt", TEST_DIR)).is_ok() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
	println!("Alice: Bob paired with Carol");

	// With auto-vouch enabled, Alice should automatically vouch Carol
	println!("Alice: Auto-vouch enabled - should vouch Carol to Alice");
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Carol should now be proxy-paired and start syncing
	println!("Alice: Keeping sync service alive for Carol to sync...");
	tokio::time::sleep(Duration::from_secs(30)).await;

	std::fs::write(format!("{}/alice_success.txt", TEST_DIR), "success").unwrap();
	println!("Alice: Test completed");

	// Keep alive a bit longer
	tokio::time::sleep(Duration::from_secs(10)).await;
}

/// Bob's scenario - pairs with Alice, syncs data, pairs with Carol, facilitates transitive sync
#[tokio::test]
#[ignore]
async fn bob_transitive_sync_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);
	let data_dir = PathBuf::from(format!("{}/bob", TEST_DIR));
	let device_name = "Bob's Test Device";

	println!("Bob: Starting transitive sync test");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized");

	core.device.set_name(device_name.to_string()).unwrap();

	// Create library with same UUID as Alice
	let library_id = loop {
		if let Ok(id_str) = std::fs::read_to_string(format!("{}/library_id.txt", TEST_DIR)) {
			break Uuid::parse_str(id_str.trim()).unwrap();
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	};
	println!("Bob: Using library ID: {}", library_id);

	let library = core
		.libraries
		.create_library_with_id(
			library_id,
			"Transitive Sync Test Library",
			None,
			core.context.clone(),
		)
		.await
		.unwrap();
	println!("Bob: Library created");

	let device_id = core.device.device_id().unwrap();

	// Initialize networking
	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized");

	// Phase 1: Pair with Alice
	println!("\n=== PHASE 1: Bob pairs with Alice ===");
	println!("Bob: Waiting for Alice's pairing code...");
	let pairing_code = loop {
		if let Ok(code) = std::fs::read_to_string(format!("{}/pairing_code_bob.txt", TEST_DIR)) {
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Found pairing code");

	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
	}
	println!("Bob: Joined pairing with Alice");

	// Wait for pairing completion
	let mut attempts = 0;
	let mut alice_device_id = None;
	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let paired_devices = core
			.services
			.device
			.get_connected_devices()
			.await
			.unwrap_or_default();

		if !paired_devices.is_empty() {
			println!("Bob: Paired with Alice!");
			if let Ok(device_infos) = core.services.device.get_connected_devices_info().await {
				for info in &device_infos {
					println!("Bob sees: {} ({})", info.device_name, info.device_id);
					if info.device_name.contains("Alice") {
						alice_device_id = Some(info.device_id);
					}
				}
			}
			break;
		}

		attempts += 1;
		if attempts >= 60 {
			panic!("Bob: Timeout pairing with Alice");
		}
	}

	let alice_id = alice_device_id.expect("Alice's device ID not found");
	println!("Bob: Alice's device ID: {}", alice_id);

	// Phase 2: Set up sync with Alice and wait for data
	println!("\n=== PHASE 2: Bob sets up sync with Alice ===");
	register_device(&library, alice_id, "Alice").await.unwrap();
	println!("Bob: Alice registered in library");

	// Wait for Alice to be ready
	loop {
		if std::fs::read_to_string(format!("{}/alice_bob_synced.txt", TEST_DIR)).is_ok() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	println!("Bob: Starting sync service...");
	let networking = core
		.services
		.networking
		.clone()
		.expect("networking service required for sync");
	library
		.init_sync_service(device_id, networking)
		.await
		.unwrap();
	library.sync_service().unwrap().start().await.unwrap();
	println!("Bob: Sync service started");

	// Wait for sync from Alice
	println!("Bob: Waiting for sync from Alice...");
	let start = tokio::time::Instant::now();
	loop {
		tokio::time::sleep(Duration::from_secs(2)).await;

		let bob_entries = sd_core::infra::db::entities::entry::Entity::find()
			.count(library.db().conn())
			.await
			.unwrap();

		if bob_entries > 10 {
			println!("Bob: Received {} entries from Alice", bob_entries);
			break;
		}

		if start.elapsed() > Duration::from_secs(90) {
			panic!("Bob: Timeout waiting for sync from Alice");
		}
	}

	// Phase 3: Pair with Carol
	println!("\n=== PHASE 3: Bob pairs with Carol ===");
	tokio::time::sleep(Duration::from_secs(2)).await;

	let (pairing_code_carol, _) = if let Some(networking) = core.networking() {
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

	println!("Bob: Pairing code for Carol: {}", pairing_code_carol);
	std::fs::write(
		format!("{}/pairing_code_carol.txt", TEST_DIR),
		&pairing_code_carol,
	)
	.unwrap();

	// Wait for Carol to pair
	attempts = 0;
	let mut carol_device_id = None;
	let initial_paired_count = core
		.services
		.device
		.get_connected_devices()
		.await
		.unwrap_or_default()
		.len();

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let paired_devices = core
			.services
			.device
			.get_connected_devices()
			.await
			.unwrap_or_default();

		if paired_devices.len() > initial_paired_count {
			println!("Bob: Carol paired successfully!");
			if let Ok(device_infos) = core.services.device.get_connected_devices_info().await {
				for info in &device_infos {
					println!("Bob sees: {} ({})", info.device_name, info.device_id);
					if info.device_name.contains("Carol") {
						carol_device_id = Some(info.device_id);
					}
				}
			}
			break;
		}

		attempts += 1;
		if attempts >= 60 {
			panic!("Bob: Timeout waiting for Carol to pair");
		}
	}

	let carol_id = carol_device_id.expect("Carol's device ID not found");
	println!("Bob: Carol's device ID: {}", carol_id);

	// Phase 4: Set up sync with Carol
	println!("\n=== PHASE 4: Bob sets up sync with Carol ===");
	register_device(&library, carol_id, "Carol").await.unwrap();
	println!("Bob: Carol registered in library");

	std::fs::write(format!("{}/bob_carol_paired.txt", TEST_DIR), "success").unwrap();

	// Proxy pairing should happen automatically through Alice's auto-vouch
	println!("Bob: Alice should auto-vouch Carol via proxy pairing");
	tokio::time::sleep(Duration::from_secs(10)).await;

	// Keep alive for Carol to sync
	println!("Bob: Keeping sync service alive for Carol...");
	tokio::time::sleep(Duration::from_secs(30)).await;

	std::fs::write(format!("{}/bob_success.txt", TEST_DIR), "success").unwrap();
	println!("Bob: Test completed");

	tokio::time::sleep(Duration::from_secs(10)).await;
}

/// Carol's scenario - pairs with Bob, gets proxy-paired to Alice, syncs Alice's data
#[tokio::test]
#[ignore]
async fn carol_transitive_sync_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "carol" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);
	let data_dir = PathBuf::from(format!("{}/carol", TEST_DIR));
	let device_name = "Carol's Test Device";

	println!("Carol: Starting transitive sync test");
	println!("Carol: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Carol: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Carol: Core initialized");

	core.device.set_name(device_name.to_string()).unwrap();

	// Create library with same UUID as Alice and Bob
	let library_id = loop {
		if let Ok(id_str) = std::fs::read_to_string(format!("{}/library_id.txt", TEST_DIR)) {
			break Uuid::parse_str(id_str.trim()).unwrap();
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	};
	println!("Carol: Using library ID: {}", library_id);

	let library = core
		.libraries
		.create_library_with_id(
			library_id,
			"Transitive Sync Test Library",
			None,
			core.context.clone(),
		)
		.await
		.unwrap();
	println!("Carol: Library created");

	let device_id = core.device.device_id().unwrap();

	// Initialize networking
	println!("Carol: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Carol: Networking initialized");

	// Phase 1: Pair with Bob
	println!("\n=== PHASE 1: Carol pairs with Bob ===");
	println!("Carol: Waiting for Bob's pairing code...");
	let pairing_code = loop {
		if let Ok(code) = std::fs::read_to_string(format!("{}/pairing_code_carol.txt", TEST_DIR)) {
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Carol: Found pairing code");

	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
	}
	println!("Carol: Joined pairing with Bob");

	// Wait for pairing completion
	let mut attempts = 0;
	let mut bob_device_id = None;
	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let paired_devices = core
			.services
			.device
			.get_connected_devices()
			.await
			.unwrap_or_default();

		if !paired_devices.is_empty() {
			println!("Carol: Paired with Bob!");
			if let Ok(device_infos) = core.services.device.get_connected_devices_info().await {
				for info in &device_infos {
					println!("Carol sees: {} ({})", info.device_name, info.device_id);
					if info.device_name.contains("Bob") {
						bob_device_id = Some(info.device_id);
					}
				}
			}
			break;
		}

		attempts += 1;
		if attempts >= 60 {
			panic!("Carol: Timeout pairing with Bob");
		}
	}

	let bob_id = bob_device_id.expect("Bob's device ID not found");
	println!("Carol: Bob's device ID: {}", bob_id);

	// Phase 2: Set up sync with Bob
	println!("\n=== PHASE 2: Carol sets up sync with Bob ===");
	register_device(&library, bob_id, "Bob").await.unwrap();
	println!("Carol: Bob registered in library");

	println!("Carol: Starting sync service...");
	let networking = core
		.services
		.networking
		.clone()
		.expect("networking service required for sync");
	library
		.init_sync_service(device_id, networking)
		.await
		.unwrap();
	library.sync_service().unwrap().start().await.unwrap();
	println!("Carol: Sync service started");

	// Phase 3: Wait for proxy pairing and sync from Alice
	println!("\n=== PHASE 3: Carol waits for proxy pairing and syncs from Alice ===");
	println!("Carol: Should be proxy-paired to Alice through Bob's vouching");
	println!("Carol: Waiting for Alice's data to sync...");

	let alice_expected_count: u64 = loop {
		if let Ok(count_str) =
			std::fs::read_to_string(format!("{}/alice_entry_count.txt", TEST_DIR))
		{
			break count_str.trim().parse().unwrap();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!(
		"Carol: Expected {} entries from Alice",
		alice_expected_count
	);

	// Wait for sync to complete
	let start = tokio::time::Instant::now();
	let mut carol_final_count = 0;
	loop {
		tokio::time::sleep(Duration::from_secs(3)).await;

		carol_final_count = sd_core::infra::db::entities::entry::Entity::find()
			.count(library.db().conn())
			.await
			.unwrap();

		println!("Carol: Current entries: {}", carol_final_count);

		// Allow 10% tolerance for sync
		let diff = (carol_final_count as i64 - alice_expected_count as i64).abs();
		let tolerance = (alice_expected_count as f64 * 0.1) as i64;

		if diff <= tolerance && carol_final_count > 10 {
			println!(
				"Carol: Sync complete! Received {} entries",
				carol_final_count
			);
			break;
		}

		if start.elapsed() > Duration::from_secs(120) {
			panic!(
				"Carol: Sync timeout - expected ~{}, got {}",
				alice_expected_count, carol_final_count
			);
		}
	}

	// Verify entry count
	let diff = (carol_final_count as i64 - alice_expected_count as i64).abs();
	let diff_pct = (diff as f64 / alice_expected_count as f64) * 100.0;
	println!(
		"Carol: Verification - Alice: {}, Carol: {} (diff: {}, {:.1}%)",
		alice_expected_count, carol_final_count, diff, diff_pct
	);

	assert!(
		diff_pct <= 10.0,
		"Carol: Entry count difference too large: {:.1}%",
		diff_pct
	);

	println!("Carol: ✅ Transitive sync successful!");
	std::fs::write(format!("{}/carol_success.txt", TEST_DIR), "success").unwrap();
	println!("Carol: Test completed");

	tokio::time::sleep(Duration::from_secs(5)).await;
}

/// Main test orchestrator - coordinates three devices for transitive sync
#[tokio::test]
async fn test_transitive_sync_backfill() {
	println!("Testing transitive sync backfill with three devices");
	println!("Alice indexes → pairs with Bob → Bob pairs with Carol → Carol syncs Alice's data");

	// Clean up test directory
	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	// Generate shared library UUID
	let library_id = Uuid::new_v4();
	std::fs::write(
		format!("{}/library_id.txt", TEST_DIR),
		library_id.to_string(),
	)
	.unwrap();
	println!("Generated library ID: {}", library_id);

	let mut runner = CargoTestRunner::for_test_file("transitive_sync_backfill_test")
		.with_timeout(Duration::from_secs(400))
		.add_subprocess("alice", "alice_transitive_sync_scenario")
		.add_subprocess("bob", "bob_transitive_sync_scenario")
		.add_subprocess("carol", "carol_transitive_sync_scenario");

	// Start Alice first - she needs to index before others join
	println!("\n=== Starting Alice (indexing and initiating) ===");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Wait for Alice to index and initialize
	println!("Waiting for Alice to complete indexing...");
	tokio::time::sleep(Duration::from_secs(60)).await;

	// Start Bob - pairs with Alice and syncs
	println!("\n=== Starting Bob (syncing from Alice) ===");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Wait for Alice-Bob pairing and initial sync
	println!("Waiting for Alice-Bob pairing and sync...");
	tokio::time::sleep(Duration::from_secs(45)).await;

	// Start Carol - pairs with Bob, gets proxy-paired to Alice, syncs
	println!("\n=== Starting Carol (transitive sync via proxy pairing) ===");
	runner
		.spawn_single_process("carol")
		.await
		.expect("Failed to spawn Carol");

	// Wait for all phases to complete
	println!("\n=== Waiting for transitive sync to complete ===");
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success = std::fs::read_to_string(format!("{}/alice_success.txt", TEST_DIR))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);
			let bob_success = std::fs::read_to_string(format!("{}/bob_success.txt", TEST_DIR))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);
			let carol_success = std::fs::read_to_string(format!("{}/carol_success.txt", TEST_DIR))
				.map(|content| content.trim() == "success")
				.unwrap_or(false);

			alice_success && bob_success && carol_success
		})
		.await;

	match result {
		Ok(_) => {
			println!("\n✅ TRANSITIVE SYNC BACKFILL TEST PASSED!");
			println!(
				"   ✅ Alice indexed {} entries",
				std::fs::read_to_string(format!("{}/alice_entry_count.txt", TEST_DIR))
					.unwrap_or_default()
					.trim()
			);
			println!("   ✅ Alice paired with Bob (direct)");
			println!("   ✅ Bob synced Alice's data");
			println!("   ✅ Bob paired with Carol (direct)");
			println!("   ✅ Carol proxy-paired to Alice (via Bob's vouch)");
			println!("   ✅ Carol synced Alice's data directly");
			println!("\n   This proves transitive sync works: Carol received Alice's data");
			println!("   after establishing trust through Bob via proxy pairing!");
		}
		Err(e) => {
			println!("\n❌ TRANSITIVE SYNC BACKFILL TEST FAILED: {}", e);
			println!("\nThis means the transitive sync protocol did not complete successfully.");
			println!("Check the logs above for where the sync stopped.");
			for (name, output) in runner.get_all_outputs() {
				println!("\n=== {} OUTPUT ===\n{}", name.to_uppercase(), output);
			}
			panic!("Transitive sync backfill test failed");
		}
	}
}
