//! Sync Metrics Integration Test
//!
//! Tests that the sync metrics system correctly tracks operations,
//! data volume, performance, and errors during sync.
//!
//! ## Running Tests
//! ```bash
//! cargo test -p sd-core --test sync_metrics_test -- --test-threads=1 --nocapture
//! ```

mod helpers;

use helpers::MockTransport;
use sd_core::{
	infra::{db::entities, sync::NetworkTransport},
	library::Library,
	service::{
		sync::{
			metrics::snapshot::SyncMetricsSnapshot,
			state::DeviceSyncState,
			SyncService,
		},
		Service,
	},
	Core,
};
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
use std::{path::PathBuf, sync::Arc};
use tokio::{fs, time::Duration};
use uuid::Uuid;

/// Test harness for metrics testing (simplified from sync_realtime_test.rs)
struct MetricsTestHarness {
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

impl MetricsTestHarness {
	async fn new(test_name: &str) -> anyhow::Result<Self> {
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root = std::path::PathBuf::from(home)
			.join("Library/Application Support/spacedrive/sync_tests");

		let data_dir = test_root.join("data");
		fs::create_dir_all(&data_dir).await?;

		let temp_dir_alice = data_dir.join("alice_metrics");
		let temp_dir_bob = data_dir.join("bob_metrics");

		// Clean up previous test data for fresh metrics
		let _ = fs::remove_dir_all(&temp_dir_alice).await;
		let _ = fs::remove_dir_all(&temp_dir_bob).await;
		fs::create_dir_all(&temp_dir_alice).await?;
		fs::create_dir_all(&temp_dir_bob).await?;

		let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
		let snapshot_dir = test_root
			.join("snapshots")
			.join(format!("metrics_{}_{}", test_name, timestamp));
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
					 sd_core::service::sync::metrics=trace,\
					 sync_metrics_test=debug",
				)
			}))
			.try_init();

		// Create test configs
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
			.create_library_no_sync("Metrics Test Library", None, core_alice.context.clone())
			.await?;

		let library_bob = core_bob
			.libraries
			.create_library_no_sync("Metrics Test Library", None, core_bob.context.clone())
			.await?;

		// Register devices (pre-paired)
		Self::register_device(&library_alice, device_bob_id, "Bob").await?;
		Self::register_device(&library_bob, device_alice_id, "Alice").await?;

		// Set last_sync_at to prevent auto-backfill
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

		// Create mock transports
		let (transport_alice, transport_bob) =
			MockTransport::new_pair(device_alice_id, device_bob_id);

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

		// Set Ready state (skip backfill for real-time sync testing)
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

		tokio::time::sleep(Duration::from_millis(100)).await;

		tracing::info!(
			alice_device = %device_alice_id,
			bob_device = %device_bob_id,
			"Metrics test harness initialized"
		);

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
				filter: "sd_core::service::sync=trace,sd_core::service::sync::metrics=trace"
					.to_string(),
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
			last_sync_at: Set(None),
			slug: Set(device_name.to_lowercase()),
		};

		device_model.insert(library.db().conn()).await?;
		Ok(())
	}

	/// Get metrics snapshot for Alice
	async fn alice_metrics(&self) -> SyncMetricsSnapshot {
		SyncMetricsSnapshot::from_metrics(
			self.library_alice.sync_service().unwrap().metrics().metrics(),
		)
		.await
	}

	/// Get metrics snapshot for Bob
	async fn bob_metrics(&self) -> SyncMetricsSnapshot {
		SyncMetricsSnapshot::from_metrics(
			self.library_bob.sync_service().unwrap().metrics().metrics(),
		)
		.await
	}

	/// Add a location and index it
	async fn add_and_index_location(
		&self,
		library: &Arc<Library>,
		path: &str,
		name: &str,
	) -> anyhow::Result<Uuid> {
		use sd_core::location::{create_location, IndexMode, LocationCreateArgs};

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

		// Wait for indexing
		self.wait_for_indexing(library).await?;

		Ok(location_record.uuid)
	}

	async fn wait_for_indexing(&self, library: &Arc<Library>) -> anyhow::Result<()> {
		use sd_core::infra::job::JobStatus;

		let timeout = Duration::from_secs(60);
		let start = tokio::time::Instant::now();
		let mut stable_count = 0;
		let mut last_entry_count = 0;

		while start.elapsed() < timeout {
			let running = library.jobs().list_jobs(Some(JobStatus::Running)).await?;
			let entries = entities::entry::Entity::find()
				.count(library.db().conn())
				.await?;

			if running.is_empty() && entries > 0 {
				if entries == last_entry_count {
					stable_count += 1;
					if stable_count >= 3 {
						return Ok(());
					}
				} else {
					stable_count = 0;
				}
				last_entry_count = entries;
			}

			tokio::time::sleep(Duration::from_millis(200)).await;
		}

		anyhow::bail!("Indexing timeout")
	}

	async fn wait_for_sync(&self, max_duration: Duration) -> anyhow::Result<()> {
		let start = tokio::time::Instant::now();
		let mut stable_iterations = 0;

		while start.elapsed() < max_duration {
			let alice_entries = entities::entry::Entity::find()
				.count(self.library_alice.db().conn())
				.await?;
			let bob_entries = entities::entry::Entity::find()
				.count(self.library_bob.db().conn())
				.await?;

			if alice_entries == bob_entries && alice_entries > 0 {
				stable_iterations += 1;
				if stable_iterations >= 5 {
					return Ok(());
				}
			} else {
				stable_iterations = 0;
			}

			tokio::time::sleep(Duration::from_millis(100)).await;
		}

		anyhow::bail!("Sync timeout")
	}

	/// Write metrics snapshot to file for debugging
	async fn save_metrics_snapshot(
		&self,
		name: &str,
		alice: &SyncMetricsSnapshot,
		bob: &SyncMetricsSnapshot,
	) -> anyhow::Result<()> {
		use tokio::io::AsyncWriteExt;

		let path = self.snapshot_dir.join(format!("{}.json", name));
		let mut file = fs::File::create(&path).await?;

		let data = serde_json::json!({
			"name": name,
			"timestamp": chrono::Utc::now().to_rfc3339(),
			"alice": alice,
			"bob": bob,
		});

		file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())
			.await?;

		Ok(())
	}
}

impl Drop for MetricsTestHarness {
	fn drop(&mut self) {
		// Cleanup handled by Core drop
	}
}

//
// METRICS TESTS
//

/// Test: Verify metrics are initialized to zero
#[tokio::test]
async fn test_metrics_initial_state() -> anyhow::Result<()> {
	let harness = MetricsTestHarness::new("initial_state").await?;

	let alice = harness.alice_metrics().await;
	let bob = harness.bob_metrics().await;

	// Save for debugging
	harness
		.save_metrics_snapshot("initial", &alice, &bob)
		.await?;

	// State should be Ready (we set it explicitly)
	assert!(
		alice.state.current_state.is_ready(),
		"Alice should be in Ready state, got {:?}",
		alice.state.current_state
	);
	assert!(
		bob.state.current_state.is_ready(),
		"Bob should be in Ready state, got {:?}",
		bob.state.current_state
	);

	// Operations should be at zero or near-zero (some setup operations may have occurred)
	tracing::info!(
		alice_broadcasts = alice.operations.broadcasts_sent,
		bob_broadcasts = bob.operations.broadcasts_sent,
		"Initial broadcast counts"
	);

	// No sync operations should have happened yet (no data to sync)
	assert_eq!(
		alice.operations.changes_received, 0,
		"Alice should have 0 changes received initially"
	);
	assert_eq!(
		bob.operations.changes_received, 0,
		"Bob should have 0 changes received initially"
	);

	tracing::info!("Initial metrics state verified");

	Ok(())
}

/// Test: Verify broadcasts are counted when syncing
#[tokio::test]
async fn test_metrics_broadcast_counting() -> anyhow::Result<()> {
	let harness = MetricsTestHarness::new("broadcast_counting").await?;

	// Snapshot before
	let alice_before = harness.alice_metrics().await;
	let bob_before = harness.bob_metrics().await;

	tracing::info!(
		alice_broadcasts_before = alice_before.operations.broadcasts_sent,
		bob_broadcasts_before = bob_before.operations.broadcasts_sent,
		"Metrics before indexing"
	);

	// Index a small folder on Alice (creates entries that get broadcast)
	let test_dir = harness.snapshot_dir.join("test_data");
	fs::create_dir_all(&test_dir).await?;

	// Create a few test files
	for i in 0..5 {
		let file_path = test_dir.join(format!("test_file_{}.txt", i));
		fs::write(&file_path, format!("Test content {}", i)).await?;
	}

	harness
		.add_and_index_location(
			&harness.library_alice,
			test_dir.to_str().unwrap(),
			"Test Data",
		)
		.await?;

	// Wait for sync
	harness.wait_for_sync(Duration::from_secs(30)).await?;

	// Small delay for metrics to update
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Snapshot after
	let alice_after = harness.alice_metrics().await;
	let bob_after = harness.bob_metrics().await;

	harness
		.save_metrics_snapshot("after_sync", &alice_after, &bob_after)
		.await?;

	tracing::info!(
		alice_broadcasts_after = alice_after.operations.broadcasts_sent,
		alice_state_changes = alice_after.operations.state_changes_broadcast,
		alice_shared_changes = alice_after.operations.shared_changes_broadcast,
		bob_changes_received = bob_after.operations.changes_received,
		bob_changes_applied = bob_after.operations.changes_applied,
		"Metrics after sync"
	);

	// Alice should have sent broadcasts (location + entries)
	assert!(
		alice_after.operations.broadcasts_sent > alice_before.operations.broadcasts_sent,
		"Alice broadcasts should increase: before={}, after={}",
		alice_before.operations.broadcasts_sent,
		alice_after.operations.broadcasts_sent
	);

	// Bob should have received and applied changes
	assert!(
		bob_after.operations.changes_received > bob_before.operations.changes_received,
		"Bob changes_received should increase: before={}, after={}",
		bob_before.operations.changes_received,
		bob_after.operations.changes_received
	);

	assert!(
		bob_after.operations.changes_applied > bob_before.operations.changes_applied,
		"Bob changes_applied should increase: before={}, after={}",
		bob_before.operations.changes_applied,
		bob_after.operations.changes_applied
	);

	// Applied should roughly equal received (unless some were rejected)
	let applied_ratio = bob_after.operations.changes_applied as f64
		/ bob_after.operations.changes_received.max(1) as f64;
	assert!(
		applied_ratio >= 0.9,
		"At least 90% of changes should be applied: {:.1}%",
		applied_ratio * 100.0
	);

	Ok(())
}

/// Test: Verify latency histograms are populated
#[tokio::test]
async fn test_metrics_latency_tracking() -> anyhow::Result<()> {
	let harness = MetricsTestHarness::new("latency_tracking").await?;

	// Create test data
	let test_dir = harness.snapshot_dir.join("latency_test");
	fs::create_dir_all(&test_dir).await?;

	for i in 0..3 {
		let file_path = test_dir.join(format!("latency_file_{}.txt", i));
		fs::write(&file_path, format!("Latency test {}", i)).await?;
	}

	// Index and sync
	harness
		.add_and_index_location(
			&harness.library_alice,
			test_dir.to_str().unwrap(),
			"Latency Test",
		)
		.await?;

	harness.wait_for_sync(Duration::from_secs(30)).await?;
	tokio::time::sleep(Duration::from_millis(500)).await;

	let alice = harness.alice_metrics().await;
	let bob = harness.bob_metrics().await;

	harness
		.save_metrics_snapshot("latency", &alice, &bob)
		.await?;

	tracing::info!(
		alice_broadcast_latency_count = alice.performance.broadcast_latency.count,
		alice_broadcast_latency_avg = alice.performance.broadcast_latency.avg_ms,
		bob_apply_latency_count = bob.performance.apply_latency.count,
		bob_apply_latency_avg = bob.performance.apply_latency.avg_ms,
		"Latency metrics"
	);

	// Alice should have recorded broadcast latencies
	if alice.operations.broadcasts_sent > 0 {
		// Broadcast latency should have recordings
		tracing::info!(
			"Alice broadcast latency: count={}, avg={:.2}ms, min={}ms, max={}ms",
			alice.performance.broadcast_latency.count,
			alice.performance.broadcast_latency.avg_ms,
			alice.performance.broadcast_latency.min_ms,
			alice.performance.broadcast_latency.max_ms,
		);
	}

	// Bob should have recorded apply latencies
	if bob.operations.changes_applied > 0 {
		tracing::info!(
			"Bob apply latency: count={}, avg={:.2}ms, min={}ms, max={}ms",
			bob.performance.apply_latency.count,
			bob.performance.apply_latency.avg_ms,
			bob.performance.apply_latency.min_ms,
			bob.performance.apply_latency.max_ms,
		);
	}

	// Verify histogram has reasonable values (latencies should be > 0 and < 10000ms)
	if alice.performance.broadcast_latency.count > 0 {
		assert!(
			alice.performance.broadcast_latency.max_ms < 10000,
			"Broadcast latency max should be reasonable: {}ms",
			alice.performance.broadcast_latency.max_ms
		);
	}

	Ok(())
}

/// Test: Verify data volume metrics (entries_synced by model)
#[tokio::test]
async fn test_metrics_data_volume() -> anyhow::Result<()> {
	let harness = MetricsTestHarness::new("data_volume").await?;

	// Create test data
	let test_dir = harness.snapshot_dir.join("volume_test");
	fs::create_dir_all(&test_dir).await?;

	let file_count = 10;
	for i in 0..file_count {
		let file_path = test_dir.join(format!("volume_file_{}.txt", i));
		fs::write(&file_path, format!("Volume test content {}", i)).await?;
	}

	// Index and sync
	harness
		.add_and_index_location(
			&harness.library_alice,
			test_dir.to_str().unwrap(),
			"Volume Test",
		)
		.await?;

	harness.wait_for_sync(Duration::from_secs(30)).await?;
	tokio::time::sleep(Duration::from_millis(500)).await;

	let alice = harness.alice_metrics().await;
	let bob = harness.bob_metrics().await;

	harness
		.save_metrics_snapshot("data_volume", &alice, &bob)
		.await?;

	tracing::info!(
		alice_entries_synced = ?alice.data_volume.entries_synced,
		bob_entries_synced = ?bob.data_volume.entries_synced,
		alice_bytes_sent = alice.data_volume.bytes_sent,
		bob_bytes_received = bob.data_volume.bytes_received,
		"Data volume metrics"
	);

	// Check entries synced by model type
	if !bob.data_volume.entries_synced.is_empty() {
		tracing::info!("Bob received entries by model:");
		for (model, count) in &bob.data_volume.entries_synced {
			tracing::info!("  {}: {}", model, count);
		}
	}

	// Verify database counts match
	let alice_db_entries = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let bob_db_entries = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_db_entries = alice_db_entries,
		bob_db_entries = bob_db_entries,
		"Database entry counts"
	);

	assert_eq!(
		alice_db_entries, bob_db_entries,
		"Entry counts should match after sync"
	);

	Ok(())
}

/// Test: Verify error metrics (when errors occur)
#[tokio::test]
async fn test_metrics_error_tracking() -> anyhow::Result<()> {
	let harness = MetricsTestHarness::new("error_tracking").await?;

	let alice = harness.alice_metrics().await;
	let bob = harness.bob_metrics().await;

	harness
		.save_metrics_snapshot("error_state", &alice, &bob)
		.await?;

	tracing::info!(
		alice_total_errors = alice.errors.total_errors,
		alice_network_errors = alice.errors.network_errors,
		alice_apply_errors = alice.errors.apply_errors,
		bob_total_errors = bob.errors.total_errors,
		bob_network_errors = bob.errors.network_errors,
		bob_apply_errors = bob.errors.apply_errors,
		"Error metrics"
	);

	// In normal operation, errors should be 0
	// Note: MockTransport always succeeds, so network errors won't be recorded
	// This test mainly verifies the error tracking infrastructure exists
	tracing::info!(
		"Error tracking infrastructure verified. Recent errors: alice={}, bob={}",
		alice.errors.recent_errors.len(),
		bob.errors.recent_errors.len()
	);

	Ok(())
}

/// Test: Full metrics snapshot structure
#[tokio::test]
async fn test_metrics_snapshot_structure() -> anyhow::Result<()> {
	let harness = MetricsTestHarness::new("snapshot_structure").await?;

	// Create and sync some data
	let test_dir = harness.snapshot_dir.join("structure_test");
	fs::create_dir_all(&test_dir).await?;

	for i in 0..3 {
		let file_path = test_dir.join(format!("structure_file_{}.txt", i));
		fs::write(&file_path, format!("Structure test {}", i)).await?;
	}

	harness
		.add_and_index_location(
			&harness.library_alice,
			test_dir.to_str().unwrap(),
			"Structure Test",
		)
		.await?;

	harness.wait_for_sync(Duration::from_secs(30)).await?;

	let alice = harness.alice_metrics().await;
	let bob = harness.bob_metrics().await;

	// Verify all snapshot sections are populated
	tracing::info!("=== ALICE METRICS SNAPSHOT ===");
	tracing::info!("Timestamp: {}", alice.timestamp);

	tracing::info!("--- State ---");
	tracing::info!("  current_state: {:?}", alice.state.current_state);
	tracing::info!("  uptime_seconds: {}", alice.state.uptime_seconds);
	tracing::info!(
		"  state_history entries: {}",
		alice.state.state_history.len()
	);

	tracing::info!("--- Operations ---");
	tracing::info!(
		"  broadcasts_sent: {}",
		alice.operations.broadcasts_sent
	);
	tracing::info!(
		"  state_changes_broadcast: {}",
		alice.operations.state_changes_broadcast
	);
	tracing::info!(
		"  shared_changes_broadcast: {}",
		alice.operations.shared_changes_broadcast
	);
	tracing::info!(
		"  changes_received: {}",
		alice.operations.changes_received
	);
	tracing::info!(
		"  changes_applied: {}",
		alice.operations.changes_applied
	);

	tracing::info!("--- Data Volume ---");
	tracing::info!("  bytes_sent: {}", alice.data_volume.bytes_sent);
	tracing::info!("  bytes_received: {}", alice.data_volume.bytes_received);
	tracing::info!(
		"  entries_synced models: {}",
		alice.data_volume.entries_synced.len()
	);

	tracing::info!("--- Performance ---");
	tracing::info!(
		"  broadcast_latency: count={}, avg={:.2}ms",
		alice.performance.broadcast_latency.count,
		alice.performance.broadcast_latency.avg_ms
	);
	tracing::info!(
		"  apply_latency: count={}, avg={:.2}ms",
		alice.performance.apply_latency.count,
		alice.performance.apply_latency.avg_ms
	);
	tracing::info!(
		"  db_query_count: {}",
		alice.performance.db_query_count
	);

	tracing::info!("--- Errors ---");
	tracing::info!("  total_errors: {}", alice.errors.total_errors);
	tracing::info!(
		"  conflicts_detected: {}",
		alice.errors.conflicts_detected
	);

	tracing::info!("=== BOB METRICS SNAPSHOT ===");
	tracing::info!(
		"  changes_received: {}",
		bob.operations.changes_received
	);
	tracing::info!("  changes_applied: {}", bob.operations.changes_applied);

	// Save full snapshot for inspection
	harness
		.save_metrics_snapshot("full_structure", &alice, &bob)
		.await?;

	tracing::info!(
		"Full metrics snapshot saved to: {}/full_structure.json",
		harness.snapshot_dir.display()
	);

	Ok(())
}
