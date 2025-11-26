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

use helpers::MockTransport;
use sd_core::{
	infra::{
		db::entities,
		event::Event,
		sync::{NetworkTransport, SyncEvent},
	},
	library::Library,
	service::{sync::state::DeviceSyncState, Service},
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use std::{path::PathBuf, sync::Arc};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex, time::Duration};
use uuid::Uuid;

/// Test harness for backfill race condition testing
struct BackfillRaceHarness {
	data_dir_alice: PathBuf,
	data_dir_bob: PathBuf,
	core_alice: Core,
	core_bob: Core,
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
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root = std::path::PathBuf::from(home)
			.join("Library/Application Support/spacedrive/sync_tests");

		// Clean up previous test data for fresh start
		let data_dir = test_root.join("data_backfill_race");
		if data_dir.exists() {
			fs::remove_dir_all(&data_dir).await?;
		}
		fs::create_dir_all(&data_dir).await?;

		let temp_dir_alice = data_dir.join("alice");
		let temp_dir_bob = data_dir.join("bob");
		fs::create_dir_all(&temp_dir_alice).await?;
		fs::create_dir_all(&temp_dir_bob).await?;

		// Create snapshot directory
		let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
		let snapshot_dir = test_root
			.join("snapshots")
			.join(format!("{}_{}", test_name, timestamp));
		fs::create_dir_all(&snapshot_dir).await?;

		// Initialize tracing
		use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

		let log_file = std::fs::File::create(snapshot_dir.join("test.log"))?;

		let _ = tracing_subscriber::registry()
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_ansi(false)
					.with_writer(log_file),
			)
			.with(fmt::layer().with_target(true).with_thread_ids(true))
			.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
				EnvFilter::new(
					"sd_core::service::sync=debug,\
						 sd_core::service::sync::peer=debug,\
						 sd_core::service::sync::backfill=info,\
						 sd_core::infra::sync=debug,\
						 sync_backfill_race_test=debug,\
						 helpers=trace",
				)
			}))
			.try_init();

		tracing::info!(
			snapshot_dir = %snapshot_dir.display(),
			"Starting backfill race condition test"
		);

		// Configure cores
		Self::create_test_config(&temp_dir_alice)?;
		Self::create_test_config(&temp_dir_bob)?;

		// Initialize cores
		let core_alice = Core::new(temp_dir_alice.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
		let device_alice_id = core_alice.device.device_id()?;

		let core_bob = Core::new(temp_dir_bob.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
		let device_bob_id = core_bob.device.device_id()?;

		// Create libraries
		let library_alice = core_alice
			.libraries
			.create_library_no_sync("Backfill Race Test", None, core_alice.context.clone())
			.await?;

		let library_bob = core_bob
			.libraries
			.create_library_no_sync("Backfill Race Test", None, core_bob.context.clone())
			.await?;

		// Register devices in each other's libraries
		Self::register_device(&library_alice, device_bob_id, "Bob").await?;
		Self::register_device(&library_bob, device_alice_id, "Alice").await?;

		// IMPORTANT: Set Alice's last_sync_at to NOW (she's "synced")
		// But leave Bob's last_sync_at as None (he needs to backfill)
		use chrono::Utc;
		use sea_orm::ActiveValue;

		// Set Alice's device record to "synced"
		let alice_device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_alice_id))
			.one(library_alice.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Alice device not found"))?;
		let mut alice_active: entities::device::ActiveModel = alice_device.into();
		alice_active.last_sync_at = ActiveValue::Set(Some(Utc::now()));
		alice_active.update(library_alice.db().conn()).await?;

		// Bob's last_sync_at stays None - he needs to backfill!
		// (This is the default from register_device)

		tracing::info!(
			alice_device = %device_alice_id,
			bob_device = %device_bob_id,
			"Devices registered - Alice synced, Bob needs backfill"
		);

		// Create mock transports
		let (transport_alice, transport_bob) =
			MockTransport::new_pair(device_alice_id, device_bob_id);

		// CRITICAL: Block Bob from receiving messages BEFORE starting services
		// This prevents the automatic backfill from running
		transport_alice.block_device(device_bob_id).await;
		transport_bob.block_device(device_bob_id).await;

		tracing::info!("Bob blocked from receiving messages");

		// Initialize sync services
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

		// Register sync services with transports
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

		// Start sync services
		library_alice.sync_service().unwrap().start().await?;
		library_bob.sync_service().unwrap().start().await?;

		// Set Alice to Ready (she's the "synced" device)
		library_alice
			.sync_service()
			.unwrap()
			.peer_sync()
			.set_state_for_test(DeviceSyncState::Ready)
			.await;

		// Bob stays in whatever state the sync service put him in
		// (should be Uninitialized since messages are blocked)
		let bob_state = library_bob
			.sync_service()
			.unwrap()
			.peer_sync()
			.state()
			.await;

		tracing::info!(
			alice_state = ?DeviceSyncState::Ready,
			bob_state = ?bob_state,
			bob_blocked = true,
			"Initial states set - Alice ready, Bob {:?} (BLOCKED)",
			bob_state
		);

		// Small delay to let background tasks settle
		tokio::time::sleep(Duration::from_millis(100)).await;

		Ok(Self {
			data_dir_alice: temp_dir_alice,
			data_dir_bob: temp_dir_bob,
			core_alice,
			core_bob,
			library_alice,
			library_bob,
			device_alice_id,
			device_bob_id,
			transport_alice,
			transport_bob,
			snapshot_dir,
		})
	}

	fn create_test_config(
		data_dir: &std::path::Path,
	) -> anyhow::Result<sd_core::config::AppConfig> {
		let logging_config = sd_core::config::LoggingConfig {
			main_filter: "sd_core=info".to_string(),
			streams: vec![sd_core::config::LogStreamConfig {
				name: "sync".to_string(),
				file_name: "sync.log".to_string(),
				filter: "sd_core::service::sync=trace,sd_core::infra::sync=trace".to_string(),
				enabled: true,
			}],
		};

		let config = sd_core::config::AppConfig {
			version: 4,
			logging: logging_config,
			data_dir: data_dir.to_path_buf(),
			log_level: "debug".to_string(),
			telemetry_enabled: false,
			preferences: sd_core::config::Preferences::default(),
			job_logging: sd_core::config::JobLoggingConfig::default(),
			services: sd_core::config::ServiceConfig {
				networking_enabled: false,
				volume_monitoring_enabled: false,
				location_watcher_enabled: false,
			},
		};

		config.save()?;
		Ok(config)
	}

	async fn register_device(
		library: &Arc<Library>,
		device_id: Uuid,
		device_name: &str,
	) -> anyhow::Result<()> {
		use chrono::Utc;

		let device_model = entities::device::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(device_id),
			name: Set(device_name.to_string()),
			os: Set("Test OS".to_string()),
			os_version: Set(Some("1.0".to_string())),
			hardware_model: Set(None),
			network_addresses: Set(serde_json::json!([])),
			is_online: Set(false),
			last_seen_at: Set(Utc::now()),
			capabilities: Set(serde_json::json!({})),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
			sync_enabled: Set(true),
			last_sync_at: Set(None), // Not synced yet!
			slug: Set(device_name.to_lowercase()),
		};

		device_model.insert(library.db().conn()).await?;
		Ok(())
	}

	/// Add and index a location, waiting for completion
	async fn add_and_index_location(
		&self,
		library: &Arc<Library>,
		path: &str,
		name: &str,
	) -> anyhow::Result<Uuid> {
		use sd_core::location::{create_location, IndexMode, LocationCreateArgs};

		tracing::info!(path = %path, name = %name, "Creating location and indexing");

		let device_record = entities::device::Entity::find()
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Device not found"))?;

		let location_args = LocationCreateArgs {
			path: std::path::PathBuf::from(path),
			name: Some(name.to_string()),
			index_mode: IndexMode::Content,
		};

		let location_db_id = create_location(
			library.clone(),
			library.event_bus(),
			location_args,
			device_record.id,
		)
		.await?;

		let location_record = entities::location::Entity::find_by_id(location_db_id)
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found"))?;

		let location_uuid = location_record.uuid;

		// Wait for indexing
		self.wait_for_indexing(library, location_db_id).await?;

		tracing::info!(location_uuid = %location_uuid, "Location indexed");

		Ok(location_uuid)
	}

	async fn wait_for_indexing(
		&self,
		library: &Arc<Library>,
		_location_id: i32,
	) -> anyhow::Result<()> {
		let mut last_count = 0u64;
		let mut stable_iterations = 0;
		let start = std::time::Instant::now();

		loop {
			tokio::time::sleep(Duration::from_millis(500)).await;

			// Count all entries (simpler than filtering by location)
			let count = entities::entry::Entity::find()
				.count(library.db().conn())
				.await?;

			if count == last_count && count > 0 {
				stable_iterations += 1;
				if stable_iterations >= 4 {
					tracing::info!(entries = count, "Indexing stable");
					return Ok(());
				}
			} else {
				stable_iterations = 0;
				last_count = count;
			}

			if start.elapsed() > Duration::from_secs(120) {
				anyhow::bail!("Indexing timed out after 120s");
			}
		}
	}

	/// Trigger backfill on Bob (also unblocks Bob)
	async fn trigger_bob_backfill(&self) -> anyhow::Result<()> {
		let sync_service = self.library_bob.sync_service().unwrap();
		let peer_sync = sync_service.peer_sync();

		// Unblock Bob so he can receive messages now
		tracing::info!("Unblocking Bob for backfill");
		self.transport_alice.unblock_device(self.device_bob_id).await;
		self.transport_bob.unblock_device(self.device_bob_id).await;

		tracing::info!("Triggering backfill on Bob");

		// Set Bob to Backfilling state
		peer_sync
			.set_state_for_test(DeviceSyncState::Backfilling {
				peer: self.device_alice_id,
				progress: 0,
			})
			.await;

		// Get the backfill manager and start backfill
		let backfill_manager = sync_service.backfill_manager();

		// Create peer info for Alice
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

		// Copy databases
		let alice_dir = snapshot_path.join("alice");
		let bob_dir = snapshot_path.join("bob");
		fs::create_dir_all(&alice_dir).await?;
		fs::create_dir_all(&bob_dir).await?;

		// Copy main databases
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
///
/// This test validates whether live events during backfill can cause data loss.
#[tokio::test]
async fn test_backfill_with_concurrent_indexing() -> anyhow::Result<()> {
	let harness = BackfillRaceHarness::new("backfill_race").await?;

	// Step 1: Alice indexes first location (data that Bob will backfill)
	let downloads_path = std::env::var("HOME").unwrap() + "/Downloads";
	tracing::info!("Step 1: Alice indexes Downloads");

	harness
		.add_and_index_location(&harness.library_alice, &downloads_path, "Downloads")
		.await?;

	let alice_entries_after_loc1 = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;

	tracing::info!(
		entries = alice_entries_after_loc1,
		"Alice has entries after first location"
	);

	// Verify Bob has very few entries (may have some from test setup)
	let bob_entries_before = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		bob_entries_before = bob_entries_before,
		alice_entries_after_loc1 = alice_entries_after_loc1,
		"Entry counts before concurrent phase"
	);

	// Bob should have far fewer entries than Alice at this point
	assert!(
		bob_entries_before < 10,
		"Bob should have almost no entries initially (has {})",
		bob_entries_before
	);

	// Step 2: Start backfill on Bob while Alice continues indexing
	// We'll run these concurrently to create the race condition
	tracing::info!("Step 2: Starting Bob's backfill AND Alice's second indexing concurrently");

	let desktop_path = std::env::var("HOME").unwrap() + "/Desktop";

	// Run backfill and indexing concurrently
	// The backfill will request data from Alice
	// Meanwhile, Alice will be creating new entries that get broadcast as live events
	let backfill_future = harness.trigger_bob_backfill();
	let indexing_future = harness.add_and_index_location(
		&harness.library_alice,
		&desktop_path,
		"Desktop",
	);

	// Start both concurrently - this is the key to triggering the race
	let (backfill_result, indexing_result) = tokio::join!(backfill_future, indexing_future);

	// Check results
	if let Err(e) = backfill_result {
		tracing::warn!(error = %e, "Backfill had error (may be expected if racing)");
	}
	if let Err(e) = indexing_result {
		tracing::warn!(error = %e, "Indexing had error");
	}

	// Step 4: Wait for everything to stabilize
	tracing::info!("Step 4: Waiting for sync to stabilize");
	harness.wait_for_sync(Duration::from_secs(60)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Step 5: Compare results
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

	// The critical assertion: Bob should have ALL entries
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

	// For now, just log the results - we're testing the hypothesis
	// If the test consistently shows a diff, the bug is confirmed
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
///
/// This test indexes on Alice first, THEN triggers backfill on Bob.
/// No concurrent activity, so no race condition should occur.
#[tokio::test]
async fn test_sequential_backfill_control() -> anyhow::Result<()> {
	let harness = BackfillRaceHarness::new("sequential_control").await?;

	// Alice indexes both locations first
	let downloads_path = std::env::var("HOME").unwrap() + "/Downloads";
	let desktop_path = std::env::var("HOME").unwrap() + "/Desktop";

	tracing::info!("Indexing both locations on Alice first");

	harness
		.add_and_index_location(&harness.library_alice, &downloads_path, "Downloads")
		.await?;

	harness
		.add_and_index_location(&harness.library_alice, &desktop_path, "Desktop")
		.await?;

	let alice_entries = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;

	tracing::info!(entries = alice_entries, "Alice has all entries");

	// Now trigger backfill on Bob (no concurrent activity)
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

	assert_eq!(
		entries_alice, entries_bob,
		"Sequential backfill should result in equal counts"
	);

	Ok(())
}
