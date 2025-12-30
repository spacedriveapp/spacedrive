//! Backfill + Live Events Race Condition Test
//!
//! Tests the hypothesis that live events arriving during backfill can advance
//! watermarks past unbackfilled entries, causing permanent data loss.
//!
//! Scenario:
//! 1. Alice indexes a location (creates entries)
//! 2. Bob starts backfilling from Alice
//! 3. WHILE backfill is in progress, Alice indexes ANOTHER location
//! 4. Live events from Alice arrive at Bob (buffered during backfill)
//! 5. After backfill completes, buffered events are processed
//! 6. If watermarks advance incorrectly, some entries will be permanently skipped
//!
//! Expected: Bob should have ALL entries from BOTH locations
//! Bug hypothesis: Bob will be missing entries from location 1

mod helpers;

use helpers::{
	add_and_index_location, create_snapshot_dir, init_test_tracing, register_device,
	set_all_devices_synced, MockTransport, TestConfigBuilder,
};
use sd_core::{
	infra::{db::entities, sync::NetworkTransport},
	library::Library,
	service::{sync::state::DeviceSyncState, Service},
	Core,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use std::{path::PathBuf, sync::Arc};
use tokio::{fs, time::Duration};
use uuid::Uuid;

/// Test harness for backfill race condition testing
struct BackfillRaceHarness {
	_data_dir_alice: PathBuf,
	_data_dir_bob: PathBuf,
	_core_alice: Core,
	_core_bob: Core,
	library_alice: Arc<Library>,
	library_bob: Arc<Library>,
	device_alice_id: Uuid,
	device_bob_id: Uuid,
	transport_alice: Arc<MockTransport>,
	transport_bob: Arc<MockTransport>,
	snapshot_dir: PathBuf,
}

impl BackfillRaceHarness {
	/// Create test harness - Bob will need to backfill (not set to Ready)
	async fn new(test_name: &str) -> anyhow::Result<Self> {
		let snapshot_dir = create_snapshot_dir(test_name).await?;
		init_test_tracing(test_name, &snapshot_dir)?;

		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root = std::path::PathBuf::from(home)
			.join("Library/Application Support/spacedrive/sync_tests");

		let data_dir = test_root.join("data_backfill_race");
		if data_dir.exists() {
			fs::remove_dir_all(&data_dir).await?;
		}
		fs::create_dir_all(&data_dir).await?;

		let temp_dir_alice = data_dir.join("alice");
		let temp_dir_bob = data_dir.join("bob");
		fs::create_dir_all(&temp_dir_alice).await?;
		fs::create_dir_all(&temp_dir_bob).await?;

		tracing::info!(
			snapshot_dir = %snapshot_dir.display(),
			"Starting backfill race condition test"
		);

		TestConfigBuilder::new(temp_dir_alice.clone()).build()?;
		TestConfigBuilder::new(temp_dir_bob.clone()).build()?;

		let core_alice = Core::new(temp_dir_alice.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
		let device_alice_id = core_alice.device.device_id()?;

		let core_bob = Core::new(temp_dir_bob.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
		let device_bob_id = core_bob.device.device_id()?;

		let library_alice = core_alice
			.libraries
			.create_library_no_sync("Backfill Race Test", None, core_alice.context.clone())
			.await?;

		let library_bob = core_bob
			.libraries
			.create_library_no_sync("Backfill Race Test", None, core_bob.context.clone())
			.await?;

		register_device(&library_alice, device_bob_id, "Bob").await?;
		register_device(&library_bob, device_alice_id, "Alice").await?;

		// Set Alice's last_sync_at (she's synced), leave Bob's as None (needs backfill)
		set_all_devices_synced(&library_alice).await?;

		tracing::info!(
			alice_device = %device_alice_id,
			bob_device = %device_bob_id,
			"Devices registered - Alice synced, Bob needs backfill"
		);

		let (transport_alice, transport_bob) =
			MockTransport::new_pair(device_alice_id, device_bob_id);

		// CRITICAL: Block Bob from receiving messages BEFORE starting services
		transport_alice.block_device(device_bob_id).await;
		transport_bob.block_device(device_bob_id).await;

		tracing::info!("Bob blocked from receiving messages");

		library_alice
			.init_sync_service(
				device_alice_id,
				transport_alice.clone() as Arc<dyn NetworkTransport>,
			)
			.await?;

		library_bob
			.init_sync_service(
				device_bob_id,
				transport_bob.clone() as Arc<dyn NetworkTransport>,
			)
			.await?;

		transport_alice
			.register_sync_service(
				device_alice_id,
				Arc::downgrade(library_alice.sync_service().unwrap()),
			)
			.await;
		transport_bob
			.register_sync_service(
				device_bob_id,
				Arc::downgrade(library_bob.sync_service().unwrap()),
			)
			.await;

		library_alice.sync_service().unwrap().start().await?;
		library_bob.sync_service().unwrap().start().await?;

		// Set Alice to Ready
		library_alice
			.sync_service()
			.unwrap()
			.peer_sync()
			.set_state_for_test(DeviceSyncState::Ready)
			.await;

		let bob_state = library_bob
			.sync_service()
			.unwrap()
			.peer_sync()
			.state()
			.await;

		tracing::info!(
			bob_state = ?bob_state,
			"Initial states set - Alice ready, Bob {:?} (BLOCKED)",
			bob_state
		);

		tokio::time::sleep(Duration::from_millis(100)).await;

		Ok(Self {
			_data_dir_alice: temp_dir_alice,
			_data_dir_bob: temp_dir_bob,
			_core_alice: core_alice,
			_core_bob: core_bob,
			library_alice,
			library_bob,
			device_alice_id,
			device_bob_id,
			transport_alice,
			transport_bob,
			snapshot_dir,
		})
	}

	/// Trigger backfill on Bob (also unblocks Bob)
	async fn trigger_bob_backfill(&self) -> anyhow::Result<()> {
		let sync_service = self.library_bob.sync_service().unwrap();
		let peer_sync = sync_service.peer_sync();

		// Unblock Bob
		tracing::info!("Unblocking Bob for backfill");
		self.transport_alice
			.unblock_device(self.device_bob_id)
			.await;
		self.transport_bob.unblock_device(self.device_bob_id).await;

		tracing::info!("Triggering backfill on Bob");

		peer_sync
			.set_state_for_test(DeviceSyncState::Backfilling {
				peer: self.device_alice_id,
				progress: 0,
			})
			.await;

		let backfill_manager = sync_service.backfill_manager();

		let peer_info = sd_core::service::sync::state::PeerInfo {
			device_id: self.device_alice_id,
			is_online: true,
			latency_ms: 1.0,
			has_complete_state: true,
			active_syncs: 0,
		};

		backfill_manager
			.start_backfill(vec![peer_info])
			.await
			.map_err(|e| anyhow::anyhow!("Backfill failed: {}", e))?;

		Ok(())
	}

	/// Wait for sync to stabilize
	async fn wait_for_sync(&self, max_duration: Duration) -> anyhow::Result<()> {
		let start = std::time::Instant::now();
		let mut last_alice = 0u64;
		let mut last_bob = 0u64;
		let mut stable = 0;

		while start.elapsed() < max_duration {
			tokio::time::sleep(Duration::from_millis(500)).await;

			let alice = entities::entry::Entity::find()
				.count(self.library_alice.db().conn())
				.await?;
			let bob = entities::entry::Entity::find()
				.count(self.library_bob.db().conn())
				.await?;

			tracing::debug!(alice = alice, bob = bob, "Entry counts");

			if alice == last_alice && bob == last_bob && alice > 0 {
				stable += 1;
				if stable >= 6 && alice == bob {
					tracing::info!(count = alice, "Sync complete - counts match");
					return Ok(());
				}
				if stable >= 10 {
					tracing::warn!(
						alice = alice,
						bob = bob,
						"Sync stabilized but counts don't match!"
					);
					return Ok(());
				}
			} else {
				stable = 0;
			}

			last_alice = alice;
			last_bob = bob;
		}

		tracing::warn!("Sync timed out");
		Ok(())
	}

	/// Capture test snapshot
	async fn capture_snapshot(&self, name: &str) -> anyhow::Result<()> {
		let snapshot_path = self.snapshot_dir.join(name);
		fs::create_dir_all(&snapshot_path).await?;

		let alice_dir = snapshot_path.join("alice");
		let bob_dir = snapshot_path.join("bob");
		fs::create_dir_all(&alice_dir).await?;
		fs::create_dir_all(&bob_dir).await?;

		// Copy databases
		let alice_db = self.library_alice.path().join("database.db");
		let bob_db = self.library_bob.path().join("database.db");

		if alice_db.exists() {
			fs::copy(&alice_db, alice_dir.join("database.db")).await?;
		}
		if bob_db.exists() {
			fs::copy(&bob_db, bob_dir.join("database.db")).await?;
		}

		// Write summary
		let entries_alice = entities::entry::Entity::find()
			.count(self.library_alice.db().conn())
			.await?;
		let entries_bob = entities::entry::Entity::find()
			.count(self.library_bob.db().conn())
			.await?;

		let locations_alice = entities::location::Entity::find()
			.count(self.library_alice.db().conn())
			.await?;
		let locations_bob = entities::location::Entity::find()
			.count(self.library_bob.db().conn())
			.await?;

		let summary = format!(
			r#"# Backfill Race Test Snapshot: {}

## Alice (Device {})
- Locations: {}
- Entries: {}

## Bob (Device {})
- Locations: {}
- Entries: {}

## Diff
- Entry difference: {} (Alice - Bob)
"#,
			name,
			self.device_alice_id,
			locations_alice,
			entries_alice,
			self.device_bob_id,
			locations_bob,
			entries_bob,
			entries_alice as i64 - entries_bob as i64
		);

		fs::write(snapshot_path.join("summary.md"), summary).await?;

		Ok(())
	}
}

/// Test: Backfill + concurrent indexing race condition
#[tokio::test]
async fn test_backfill_with_concurrent_indexing() -> anyhow::Result<()> {
	let harness = BackfillRaceHarness::new("backfill_race").await?;

	// Step 1: Alice indexes first location
	// Use Spacedrive crates directory for deterministic testing
	let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let crates_path = project_root.join("crates");
	tracing::info!("Step 1: Alice indexes crates");

	add_and_index_location(
		&harness.library_alice,
		crates_path.to_str().unwrap(),
		"crates",
	)
	.await?;

	let alice_entries_after_loc1 = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;

	tracing::info!(
		entries = alice_entries_after_loc1,
		"Alice has entries after first location"
	);

	let bob_entries_before = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		bob_entries_before = bob_entries_before,
		alice_entries_after_loc1 = alice_entries_after_loc1,
		"Entry counts before concurrent phase"
	);

	assert!(
		bob_entries_before < 10,
		"Bob should have almost no entries initially (has {})",
		bob_entries_before
	);

	// Step 2: Start backfill on Bob while Alice continues indexing
	tracing::info!("Step 2: Starting Bob's backfill AND Alice's second indexing concurrently");

	// Use Spacedrive source code for deterministic testing across all environments
	let test_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();

	let backfill_future = harness.trigger_bob_backfill();
	let indexing_future = add_and_index_location(
		&harness.library_alice,
		test_path.to_str().unwrap(),
		"spacedrive",
	);

	// Run concurrently - this is the key to triggering the race
	let (backfill_result, indexing_result) = tokio::join!(backfill_future, indexing_future);

	if let Err(e) = backfill_result {
		tracing::warn!(error = %e, "Backfill had error (may be expected if racing)");
	}
	if let Err(e) = indexing_result {
		tracing::warn!(error = %e, "Indexing had error");
	}

	// Step 3: Wait for everything to stabilize
	tracing::info!("Step 3: Waiting for sync to stabilize");
	harness.wait_for_sync(Duration::from_secs(60)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Step 4: Compare results
	let entries_alice = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let entries_bob = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	let locations_alice = entities::location::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let locations_bob = entities::location::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_entries = entries_alice,
		bob_entries = entries_bob,
		alice_locations = locations_alice,
		bob_locations = locations_bob,
		"Final counts"
	);

	let diff = (entries_alice as i64 - entries_bob as i64).abs();

	if diff > 5 {
		tracing::error!(
			diff = diff,
			alice = entries_alice,
			bob = entries_bob,
			"RACE CONDITION CONFIRMED: Bob is missing {} entries!",
			diff
		);
	}

	assert!(
		diff <= 5,
		"Entry count mismatch: Alice has {}, Bob has {} (diff: {}). \
		This suggests watermarks advanced past unbackfilled entries.",
		entries_alice,
		entries_bob,
		diff
	);

	Ok(())
}

/// Test: Sequential indexing (control - should always pass)
#[tokio::test]
async fn test_sequential_backfill_control() -> anyhow::Result<()> {
	let harness = BackfillRaceHarness::new("sequential_control").await?;

	// Alice indexes both locations first
	// Use Spacedrive source code for deterministic testing
	let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let core_path = project_root.join("core");
	let apps_path = project_root.join("apps");

	tracing::info!("Indexing both locations on Alice first");

	add_and_index_location(&harness.library_alice, core_path.to_str().unwrap(), "core").await?;
	add_and_index_location(&harness.library_alice, apps_path.to_str().unwrap(), "apps").await?;

	let alice_entries = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;

	tracing::info!(entries = alice_entries, "Alice has all entries");

	// Now trigger backfill on Bob
	tracing::info!("Starting Bob's backfill (no concurrent indexing)");
	harness.trigger_bob_backfill().await?;

	// Wait for sync
	harness.wait_for_sync(Duration::from_secs(60)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Compare
	let entries_alice = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let entries_bob = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		alice = entries_alice,
		bob = entries_bob,
		"Final counts (sequential)"
	);

	let diff = (entries_alice as i64 - entries_bob as i64).abs();
	assert!(
		diff <= 5,
		"Sequential backfill should result in similar counts: Alice {}, Bob {} (diff: {})",
		entries_alice,
		entries_bob,
		diff
	);

	Ok(())
}
