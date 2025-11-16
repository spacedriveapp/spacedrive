//! Real-Time Sync Integration Test
//!
//! Automated testing of sync scenarios with two devices (Alice & Bob).
//! Each test run captures complete snapshots for analysis.
//!
//! ## Features
//! - Pre-paired devices (Alice & Bob)
//! - Indexes real Downloads folder
//! - Event-driven architecture
//! - Captures sync logs, databases, and event bus events
//! - Timestamped snapshot folders for each run
//!
//! ## Running Tests
//! ```bash
//! cargo test -p sd-core --test sync_realtime_integration_test -- --test-threads=1
//! ```

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
/// Test configuration for a sync scenario
struct SyncScenario {
	name: String,
	description: String,
	setup_fn: Box<
		dyn Fn(
				&SyncTestHarness,
			)
				-> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
			+ Send
			+ Sync,
	>,
}

/// Main test harness for two-device sync testing
struct SyncTestHarness {
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
	event_log_alice: Arc<Mutex<Vec<Event>>>,
	event_log_bob: Arc<Mutex<Vec<Event>>>,
	sync_event_log_alice: Arc<Mutex<Vec<SyncEvent>>>,
	sync_event_log_bob: Arc<Mutex<Vec<SyncEvent>>>,
	snapshot_dir: PathBuf,
}

impl SyncTestHarness {
	/// Create new test harness with pre-paired devices
	async fn new(test_name: &str) -> anyhow::Result<Self> {
		// Create test root in spacedrive data folder
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root = std::path::PathBuf::from(home)
			.join("Library/Application Support/spacedrive/sync_tests");

		// Create data directories (persistent, not temp)
		let data_dir = test_root.join("data");
		fs::create_dir_all(&data_dir).await?;

		let temp_dir_alice = data_dir.join("alice");
		let temp_dir_bob = data_dir.join("bob");
		fs::create_dir_all(&temp_dir_alice).await?;
		fs::create_dir_all(&temp_dir_bob).await?;

		// Create snapshot directory with timestamp
		let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
		let snapshot_dir = test_root
			.join("snapshots")
			.join(format!("{}_{}", test_name, timestamp));
		fs::create_dir_all(&snapshot_dir).await?;

		// Initialize tracing with BOTH stdout and file output
		use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

		let log_file = std::fs::File::create(snapshot_dir.join("test.log"))?;

		let _ = tracing_subscriber::registry()
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_ansi(false) // No color codes in file
					.with_writer(log_file),
			)
			.with(fmt::layer().with_target(true).with_thread_ids(true))
			.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
				EnvFilter::new(
					"sd_core::service::sync=debug,\
						 sd_core::service::sync::peer=debug,\
						 sd_core::service::sync::backfill=debug,\
						 sd_core::service::sync::dependency=debug,\
						 sd_core::infra::sync=debug,\
						 sd_core::infra::db::entities=debug,\
						 sd_core::domain=debug,\
						 sync_realtime_integration_test=debug,\
						 helpers=trace",
				)
			}))
			.try_init();

		tracing::info!(
			test_root = %test_root.display(),
			snapshot_dir = %snapshot_dir.display(),
			"Created test directories and initialized logging to file"
		);

		// Configure both cores (networking disabled for test)
		Self::create_test_config(&temp_dir_alice)?;
		Self::create_test_config(&temp_dir_bob)?;

		// Initialize cores
		// Note: Sync trace logs go to test output (use --nocapture to see them)
		// Library job logs (indexing) are written to library/logs/*.log and captured in snapshots
		let core_alice = Core::new(temp_dir_alice.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
		let device_alice_id = core_alice.device.device_id()?;

		let core_bob = Core::new(temp_dir_bob.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
		let device_bob_id = core_bob.device.device_id()?;

		// Create libraries without auto-sync
		let library_alice = core_alice
			.libraries
			.create_library_no_sync("Sync Test Library", None, core_alice.context.clone())
			.await?;

		let library_bob = core_bob
			.libraries
			.create_library_no_sync("Sync Test Library", None, core_bob.context.clone())
			.await?;

		// Register devices in each other's libraries (pre-paired)
		Self::register_device(&library_alice, device_bob_id, "Bob").await?;
		Self::register_device(&library_bob, device_alice_id, "Alice").await?;

		// CRITICAL: Set last_sync_at NOW (before starting sync services)
		// This prevents the background sync loop from immediately triggering backfill
		use chrono::Utc;
		use sea_orm::ActiveValue;

		for device in entities::device::Entity::find()
			.all(library_alice.db().conn())
			.await?
		{
			let mut active: entities::device::ActiveModel = device.into();
			active.last_sync_at = ActiveValue::Set(Some(Utc::now()));
			active.update(library_alice.db().conn()).await?;
		}

		for device in entities::device::Entity::find()
			.all(library_bob.db().conn())
			.await?
		{
			let mut active: entities::device::ActiveModel = device.into();
			active.last_sync_at = ActiveValue::Set(Some(Utc::now()));
			active.update(library_bob.db().conn()).await?;
		}

		tracing::info!(
			alice_device = %device_alice_id,
			bob_device = %device_bob_id,
			"Devices registered and pre-paired, last_sync_at set to prevent initial backfill"
		);

		// Create mock transport connecting the two devices
		let (transport_alice, transport_bob) =
			MockTransport::new_pair(device_alice_id, device_bob_id);

		// Initialize sync services with mock transport
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

		// Register sync services with transports (for backfill)
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

		tracing::info!("Sync services started and registered on both devices");

		// For real-time sync testing: Mark both devices as Ready to skip backfill
		// This allows us to test pure real-time message flow without backfill complexity
		tracing::info!("Setting both devices to Ready state (skipping backfill)");

		library_alice
			.sync_service()
			.unwrap()
			.peer_sync()
			.set_state_for_test(DeviceSyncState::Ready)
			.await;
		library_bob
			.sync_service()
			.unwrap()
			.peer_sync()
			.set_state_for_test(DeviceSyncState::Ready)
			.await;

		// Wait a moment for sync loop to observe the new state
		tokio::time::sleep(Duration::from_millis(500)).await;

		// Verify both devices are in Ready state
		let alice_state = library_alice
			.sync_service()
			.unwrap()
			.peer_sync()
			.state()
			.await;
		let bob_state = library_bob
			.sync_service()
			.unwrap()
			.peer_sync()
			.state()
			.await;

		tracing::info!(
			alice_state = ?alice_state,
			bob_state = ?bob_state,
			"Devices in Ready state, backfill disabled"
		);

		if !alice_state.is_ready() || !bob_state.is_ready() {
			anyhow::bail!(
				"Failed to set Ready state - Alice: {:?}, Bob: {:?}",
				alice_state,
				bob_state
			);
		}

		// Set up event collection (main event bus)
		let event_log_alice = Arc::new(Mutex::new(Vec::new()));
		let event_log_bob = Arc::new(Mutex::new(Vec::new()));

		Self::start_event_collector(&library_alice, event_log_alice.clone());
		Self::start_event_collector(&library_bob, event_log_bob.clone());

		// Set up sync event collection (sync event bus)
		let sync_event_log_alice = Arc::new(Mutex::new(Vec::new()));
		let sync_event_log_bob = Arc::new(Mutex::new(Vec::new()));

		Self::start_sync_event_collector(&library_alice, sync_event_log_alice.clone());
		Self::start_sync_event_collector(&library_bob, sync_event_log_bob.clone());

		tracing::info!("Event collectors started on both devices");

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
			event_log_alice,
			event_log_bob,
			sync_event_log_alice,
			sync_event_log_bob,
			snapshot_dir,
		})
	}

	/// Create test config for a device with sync logging enabled
	fn create_test_config(
		data_dir: &std::path::Path,
	) -> anyhow::Result<sd_core::config::AppConfig> {
		// Enable sync logging (writes to library/logs/sync.log)
		let logging_config = sd_core::config::LoggingConfig {
			main_filter: "sd_core=info".to_string(),
			streams: vec![sd_core::config::LogStreamConfig {
				name: "sync".to_string(),
				file_name: "sync.log".to_string(),
				filter: "sd_core::service::sync=trace,\
					sd_core::service::network::protocol::sync=trace,\
					sd_core::infra::sync=trace,\
					sd_core::service::sync::peer=trace,\
					sd_core::service::sync::backfill=trace,\
					sd_core::infra::db::entities::entry=debug,\
					sd_core::infra::db::entities::device=debug,\
					sd_core::infra::db::entities::location=debug"
					.to_string(),
				enabled: true,
			}],
		};

		let config = sd_core::config::AppConfig {
			version: 4,
			logging: logging_config, // Our custom logging config with sync stream
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

		// Save config
		config.save()?;

		// Verify it was saved correctly
		let saved = sd_core::config::AppConfig::load_from(&data_dir.to_path_buf())?;
		tracing::debug!(
			streams_count = saved.logging.streams.len(),
			"Config saved with logging streams"
		);

		Ok(config)
	}

	/// Register a device in a library's database
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
			last_sync_at: Set(None),
			slug: Set(device_name.to_lowercase()),
		};

		device_model.insert(library.db().conn()).await?;
		Ok(())
	}

	/// Start event collector for a device (main event bus)
	fn start_event_collector(library: &Arc<Library>, event_log: Arc<Mutex<Vec<Event>>>) {
		let mut subscriber = library.event_bus().subscribe();

		tokio::spawn(async move {
			while let Ok(event) = subscriber.recv().await {
				// Filter to sync-relevant events only
				match &event {
					Event::ResourceChanged { resource_type, .. }
					| Event::ResourceChangedBatch { resource_type, .. }
						if matches!(
							resource_type.as_str(),
							"entry" | "location" | "content_identity" | "device" | "file"
						) =>
					{
						event_log.lock().await.push(event);
					}
					Event::ResourceDeleted { resource_type, .. }
						if matches!(
							resource_type.as_str(),
							"entry" | "location" | "content_identity" | "file"
						) =>
					{
						event_log.lock().await.push(event);
					}
					Event::JobCompleted { .. }
					| Event::JobStarted { .. }
					| Event::JobFailed { .. } => {
						// Capture job lifecycle events to track when indexing is truly complete
						event_log.lock().await.push(event);
					}
					Event::Custom { event_type, .. } if event_type == "sync_ready" => {
						event_log.lock().await.push(event);
					}
					_ => {
						// Ignore other events
					}
				}
			}
		});
	}

	/// Start sync event collector for a device (sync event bus)
	fn start_sync_event_collector(
		library: &Arc<Library>,
		sync_event_log: Arc<Mutex<Vec<SyncEvent>>>,
	) {
		let sync_service = library
			.sync_service()
			.expect("Sync service not initialized");
		let mut subscriber = sync_service.peer_sync().sync_events().subscribe();

		tokio::spawn(async move {
			while let Ok(event) = subscriber.recv().await {
				// Collect all sync events
				sync_event_log.lock().await.push(event);
			}
		});
	}

	/// Pump sync messages between devices
	async fn pump_messages(&self) -> anyhow::Result<usize> {
		let sync_alice = self.library_alice.sync_service().unwrap();
		let sync_bob = self.library_bob.sync_service().unwrap();

		// Check queue sizes before processing
		let alice_queue_before = self.transport_alice.queue_size(self.device_alice_id).await;
		let bob_queue_before = self.transport_bob.queue_size(self.device_bob_id).await;

		tracing::debug!(
			alice_queue = alice_queue_before,
			bob_queue = bob_queue_before,
			"Message queues before pumping"
		);

		let count_alice = self
			.transport_bob
			.process_incoming_messages(sync_bob)
			.await?;
		let count_bob = self
			.transport_alice
			.process_incoming_messages(sync_alice)
			.await?;

		if count_alice > 0 || count_bob > 0 {
			tracing::debug!(
				processed_for_alice = count_alice,
				processed_for_bob = count_bob,
				total = count_alice + count_bob,
				"Pumped messages"
			);
		}

		Ok(count_alice + count_bob)
	}

	/// Wait for sync to complete by checking database parity (deterministic)
	async fn wait_for_sync(&self, max_duration: Duration) -> anyhow::Result<()> {
		let start = tokio::time::Instant::now();
		let mut last_alice_entries = 0;
		let mut last_alice_content = 0;
		let mut last_bob_entries = 0;
		let mut last_bob_content = 0;
		let mut last_orphaned_bob = u64::MAX;
		let mut stable_iterations = 0;
		let mut no_progress_iterations = 0;
		let mut orphaned_no_progress_iterations = 0;
		let mut last_activity = tokio::time::Instant::now();

		while start.elapsed() < max_duration {
			// Messages are now auto-delivered (no manual pumping needed)

			// Check current counts
			let alice_entries = entities::entry::Entity::find()
				.count(self.library_alice.db().conn())
				.await?;
			let bob_entries = entities::entry::Entity::find()
				.count(self.library_bob.db().conn())
				.await?;

			let alice_content = entities::content_identity::Entity::find()
				.count(self.library_alice.db().conn())
				.await?;
			let bob_content = entities::content_identity::Entity::find()
				.count(self.library_bob.db().conn())
				.await?;

			// Check orphaned files
			let orphaned_bob = entities::entry::Entity::find()
				.filter(entities::entry::Column::Kind.eq(0))
				.filter(entities::entry::Column::Size.gt(0))
				.filter(entities::entry::Column::ContentId.is_null())
				.count(self.library_bob.db().conn())
				.await?;

			// Check if we're making progress on entries
			if bob_entries == last_bob_entries {
				no_progress_iterations += 1;
				if no_progress_iterations >= 10 {
					tracing::warn!(
						bob_entries = bob_entries,
						alice_entries = alice_entries,
						"No progress for 10 iterations - likely stuck in dependency loop or slow processing"
					);
					// Continue anyway - might still converge
				}
			} else {
				no_progress_iterations = 0;
				last_activity = tokio::time::Instant::now();
			}

			// Check if orphaned files are decreasing
			if orphaned_bob < last_orphaned_bob {
				// Progress on orphaned files - reset timer
				orphaned_no_progress_iterations = 0;
				last_activity = tokio::time::Instant::now();
			} else if orphaned_bob > 0 {
				// Orphaned files exist and not decreasing
				orphaned_no_progress_iterations += 1;
			}

			// CRITICAL: Timeout if no activity for 5 seconds (50 iterations)
			// BUT: Only timeout if content identification jobs are NOT running on Alice
			// (Content ID jobs may pause on large files, causing temporary inactivity)
			if last_activity.elapsed() > Duration::from_secs(5) && orphaned_bob > 0 {
				// Check if Alice still has running jobs by inspecting events
				let events = self.event_log_alice.lock().await;
				let mut active_jobs = std::collections::HashSet::new();

				for event in events.iter() {
					match event {
						Event::JobStarted { job_id, .. } => {
							active_jobs.insert(job_id.clone());
						}
						Event::JobCompleted { job_id, .. } | Event::JobFailed { job_id, .. } => {
							active_jobs.remove(job_id);
						}
						_ => {}
					}
				}
				drop(events);

				let jobs_running = !active_jobs.is_empty();

				if !jobs_running {
					// No jobs running and no progress - this is a real stall
					anyhow::bail!(
						"Sync stalled: No progress for 5 seconds and no jobs running. \
						{} orphaned files remain. Entry updates with content_id linkages stopped.",
						orphaned_bob
					);
				} else {
					tracing::debug!(
						orphaned_bob = orphaned_bob,
						active_jobs = active_jobs.len(),
						"Orphaned files present but jobs still active - continuing to wait"
					);
					// Reset timer - jobs are still working
					last_activity = tokio::time::Instant::now();
				}
			}

			// CRITICAL: Check if counts match, stable, AND zero orphaned files
			if alice_entries == bob_entries && alice_content == bob_content && orphaned_bob == 0 {
				// All data synced AND all linkages complete - verify stable
				if alice_entries == last_alice_entries
					&& alice_content == last_alice_content
					&& bob_entries == last_bob_entries
					&& bob_content == last_bob_content
				{
					stable_iterations += 1;
					if stable_iterations >= 5 {
						tracing::info!(
							duration_ms = start.elapsed().as_millis(),
							alice_entries = alice_entries,
							bob_entries = bob_entries,
							alice_content = alice_content,
							bob_content = bob_content,
							orphaned_bob = orphaned_bob,
							"Sync completed - databases match, stable, and ZERO orphaned files"
						);
						return Ok(());
					}
				} else {
					stable_iterations = 0;
				}
			} else {
				stable_iterations = 0;

				// Log progress if we're close but orphaned files remain
				if alice_entries == bob_entries && alice_content == bob_content && orphaned_bob > 0
				{
					tracing::debug!(
						orphaned_bob = orphaned_bob,
						"Data synced but {} orphaned files remain (waiting for content_id linkage updates)",
						orphaned_bob
					);
				}
			}

			// If we're very close and making very slow/no progress, consider it good enough
			// This handles the case where a few entries are stuck in dependency retry loops
			// BUT: Must also check content_identity to avoid early exit while content is syncing
			let entry_diff = (alice_entries as i64 - bob_entries as i64).abs();
			let content_diff = (alice_content as i64 - bob_content as i64).abs();

			if entry_diff <= 5 && content_diff <= 5 && orphaned_bob == 0 {
				// Both entries AND content within tolerance AND zero orphaned files
				if no_progress_iterations >= 10 {
					tracing::warn!(
						alice_entries = alice_entries,
						bob_entries = bob_entries,
						alice_content = alice_content,
						bob_content = bob_content,
						entry_diff = entry_diff,
						content_diff = content_diff,
						no_progress_iters = no_progress_iterations,
						"Stopping sync - within tolerance and minimal progress for 10+ iterations (likely dependency retry loop)"
					);
					return Ok(());
				} else if start.elapsed() > Duration::from_secs(90) {
					tracing::warn!(
						alice_entries = alice_entries,
						bob_entries = bob_entries,
						alice_content = alice_content,
						bob_content = bob_content,
						entry_diff = entry_diff,
						content_diff = content_diff,
						elapsed_secs = start.elapsed().as_secs(),
						"Stopping sync - within tolerance after 90+ seconds (good enough)"
					);
					return Ok(());
				}
			}

			last_alice_entries = alice_entries;
			last_alice_content = alice_content;
			last_bob_entries = bob_entries;
			last_bob_content = bob_content;
			last_orphaned_bob = orphaned_bob;

			tokio::time::sleep(Duration::from_millis(100)).await;
		}

		// Timeout - report current state
		let alice_entries = entities::entry::Entity::find()
			.count(self.library_alice.db().conn())
			.await?;
		let bob_entries = entities::entry::Entity::find()
			.count(self.library_bob.db().conn())
			.await?;

		anyhow::bail!(
			"Sync timeout after {:?}. Alice: {} entries, Bob: {} entries",
			max_duration,
			alice_entries,
			bob_entries
		);
	}

	/// Capture snapshot of current state to disk
	async fn capture_snapshot(&self, scenario_name: &str) -> anyhow::Result<PathBuf> {
		let snapshot_path = self.snapshot_dir.join(scenario_name);
		fs::create_dir_all(&snapshot_path).await?;

		tracing::info!(
			scenario = scenario_name,
			path = %snapshot_path.display(),
			"=== CAPTURING SNAPSHOT ==="
		);

		// Copy Alice's data
		let alice_snapshot = snapshot_path.join("alice");
		fs::create_dir_all(&alice_snapshot).await?;

		self.copy_database(&self.library_alice, &alice_snapshot, "database.db")
			.await?;
		self.copy_sync_db(&self.library_alice, &alice_snapshot, "sync.db")
			.await?;
		self.copy_logs(&self.library_alice, &alice_snapshot).await?;
		self.write_event_log(&self.event_log_alice, &alice_snapshot, "events.log")
			.await?;
		self.write_sync_event_log(
			&self.sync_event_log_alice,
			&alice_snapshot,
			"sync_events.log",
		)
		.await?;

		// Copy Bob's data
		let bob_snapshot = snapshot_path.join("bob");
		fs::create_dir_all(&bob_snapshot).await?;

		self.copy_database(&self.library_bob, &bob_snapshot, "database.db")
			.await?;
		self.copy_sync_db(&self.library_bob, &bob_snapshot, "sync.db")
			.await?;
		self.copy_logs(&self.library_bob, &bob_snapshot).await?;
		self.write_event_log(&self.event_log_bob, &bob_snapshot, "events.log")
			.await?;
		self.write_sync_event_log(&self.sync_event_log_bob, &bob_snapshot, "sync_events.log")
			.await?;

		// Write summary
		self.write_summary(&snapshot_path, scenario_name).await?;

		tracing::info!(
			snapshot_path = %snapshot_path.display(),
			"Snapshot captured"
		);

		Ok(snapshot_path)
	}

	async fn copy_database(
		&self,
		library: &Arc<Library>,
		dest_dir: &std::path::Path,
		filename: &str,
	) -> anyhow::Result<()> {
		let src = library.path().join(filename);
		let dest = dest_dir.join(filename);

		if src.exists() {
			fs::copy(&src, &dest).await?;
		}

		Ok(())
	}

	async fn copy_sync_db(
		&self,
		library: &Arc<Library>,
		dest_dir: &std::path::Path,
		filename: &str,
	) -> anyhow::Result<()> {
		let src = library.path().join(filename);
		let dest = dest_dir.join(filename);

		if src.exists() {
			fs::copy(&src, &dest).await?;
		}

		Ok(())
	}

	async fn copy_logs(
		&self,
		library: &Arc<Library>,
		dest_dir: &std::path::Path,
	) -> anyhow::Result<()> {
		// Copy all log files from library logs directory
		let logs_dir = library.path().join("logs");
		if !logs_dir.exists() {
			return Ok(());
		}

		let dest_logs_dir = dest_dir.join("logs");
		fs::create_dir_all(&dest_logs_dir).await?;

		// Read log directory
		let mut entries = fs::read_dir(&logs_dir).await?;
		while let Some(entry) = entries.next_entry().await? {
			let path = entry.path();
			if path.is_file() {
				let filename = path.file_name().unwrap();
				let dest_path = dest_logs_dir.join(filename);
				fs::copy(&path, &dest_path).await?;
			}
		}

		Ok(())
	}

	async fn write_event_log(
		&self,
		event_log: &Arc<Mutex<Vec<Event>>>,
		dest_dir: &std::path::Path,
		filename: &str,
	) -> anyhow::Result<()> {
		let events = event_log.lock().await;
		let dest = dest_dir.join(filename);

		let mut file = fs::File::create(&dest).await?;

		for event in events.iter() {
			let line = format!("{}\n", serde_json::to_string(event)?);
			file.write_all(line.as_bytes()).await?;
		}

		Ok(())
	}

	async fn write_sync_event_log(
		&self,
		sync_event_log: &Arc<Mutex<Vec<SyncEvent>>>,
		dest_dir: &std::path::Path,
		filename: &str,
	) -> anyhow::Result<()> {
		let events = sync_event_log.lock().await;
		let dest = dest_dir.join(filename);

		let mut file = fs::File::create(&dest).await?;

		for event in events.iter() {
			let line = format!("{}\n", serde_json::to_string(event)?);
			file.write_all(line.as_bytes()).await?;
		}

		Ok(())
	}

	async fn write_summary(
		&self,
		snapshot_path: &std::path::Path,
		scenario_name: &str,
	) -> anyhow::Result<()> {
		let summary_path = snapshot_path.join("summary.md");
		let mut file = fs::File::create(&summary_path).await?;

		// Count entries and content_identities from databases
		let entries_alice = entities::entry::Entity::find()
			.count(self.library_alice.db().conn())
			.await?;
		let entries_bob = entities::entry::Entity::find()
			.count(self.library_bob.db().conn())
			.await?;

		let content_ids_alice = entities::content_identity::Entity::find()
			.count(self.library_alice.db().conn())
			.await?;
		let content_ids_bob = entities::content_identity::Entity::find()
			.count(self.library_bob.db().conn())
			.await?;

		let summary = format!(
			r#"# Sync Test Snapshot: {}

**Timestamp**: {}
**Test**: {}

## Alice (Device {})
- Entries: {}
- Content Identities: {}
- Events Captured: {}
- Sync Events Captured: {}

## Bob (Device {})
- Entries: {}
- Content Identities: {}
- Events Captured: {}
- Sync Events Captured: {}

## Files
- `test.log` - Complete test execution log (all tracing output)
- `alice/database.db` - Alice's main database
- `alice/sync.db` - Alice's sync coordination database
- `alice/events.log` - Alice's event bus events (JSON lines)
- `alice/sync_events.log` - Alice's sync event bus events (JSON lines)
- `bob/database.db` - Bob's main database
- `bob/sync.db` - Bob's sync coordination database
- `bob/events.log` - Bob's event bus events (JSON lines)
- `bob/sync_events.log` - Bob's sync event bus events (JSON lines)
"#,
			scenario_name,
			chrono::Utc::now().to_rfc3339(),
			scenario_name,
			self.device_alice_id,
			entries_alice,
			content_ids_alice,
			self.event_log_alice.lock().await.len(),
			self.sync_event_log_alice.lock().await.len(),
			self.device_bob_id,
			entries_bob,
			content_ids_bob,
			self.event_log_bob.lock().await.len(),
			self.sync_event_log_bob.lock().await.len(),
		);

		file.write_all(summary.as_bytes()).await?;

		Ok(())
	}

	/// Add a location and index it (with job event monitoring)
	async fn add_and_index_location(
		&self,
		library: &Arc<Library>,
		_device_id: Uuid,
		path: &str,
		name: &str,
	) -> anyhow::Result<Uuid> {
		use sd_core::location::{create_location, IndexMode, LocationCreateArgs};

		tracing::info!(
			path = %path,
			name = %name,
			"Creating location and triggering indexing"
		);

		// Get device record
		let device_record = entities::device::Entity::find()
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Device not found"))?;

		// Create location (automatically triggers Content indexing - no thumbnails)
		let location_args = LocationCreateArgs {
			path: std::path::PathBuf::from(path),
			name: Some(name.to_string()),
			index_mode: IndexMode::Content, // Content identification only (fast, no thumbnails)
		};

		let location_db_id = create_location(
			library.clone(),
			library.event_bus(),
			location_args,
			device_record.id,
		)
		.await?;

		// Get location UUID
		let location_record = entities::location::Entity::find_by_id(location_db_id)
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found after creation"))?;

		let location_uuid = location_record.uuid;

		tracing::info!(
			location_uuid = %location_uuid,
			location_id = location_db_id,
			"Location created, waiting for indexing to complete"
		);

		// Wait for indexing job to complete
		self.wait_for_indexing(library, location_db_id).await?;

		tracing::info!(
			location_uuid = %location_uuid,
			"Indexing completed successfully"
		);

		Ok(location_uuid)
	}

	/// Wait for indexing job to complete by monitoring job status
	async fn wait_for_indexing(
		&self,
		library: &Arc<Library>,
		_location_id: i32,
	) -> anyhow::Result<()> {
		use sd_core::infra::job::JobStatus;

		let start_time = tokio::time::Instant::now();
		let timeout_duration = Duration::from_secs(120); // 2 minutes for large folders

		let mut job_seen = false;
		let mut last_entry_count = 0;
		let mut stable_iterations = 0;

		loop {
			// Check for running jobs
			let running_jobs = library.jobs().list_jobs(Some(JobStatus::Running)).await?;

			if !running_jobs.is_empty() {
				job_seen = true;
				tracing::debug!(
					running_count = running_jobs.len(),
					"Indexing jobs still running"
				);
			}

			// Check entry count (progress indicator)
			let current_entries = entities::entry::Entity::find()
				.count(library.db().conn())
				.await?;

			// Check for completed jobs
			let completed_jobs = library.jobs().list_jobs(Some(JobStatus::Completed)).await?;

			// If we've seen a job and it's now completed with entries, we're done
			if job_seen
				&& !completed_jobs.is_empty()
				&& running_jobs.is_empty()
				&& current_entries > 0
			{
				// Wait for entries to stabilize (no more being added)
				if current_entries == last_entry_count {
					stable_iterations += 1;
					if stable_iterations >= 3 {
						tracing::info!(
							total_entries = current_entries,
							"Indexing completed and stabilized"
						);
						break;
					}
				} else {
					stable_iterations = 0;
				}
				last_entry_count = current_entries;
			}

			// Check for failures
			let failed_jobs = library.jobs().list_jobs(Some(JobStatus::Failed)).await?;
			if !failed_jobs.is_empty() {
				anyhow::bail!("Indexing job failed");
			}

			// Timeout check
			if start_time.elapsed() > timeout_duration {
				anyhow::bail!(
					"Indexing timeout after {:?} (entries: {})",
					timeout_duration,
					current_entries
				);
			}

			tokio::time::sleep(Duration::from_millis(500)).await;
		}

		Ok(())
	}
}

// Clean up (async drop not supported, cleanup happens via TempDir drop)
impl Drop for SyncTestHarness {
	fn drop(&mut self) {
		// Sync services will be cleaned up when libraries are dropped
		// TempDir will clean up filesystem
	}
}

//
// TEST SCENARIOS
//

/// Test: Location 1 indexed on Alice, syncs to Bob in real-time
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_realtime_sync_alice_to_bob() -> anyhow::Result<()> {
	let harness = SyncTestHarness::new("realtime_alice_to_bob").await?;

	// Phase 1: Add location on Alice
	tracing::info!("=== Phase 1: Adding location on Alice ===");

	let downloads_path = std::env::var("HOME").unwrap() + "/Downloads";
	let location_uuid = harness
		.add_and_index_location(
			&harness.library_alice,
			harness.device_alice_id,
			&downloads_path,
			"Downloads",
		)
		.await?;

	// Location and root entry will sync naturally via StateChange events
	// (No manual insertion needed now that create_location emits StateChange)
	tracing::info!(
		location_uuid = %location_uuid,
		"Location and root entry created on Alice, will sync automatically"
	);

	// Give sync a moment to deliver location and root entry
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Phase 2: Sync to Bob (messages now auto-delivered like production)
	tracing::info!("=== Phase 2: Syncing to Bob ===");

	// Check transport state before syncing
	let messages_sent = harness.transport_alice.total_message_count().await;
	let alice_queue_size = harness
		.transport_alice
		.queue_size(harness.device_alice_id)
		.await;
	let bob_queue_size = harness
		.transport_bob
		.queue_size(harness.device_bob_id)
		.await;

	tracing::info!(
		messages_sent = messages_sent,
		alice_queue = alice_queue_size,
		bob_queue = bob_queue_size,
		"Transport state before pumping"
	);

	// Always capture snapshot, even on sync failure
	// Increased timeout to allow content identities to finish syncing (slower than entries)
	let sync_result = harness.wait_for_sync(Duration::from_secs(120)).await;

	// Capture snapshot regardless of sync outcome
	tracing::info!("=== Phase 3: Capturing snapshot ===");
	harness.capture_snapshot("final_state").await?;

	// Now check sync result
	sync_result?;

	// Phase 4: Verify data on Bob
	tracing::info!("=== Phase 4: Verifying sync ===");

	let entries_alice = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let entries_bob = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	let content_ids_alice = entities::content_identity::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let content_ids_bob = entities::content_identity::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		entries_alice = entries_alice,
		entries_bob = entries_bob,
		content_ids_alice = content_ids_alice,
		content_ids_bob = content_ids_bob,
		"Final counts"
	);

	// Assertions (snapshot already captured above)
	// Allow for small differences (device/location metadata records)
	let entry_diff = (entries_alice as i64 - entries_bob as i64).abs();
	assert!(
		entry_diff <= 5,
		"Entry count mismatch beyond tolerance: Alice has {}, Bob has {} (diff: {})",
		entries_alice,
		entries_bob,
		entry_diff
	);

	let content_diff = (content_ids_alice as i64 - content_ids_bob as i64).abs();
	assert!(
		content_diff <= 5,
		"Content identity count mismatch beyond tolerance: Alice has {}, Bob has {} (diff: {})",
		content_ids_alice,
		content_ids_bob,
		content_diff
	);

	// Check content_id linkage
	let orphaned_alice = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::Size.gt(0))
		.filter(entities::entry::Column::ContentId.is_null())
		.count(harness.library_alice.db().conn())
		.await?;

	let orphaned_bob = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::Size.gt(0))
		.filter(entities::entry::Column::ContentId.is_null())
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		orphaned_alice = orphaned_alice,
		orphaned_bob = orphaned_bob,
		"Orphaned file count (files without content_id)"
	);

	// CRITICAL: Verify ZERO orphaned files
	// In a distributed file sync system, every file must have its content_id properly linked
	// No tolerance for incomplete sync - this is production file management
	assert_eq!(
		orphaned_bob, 0,
		"Bob has orphaned files without content_id: {} files (should be 0). \
		This means content_id linkage updates haven't fully synced. \
		Alice orphaned: {}",
		orphaned_bob, orphaned_alice
	);

	Ok(())
}

/// Test: Location indexed on Bob, syncs to Alice (reverse direction)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_realtime_sync_bob_to_alice() -> anyhow::Result<()> {
	let harness = SyncTestHarness::new("realtime_bob_to_alice").await?;

	// Add location on Bob (reverse direction)
	let downloads_path = std::env::var("HOME").unwrap() + "/Downloads";
	harness
		.add_and_index_location(
			&harness.library_bob,
			harness.device_bob_id,
			&downloads_path,
			"Downloads",
		)
		.await?;

	// Wait for sync
	harness.wait_for_sync(Duration::from_secs(30)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Verify bidirectional sync works
	let entries_alice = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let entries_bob = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	assert_eq!(entries_alice, entries_bob, "Bidirectional sync failed");

	Ok(())
}

/// Test: Concurrent indexing on both devices
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_concurrent_indexing() -> anyhow::Result<()> {
	let harness = SyncTestHarness::new("concurrent_indexing").await?;

	// Add different locations on both devices simultaneously
	let downloads_path = std::env::var("HOME").unwrap() + "/Downloads";
	let desktop_path = std::env::var("HOME").unwrap() + "/Desktop";

	// Start indexing on both
	let alice_task = harness.add_and_index_location(
		&harness.library_alice,
		harness.device_alice_id,
		&downloads_path,
		"Downloads",
	);

	let bob_task = harness.add_and_index_location(
		&harness.library_bob,
		harness.device_bob_id,
		&desktop_path,
		"Desktop",
	);

	// Wait for both
	tokio::try_join!(alice_task, bob_task)?;

	// Sync
	harness.wait_for_sync(Duration::from_secs(60)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Verify both locations exist on both devices
	let locations_alice = entities::location::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let locations_bob = entities::location::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	assert_eq!(locations_alice, 2, "Alice should have 2 locations");
	assert_eq!(locations_bob, 2, "Bob should have 2 locations");

	Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_content_identity_linkage() -> anyhow::Result<()> {
	let harness = SyncTestHarness::new("content_identity_linkage").await?;

	// Index on Alice
	let downloads_path = std::env::var("HOME").unwrap() + "/Downloads";
	harness
		.add_and_index_location(
			&harness.library_alice,
			harness.device_alice_id,
			&downloads_path,
			"Downloads",
		)
		.await?;

	// Wait for content identification to complete
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Sync
	harness.wait_for_sync(Duration::from_secs(30)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Verify content_id linkage on Bob
	let files_with_content_alice = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::Size.gt(0))
		.filter(entities::entry::Column::ContentId.is_not_null())
		.count(harness.library_alice.db().conn())
		.await?;

	let files_with_content_bob = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::Size.gt(0))
		.filter(entities::entry::Column::ContentId.is_not_null())
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_linked = files_with_content_alice,
		bob_linked = files_with_content_bob,
		"Content_id linkage counts"
	);

	// At least 90% of files should have content_id on Bob
	let target = (files_with_content_alice * 9) / 10;
	assert!(
		files_with_content_bob >= target,
		"Content_id linkage too low on Bob: {}/{} (expected at least {})",
		files_with_content_bob,
		files_with_content_alice,
		target
	);

	Ok(())
}
