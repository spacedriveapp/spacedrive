//! Common test harness and utilities for sync integration tests
//!
//! Provides reusable components to reduce duplication across sync tests.

use super::MockTransport;
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

/// Builder for creating common test configurations
pub struct TestConfigBuilder {
	data_dir: PathBuf,
	sync_log_filter: String,
}

impl TestConfigBuilder {
	pub fn new(data_dir: PathBuf) -> Self {
		Self {
			data_dir,
			sync_log_filter: "sd_core::service::sync=trace,\
				sd_core::service::network::protocol::sync=trace,\
				sd_core::infra::sync=trace,\
				sd_core::service::sync::peer=trace,\
				sd_core::service::sync::backfill=trace,\
				sd_core::infra::db::entities::entry=debug,\
				sd_core::infra::db::entities::device=debug,\
				sd_core::infra::db::entities::location=debug"
				.to_string(),
		}
	}

	#[allow(dead_code)]
	pub fn with_sync_filter(mut self, filter: impl Into<String>) -> Self {
		self.sync_log_filter = filter.into();
		self
	}

	pub fn build(self) -> anyhow::Result<sd_core::config::AppConfig> {
		let logging_config = sd_core::config::LoggingConfig {
			main_filter: "sd_core=info".to_string(),
			streams: vec![sd_core::config::LogStreamConfig {
				name: "sync".to_string(),
				file_name: "sync.log".to_string(),
				filter: self.sync_log_filter,
				enabled: true,
			}],
		};

		let config = sd_core::config::AppConfig {
			version: 4,
			logging: logging_config,
			data_dir: self.data_dir.clone(),
			log_level: "debug".to_string(),
			telemetry_enabled: false,
			preferences: sd_core::config::Preferences::default(),
			job_logging: sd_core::config::JobLoggingConfig::default(),
			services: sd_core::config::ServiceConfig {
				networking_enabled: false,
				volume_monitoring_enabled: false,
				fs_watcher_enabled: false,
				statistics_listener_enabled: false,
			},
		};

		config.save()?;
		Ok(config)
	}
}

/// Initialize tracing for a test
pub fn init_test_tracing(test_name: &str, snapshot_dir: &std::path::Path) -> anyhow::Result<()> {
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
				 sd_core::service::sync::backfill=debug,\
				 sd_core::service::sync::dependency=debug,\
				 sd_core::infra::sync=debug,\
				 sd_core::infra::db::entities=debug,\
				 helpers=trace",
			)
		}))
		.try_init();

	tracing::info!(
		snapshot_dir = %snapshot_dir.display(),
		"Initialized logging for {}",
		test_name
	);

	Ok(())
}

/// Register a device in a library's database
pub async fn register_device(
	library: &Arc<Library>,
	device_id: Uuid,
	device_name: &str,
) -> anyhow::Result<()> {
	use chrono::Utc;

	let device_model = entities::device::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(device_id),
		name: Set(device_name.to_string()),
		slug: Set(device_name.to_lowercase()),
		os: Set("Test OS".to_string()),
		os_version: Set(Some("1.0".to_string())),
		hardware_model: Set(None),
		cpu_model: Set(None),
		cpu_architecture: Set(None),
		cpu_cores_physical: Set(None),
		cpu_cores_logical: Set(None),
		cpu_frequency_mhz: Set(None),
		memory_total_bytes: Set(None),
		form_factor: Set(None),
		manufacturer: Set(None),
		gpu_models: Set(None),
		boot_disk_type: Set(None),
		boot_disk_capacity_bytes: Set(None),
		swap_total_bytes: Set(None),
		network_addresses: Set(serde_json::json!([])),
		is_online: Set(false),
		last_seen_at: Set(Utc::now()),
		capabilities: Set(serde_json::json!({})),
		created_at: Set(Utc::now()),
		updated_at: Set(Utc::now()),
		sync_enabled: Set(true),
		last_sync_at: Set(None),
	};

	// Check if device already exists
	let existing = entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device_id))
		.one(library.db().conn())
		.await?;

	if existing.is_none() {
		device_model.insert(library.db().conn()).await?;
	}

	Ok(())
}

/// Create a mock volume for testing
pub async fn create_test_volume(
	library: &Arc<Library>,
	device_id: Uuid,
	fingerprint: &str,
	display_name: &str,
) -> anyhow::Result<()> {
	use chrono::Utc;

	let volume_model = entities::volume::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Uuid::new_v4()),
		device_id: Set(device_id),
		fingerprint: Set(fingerprint.to_string()),
		display_name: Set(Some(display_name.to_string())),
		tracked_at: Set(Utc::now()),
		last_seen_at: Set(Utc::now()),
		is_online: Set(true),
		total_capacity: Set(Some(500_000_000_000)),     // 500GB
		available_capacity: Set(Some(250_000_000_000)), // 250GB available
		unique_bytes: Set(None),
		read_speed_mbps: Set(Some(500)),
		write_speed_mbps: Set(Some(400)),
		last_speed_test_at: Set(None),
		total_file_count: Set(None),
		total_directory_count: Set(None),
		last_indexed_at: Set(None),
		file_system: Set(Some("APFS".to_string())),
		mount_point: Set(Some("/Volumes/TestDrive".to_string())),
		is_removable: Set(Some(true)),
		is_network_drive: Set(Some(false)),
		device_model: Set(Some("SSD Model".to_string())),
		volume_type: Set(Some("External".to_string())),
		is_user_visible: Set(Some(true)),
		auto_track_eligible: Set(Some(true)),
		cloud_identifier: Set(None),
		cloud_config: Set(None),
	};

	volume_model.insert(library.db().conn()).await?;
	Ok(())
}

/// Set all devices in a library to "synced" state (prevents auto-backfill)
pub async fn set_all_devices_synced(library: &Arc<Library>) -> anyhow::Result<()> {
	use chrono::Utc;
	use sea_orm::ActiveValue;

	for device in entities::device::Entity::find()
		.all(library.db().conn())
		.await?
	{
		let mut active: entities::device::ActiveModel = device.into();
		active.last_sync_at = ActiveValue::Set(Some(Utc::now()));
		active.update(library.db().conn()).await?;
	}

	Ok(())
}

/// Wait for indexing to complete by monitoring job status
pub async fn wait_for_indexing(
	library: &Arc<Library>,
	_location_id: i32,
	timeout: Duration,
) -> anyhow::Result<()> {
	use sd_core::infra::job::JobStatus;

	let start_time = tokio::time::Instant::now();
	let mut job_seen = false;
	let mut last_entry_count = 0;
	let mut stable_iterations = 0;

	loop {
		let running_jobs = library.jobs().list_jobs(Some(JobStatus::Running)).await?;

		if !running_jobs.is_empty() {
			job_seen = true;
			tracing::debug!(
				running_count = running_jobs.len(),
				"Indexing jobs still running"
			);
		}

		let current_entries = entities::entry::Entity::find()
			.count(library.db().conn())
			.await?;

		let completed_jobs = library.jobs().list_jobs(Some(JobStatus::Completed)).await?;

		if job_seen && !completed_jobs.is_empty() && running_jobs.is_empty() && current_entries > 0
		{
			if current_entries == last_entry_count {
				stable_iterations += 1;
				if stable_iterations >= 3 {
					tracing::info!(
						total_entries = current_entries,
						"Indexing completed and stabilized"
					);
					return Ok(());
				}
			} else {
				stable_iterations = 0;
			}
			last_entry_count = current_entries;
		}

		let failed_jobs = library.jobs().list_jobs(Some(JobStatus::Failed)).await?;
		if !failed_jobs.is_empty() {
			anyhow::bail!("Indexing job failed");
		}

		if start_time.elapsed() > timeout {
			anyhow::bail!(
				"Indexing timeout after {:?} (entries: {})",
				timeout,
				current_entries
			);
		}

		tokio::time::sleep(Duration::from_millis(500)).await;
	}
}

/// Wait for sync to complete between two devices using the sophisticated stability algorithm
///
/// This waits for Alice to stabilize first (no new entries/content), then checks if Bob caught up.
/// This prevents false positives where counts match at intermediate states.
pub async fn wait_for_sync(
	library_alice: &Arc<Library>,
	library_bob: &Arc<Library>,
	max_duration: Duration,
) -> anyhow::Result<()> {
	let start = tokio::time::Instant::now();
	let mut last_alice_entries = 0;
	let mut last_alice_content = 0;
	let mut last_bob_entries = 0;
	let mut stable_iterations = 0;
	let mut no_progress_iterations = 0;
	let mut alice_stable_iterations = 0;

	while start.elapsed() < max_duration {
		let alice_entries = entities::entry::Entity::find()
			.count(library_alice.db().conn())
			.await?;
		let bob_entries = entities::entry::Entity::find()
			.count(library_bob.db().conn())
			.await?;

		let alice_content = entities::content_identity::Entity::find()
			.count(library_alice.db().conn())
			.await?;
		let bob_content = entities::content_identity::Entity::find()
			.count(library_bob.db().conn())
			.await?;

		// Check if Alice has stabilized
		if alice_entries == last_alice_entries && alice_content == last_alice_content {
			alice_stable_iterations += 1;
		} else {
			alice_stable_iterations = 0;
		}

		// Check if Bob is making progress
		if bob_entries == last_bob_entries {
			no_progress_iterations += 1;
			if no_progress_iterations >= 10 {
				tracing::warn!(
					bob_entries = bob_entries,
					alice_entries = alice_entries,
					"No progress for 10 iterations - likely stuck in dependency loop or slow processing"
				);
			}
		} else {
			no_progress_iterations = 0;
		}

		// Only check sync completion if Alice has stabilized first
		if alice_stable_iterations >= 5 {
			if alice_entries == bob_entries && alice_content == bob_content {
				stable_iterations += 1;
				if stable_iterations >= 5 {
					tracing::info!(
						duration_ms = start.elapsed().as_millis(),
						alice_entries = alice_entries,
						bob_entries = bob_entries,
						alice_content = alice_content,
						bob_content = bob_content,
						"Sync completed - Alice stable and Bob caught up"
					);
					return Ok(());
				}
			} else {
				stable_iterations = 0;
			}
		} else {
			stable_iterations = 0;
			tracing::debug!(
				alice_stable_iters = alice_stable_iterations,
				alice_entries = alice_entries,
				alice_content = alice_content,
				"Waiting for Alice to stabilize before checking sync"
			);
		}

		// If we're very close and making very slow/no progress, consider it good enough
		let entry_diff = (alice_entries as i64 - bob_entries as i64).abs();
		let content_diff = (alice_content as i64 - bob_content as i64).abs();

		if entry_diff <= 5 && content_diff <= 5 {
			if no_progress_iterations >= 10 {
				tracing::warn!(
					alice_entries = alice_entries,
					bob_entries = bob_entries,
					alice_content = alice_content,
					bob_content = bob_content,
					entry_diff = entry_diff,
					content_diff = content_diff,
					no_progress_iters = no_progress_iterations,
					"Stopping sync - within tolerance and minimal progress"
				);
				return Ok(());
			} else if start.elapsed() > Duration::from_secs(90) {
				tracing::warn!(
					alice_entries = alice_entries,
					bob_entries = bob_entries,
					entry_diff = entry_diff,
					content_diff = content_diff,
					elapsed_secs = start.elapsed().as_secs(),
					"Stopping sync - within tolerance after 90+ seconds"
				);
				return Ok(());
			}
		}

		last_alice_entries = alice_entries;
		last_alice_content = alice_content;
		last_bob_entries = bob_entries;

		tokio::time::sleep(Duration::from_millis(100)).await;
	}

	let alice_entries = entities::entry::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let bob_entries = entities::entry::Entity::find()
		.count(library_bob.db().conn())
		.await?;

	anyhow::bail!(
		"Sync timeout after {:?}. Alice: {} entries, Bob: {} entries",
		max_duration,
		alice_entries,
		bob_entries
	);
}

/// Add a location and wait for indexing to complete
pub async fn add_and_index_location(
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

	// Wait for indexing with 120s timeout
	wait_for_indexing(library, location_db_id, Duration::from_secs(120)).await?;

	tracing::info!(location_uuid = %location_uuid, "Location indexed");

	Ok(location_uuid)
}

/// Create a timestamped snapshot directory
pub async fn create_snapshot_dir(test_name: &str) -> anyhow::Result<PathBuf> {
	let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
	let test_root =
		std::path::PathBuf::from(home).join("Library/Application Support/spacedrive/sync_tests");

	let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
	let snapshot_dir = test_root
		.join("snapshots")
		.join(format!("{}_{}", test_name, timestamp));
	fs::create_dir_all(&snapshot_dir).await?;

	Ok(snapshot_dir)
}

/// Snapshot utilities for capturing test state
#[allow(dead_code)]
pub struct SnapshotCapture {
	snapshot_dir: PathBuf,
}

#[allow(dead_code)]
impl SnapshotCapture {
	pub fn new(snapshot_dir: PathBuf) -> Self {
		Self { snapshot_dir }
	}

	/// Copy a database file to the snapshot
	pub async fn copy_database(
		&self,
		library: &Arc<Library>,
		dest_subdir: &str,
		filename: &str,
	) -> anyhow::Result<()> {
		let src = library.path().join(filename);
		let dest_dir = self.snapshot_dir.join(dest_subdir);
		fs::create_dir_all(&dest_dir).await?;
		let dest = dest_dir.join(filename);

		if src.exists() {
			fs::copy(&src, &dest).await?;
		}

		Ok(())
	}

	/// Copy all log files from a library
	pub async fn copy_logs(&self, library: &Arc<Library>, dest_subdir: &str) -> anyhow::Result<()> {
		let logs_dir = library.path().join("logs");
		if !logs_dir.exists() {
			return Ok(());
		}

		let dest_logs_dir = self.snapshot_dir.join(dest_subdir).join("logs");
		fs::create_dir_all(&dest_logs_dir).await?;

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

	/// Write event log to JSON lines format
	pub async fn write_event_log(
		&self,
		events: &[Event],
		dest_subdir: &str,
		filename: &str,
	) -> anyhow::Result<()> {
		let dest_dir = self.snapshot_dir.join(dest_subdir);
		fs::create_dir_all(&dest_dir).await?;
		let dest = dest_dir.join(filename);

		let mut file = fs::File::create(&dest).await?;

		for event in events {
			let line = format!("{}\n", serde_json::to_string(event)?);
			file.write_all(line.as_bytes()).await?;
		}

		Ok(())
	}

	/// Write sync event log to JSON lines format
	pub async fn write_sync_event_log(
		&self,
		events: &[SyncEvent],
		dest_subdir: &str,
		filename: &str,
	) -> anyhow::Result<()> {
		let dest_dir = self.snapshot_dir.join(dest_subdir);
		fs::create_dir_all(&dest_dir).await?;
		let dest = dest_dir.join(filename);

		let mut file = fs::File::create(&dest).await?;

		for event in events {
			let line = format!("{}\n", serde_json::to_string(event)?);
			file.write_all(line.as_bytes()).await?;
		}

		Ok(())
	}

	/// Write a comprehensive summary markdown
	pub async fn write_summary(
		&self,
		test_name: &str,
		library_alice: &Arc<Library>,
		library_bob: &Arc<Library>,
		device_alice_id: Uuid,
		device_bob_id: Uuid,
		alice_events: usize,
		bob_events: usize,
		alice_sync_events: usize,
		bob_sync_events: usize,
	) -> anyhow::Result<()> {
		let summary_path = self.snapshot_dir.join("summary.md");
		let mut file = fs::File::create(&summary_path).await?;

		let entries_alice = entities::entry::Entity::find()
			.count(library_alice.db().conn())
			.await?;
		let entries_bob = entities::entry::Entity::find()
			.count(library_bob.db().conn())
			.await?;

		let content_ids_alice = entities::content_identity::Entity::find()
			.count(library_alice.db().conn())
			.await?;
		let content_ids_bob = entities::content_identity::Entity::find()
			.count(library_bob.db().conn())
			.await?;

		let alice_files_linked = entities::entry::Entity::find()
			.filter(entities::entry::Column::Kind.eq(0))
			.filter(entities::entry::Column::ContentId.is_not_null())
			.count(library_alice.db().conn())
			.await?;
		let bob_files_linked = entities::entry::Entity::find()
			.filter(entities::entry::Column::Kind.eq(0))
			.filter(entities::entry::Column::ContentId.is_not_null())
			.count(library_bob.db().conn())
			.await?;
		let alice_total_files = entities::entry::Entity::find()
			.filter(entities::entry::Column::Kind.eq(0))
			.count(library_alice.db().conn())
			.await?;
		let bob_total_files = entities::entry::Entity::find()
			.filter(entities::entry::Column::Kind.eq(0))
			.count(library_bob.db().conn())
			.await?;

		let alice_linkage_pct = if alice_total_files > 0 {
			(alice_files_linked * 100) / alice_total_files
		} else {
			0
		};
		let bob_linkage_pct = if bob_total_files > 0 {
			(bob_files_linked * 100) / bob_total_files
		} else {
			0
		};

		let summary = format!(
			r#"# Sync Test Snapshot: {}

**Timestamp**: {}
**Test**: {}

## Alice (Device {})
- Entries: {}
- Content Identities: {}
- Files with content_id: {}/{} ({}%)
- Events Captured: {}
- Sync Events Captured: {}

## Bob (Device {})
- Entries: {}
- Content Identities: {}
- Files with content_id: {}/{} ({}%)
- Events Captured: {}
- Sync Events Captured: {}

## Diff
- Entry difference: {}
- Content identity difference: {}

## Files
- `test.log` - Complete test execution log
- `alice/database.db` - Alice's main database
- `alice/sync.db` - Alice's sync coordination database
- `alice/events.log` - Alice's event bus events (JSON lines)
- `alice/sync_events.log` - Alice's sync event bus events (JSON lines)
- `alice/logs/` - Alice's library logs
- `bob/database.db` - Bob's main database
- `bob/sync.db` - Bob's sync coordination database
- `bob/events.log` - Bob's event bus events (JSON lines)
- `bob/sync_events.log` - Bob's sync event bus events (JSON lines)
- `bob/logs/` - Bob's library logs
"#,
			test_name,
			chrono::Utc::now().to_rfc3339(),
			test_name,
			device_alice_id,
			entries_alice,
			content_ids_alice,
			alice_files_linked,
			alice_total_files,
			alice_linkage_pct,
			alice_events,
			alice_sync_events,
			device_bob_id,
			entries_bob,
			content_ids_bob,
			bob_files_linked,
			bob_total_files,
			bob_linkage_pct,
			bob_events,
			bob_sync_events,
			entries_alice as i64 - entries_bob as i64,
			content_ids_alice as i64 - content_ids_bob as i64,
		);

		file.write_all(summary.as_bytes()).await?;

		Ok(())
	}
}

/// Builder for creating a two-device sync test harness
#[allow(dead_code)]
pub struct TwoDeviceHarnessBuilder {
	test_name: String,
	data_dir_alice: PathBuf,
	data_dir_bob: PathBuf,
	snapshot_dir: PathBuf,
	start_in_ready_state: bool,
	collect_events: bool,
	collect_sync_events: bool,
}

#[allow(dead_code)]
impl TwoDeviceHarnessBuilder {
	pub async fn new(test_name: impl Into<String>) -> anyhow::Result<Self> {
		let test_name = test_name.into();
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root = std::path::PathBuf::from(home)
			.join("Library/Application Support/spacedrive/sync_tests");

		let data_dir = test_root.join("data");
		fs::create_dir_all(&data_dir).await?;

		let temp_dir_alice = data_dir.join("alice");
		let temp_dir_bob = data_dir.join("bob");
		fs::create_dir_all(&temp_dir_alice).await?;
		fs::create_dir_all(&temp_dir_bob).await?;

		let snapshot_dir = create_snapshot_dir(&test_name).await?;

		Ok(Self {
			test_name,
			data_dir_alice: temp_dir_alice,
			data_dir_bob: temp_dir_bob,
			snapshot_dir,
			start_in_ready_state: true,
			collect_events: false,
			collect_sync_events: false,
		})
	}

	/// Start devices in Ready state (skip backfill)
	pub fn start_in_ready_state(mut self, ready: bool) -> Self {
		self.start_in_ready_state = ready;
		self
	}

	/// Collect main event bus events
	pub fn collect_events(mut self, collect: bool) -> Self {
		self.collect_events = collect;
		self
	}

	/// Collect sync event bus events
	pub fn collect_sync_events(mut self, collect: bool) -> Self {
		self.collect_sync_events = collect;
		self
	}

	pub async fn build(self) -> anyhow::Result<TwoDeviceHarness> {
		// Initialize tracing
		init_test_tracing(&self.test_name, &self.snapshot_dir)?;

		tracing::info!(
			snapshot_dir = %self.snapshot_dir.display(),
			alice_dir = %self.data_dir_alice.display(),
			bob_dir = %self.data_dir_bob.display(),
			"Test directories initialized"
		);

		// Create test configs
		TestConfigBuilder::new(self.data_dir_alice.clone()).build()?;
		TestConfigBuilder::new(self.data_dir_bob.clone()).build()?;

		// Initialize cores
		let core_alice = Core::new(self.data_dir_alice.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
		let device_alice_id = core_alice.device.device_id()?;

		let core_bob = Core::new(self.data_dir_bob.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
		let device_bob_id = core_bob.device.device_id()?;

		// Create libraries
		let library_alice = core_alice
			.libraries
			.create_library_no_sync("Test Library", None, core_alice.context.clone())
			.await?;

		let library_bob = core_bob
			.libraries
			.create_library_no_sync("Test Library", None, core_bob.context.clone())
			.await?;

		// Register devices in each other's libraries
		register_device(&library_alice, device_bob_id, "Bob").await?;
		register_device(&library_bob, device_alice_id, "Alice").await?;

		// Set last_sync_at to prevent auto-backfill
		set_all_devices_synced(&library_alice).await?;
		set_all_devices_synced(&library_bob).await?;

		tracing::info!(
			alice_device = %device_alice_id,
			bob_device = %device_bob_id,
			"Devices registered and pre-paired"
		);

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

		// Set state if requested
		if self.start_in_ready_state {
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

			tokio::time::sleep(Duration::from_millis(500)).await;

			tracing::info!("Both devices set to Ready state");
		}

		// Set up event collection if requested
		let event_log_alice = if self.collect_events {
			let log = Arc::new(Mutex::new(Vec::new()));
			start_event_collector(&library_alice, log.clone());
			Some(log)
		} else {
			None
		};

		let event_log_bob = if self.collect_events {
			let log = Arc::new(Mutex::new(Vec::new()));
			start_event_collector(&library_bob, log.clone());
			Some(log)
		} else {
			None
		};

		let sync_event_log_alice = if self.collect_sync_events {
			let log = Arc::new(Mutex::new(Vec::new()));
			start_sync_event_collector(&library_alice, log.clone());
			Some(log)
		} else {
			None
		};

		let sync_event_log_bob = if self.collect_sync_events {
			let log = Arc::new(Mutex::new(Vec::new()));
			start_sync_event_collector(&library_bob, log.clone());
			Some(log)
		} else {
			None
		};

		Ok(TwoDeviceHarness {
			data_dir_alice: self.data_dir_alice,
			data_dir_bob: self.data_dir_bob,
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
			snapshot_dir: self.snapshot_dir,
		})
	}
}

/// Two-device sync test harness
pub struct TwoDeviceHarness {
	pub data_dir_alice: PathBuf,
	pub data_dir_bob: PathBuf,
	pub core_alice: Core,
	pub core_bob: Core,
	pub library_alice: Arc<Library>,
	pub library_bob: Arc<Library>,
	pub device_alice_id: Uuid,
	pub device_bob_id: Uuid,
	pub transport_alice: Arc<MockTransport>,
	pub transport_bob: Arc<MockTransport>,
	pub event_log_alice: Option<Arc<Mutex<Vec<Event>>>>,
	pub event_log_bob: Option<Arc<Mutex<Vec<Event>>>>,
	pub sync_event_log_alice: Option<Arc<Mutex<Vec<SyncEvent>>>>,
	pub sync_event_log_bob: Option<Arc<Mutex<Vec<SyncEvent>>>>,
	pub snapshot_dir: PathBuf,
}

impl TwoDeviceHarness {
	/// Wait for sync to complete using the sophisticated algorithm
	pub async fn wait_for_sync(&self, max_duration: Duration) -> anyhow::Result<()> {
		wait_for_sync(&self.library_alice, &self.library_bob, max_duration).await
	}

	/// Add and index a location on Alice
	pub async fn add_and_index_location_alice(
		&self,
		path: &str,
		name: &str,
	) -> anyhow::Result<Uuid> {
		add_and_index_location(&self.library_alice, path, name).await
	}

	/// Add and index a location on Bob
	pub async fn add_and_index_location_bob(&self, path: &str, name: &str) -> anyhow::Result<Uuid> {
		add_and_index_location(&self.library_bob, path, name).await
	}

	/// Capture comprehensive snapshot
	pub async fn capture_snapshot(&self, scenario_name: &str) -> anyhow::Result<PathBuf> {
		let snapshot_path = self.snapshot_dir.join(scenario_name);
		fs::create_dir_all(&snapshot_path).await?;

		tracing::info!(
			scenario = scenario_name,
			path = %snapshot_path.display(),
			"Capturing snapshot"
		);

		let capture = SnapshotCapture::new(snapshot_path.clone());

		// Copy Alice's data
		capture
			.copy_database(&self.library_alice, "alice", "database.db")
			.await?;
		capture
			.copy_database(&self.library_alice, "alice", "sync.db")
			.await?;
		capture.copy_logs(&self.library_alice, "alice").await?;

		if let Some(events) = &self.event_log_alice {
			let events = events.lock().await;
			capture
				.write_event_log(&events, "alice", "events.log")
				.await?;
		}

		if let Some(sync_events) = &self.sync_event_log_alice {
			let events = sync_events.lock().await;
			capture
				.write_sync_event_log(&events, "alice", "sync_events.log")
				.await?;
		}

		// Copy Bob's data
		capture
			.copy_database(&self.library_bob, "bob", "database.db")
			.await?;
		capture
			.copy_database(&self.library_bob, "bob", "sync.db")
			.await?;
		capture.copy_logs(&self.library_bob, "bob").await?;

		if let Some(events) = &self.event_log_bob {
			let events = events.lock().await;
			capture
				.write_event_log(&events, "bob", "events.log")
				.await?;
		}

		if let Some(sync_events) = &self.sync_event_log_bob {
			let events = sync_events.lock().await;
			capture
				.write_sync_event_log(&events, "bob", "sync_events.log")
				.await?;
		}

		// Write summary
		let alice_events = self
			.event_log_alice
			.as_ref()
			.map(|e| e.blocking_lock().len())
			.unwrap_or(0);
		let bob_events = self
			.event_log_bob
			.as_ref()
			.map(|e| e.blocking_lock().len())
			.unwrap_or(0);
		let alice_sync_events = self
			.sync_event_log_alice
			.as_ref()
			.map(|e| e.blocking_lock().len())
			.unwrap_or(0);
		let bob_sync_events = self
			.sync_event_log_bob
			.as_ref()
			.map(|e| e.blocking_lock().len())
			.unwrap_or(0);

		capture
			.write_summary(
				scenario_name,
				&self.library_alice,
				&self.library_bob,
				self.device_alice_id,
				self.device_bob_id,
				alice_events,
				bob_events,
				alice_sync_events,
				bob_sync_events,
			)
			.await?;

		tracing::info!(
			snapshot_path = %snapshot_path.display(),
			"Snapshot captured"
		);

		Ok(snapshot_path)
	}
}

/// Start event collector for main event bus
#[allow(dead_code)]
fn start_event_collector(library: &Arc<Library>, event_log: Arc<Mutex<Vec<Event>>>) {
	let mut subscriber = library.event_bus().subscribe();

	tokio::spawn(async move {
		while let Ok(event) = subscriber.recv().await {
			match &event {
				Event::ResourceChanged { resource_type, .. }
				| Event::ResourceChangedBatch { resource_type, .. }
					if matches!(
						resource_type.as_str(),
						"entry" | "location" | "content_identity" | "device"
					) =>
				{
					event_log.lock().await.push(event);
				}
				Event::ResourceDeleted { resource_type, .. }
					if matches!(
						resource_type.as_str(),
						"entry" | "location" | "content_identity"
					) =>
				{
					event_log.lock().await.push(event);
				}
				Event::Custom { event_type, .. } if event_type == "sync_ready" => {
					event_log.lock().await.push(event);
				}
				_ => {}
			}
		}
	});
}

/// Start event collector for sync event bus
#[allow(dead_code)]
fn start_sync_event_collector(library: &Arc<Library>, sync_event_log: Arc<Mutex<Vec<SyncEvent>>>) {
	let sync_service = library
		.sync_service()
		.expect("Sync service not initialized");
	let mut subscriber = sync_service.peer_sync().sync_events().subscribe();

	tokio::spawn(async move {
		while let Ok(event) = subscriber.recv().await {
			sync_event_log.lock().await.push(event);
		}
	});
}
