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
use tokio::{fs, sync::Mutex, time::Duration};
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
				sd_core::infra::db::entities::device=debug,\
				sd_core::infra::db::entities::space=debug"
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
			proxy_pairing: sd_core::config::app_config::ProxyPairingConfig::default(),
			mounts: sd_core::config::app_config::MountsConfig::default(),
			replication: sd_core::config::app_config::ReplicationConfig::default(),
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
) -> anyhow::Result<Uuid> {
	use chrono::Utc;

	let volume_uuid = Uuid::new_v4();

	let volume_model = entities::volume::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(volume_uuid),
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
	Ok(volume_uuid)
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

/// Builder for creating a two-device sync test harness
#[allow(dead_code)]
pub struct TwoDeviceHarnessBuilder {
	test_name: String,
	test_data: super::TestDataDir,
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
		let test_name_str = test_name.into();

		// Use TestDataDir for proper temp directory management
		let test_data = super::TestDataDir::new(&test_name_str)?;
		let test_root = test_data.path().to_path_buf();

		let data_dir = test_root.join("data");
		fs::create_dir_all(&data_dir).await?;

		let temp_dir_alice = data_dir.join("alice");
		let temp_dir_bob = data_dir.join("bob");
		fs::create_dir_all(&temp_dir_alice).await?;
		fs::create_dir_all(&temp_dir_bob).await?;

		let snapshot_dir = test_root.join("snapshots");
		fs::create_dir_all(&snapshot_dir).await?;

		Ok(Self {
			test_name: test_name_str,
			test_data,
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

		// Initialize volume managers for both cores
		tracing::info!("Initializing volume managers");
		core_alice
			.volumes
			.initialize()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to initialize Alice volume manager: {}", e))?;
		core_bob
			.volumes
			.initialize()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to initialize Bob volume manager: {}", e))?;

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
			test_data: self.test_data,
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
	test_data: super::TestDataDir,
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
	/// Get access to the snapshot manager (if snapshots enabled via SD_TEST_SNAPSHOTS=1)
	pub fn snapshot_manager(&self) -> Option<&super::SnapshotManager> {
		self.test_data.snapshot_manager()
	}

	/// Capture snapshot with label (convenience method)
	pub async fn capture_snapshot(&self, label: &str) -> anyhow::Result<()> {
		if let Some(manager) = self.snapshot_manager() {
			manager.capture(label).await?;
		}
		Ok(())
	}

	/// Create spaces on Alice through the sync path. Spaces are a shared model,
	/// so they give the engine real traffic to carry now that indexing writes
	/// nothing the library syncs.
	pub async fn create_spaces_alice(&self, count: usize) -> anyhow::Result<Vec<Uuid>> {
		let mut created = Vec::with_capacity(count);
		for index in 0..count {
			let now = chrono::Utc::now();
			let model = entities::space::ActiveModel {
				id: sea_orm::NotSet,
				uuid: Set(Uuid::new_v4()),
				name: Set(format!("Space {index}")),
				icon: Set("Folder".to_string()),
				color: Set("#3B82F6".to_string()),
				order: Set(index as i32),
				created_at: Set(now.into()),
				updated_at: Set(now.into()),
			}
			.insert(self.library_alice.db().conn())
			.await?;
			self.library_alice
				.sync_model(&model, sd_core::infra::sync::ChangeType::Insert)
				.await?;
			created.push(model.uuid);
		}
		Ok(created)
	}

	/// Wait until Bob holds every given space.
	pub async fn wait_for_spaces_on_bob(
		&self,
		uuids: &[Uuid],
		max_duration: Duration,
	) -> anyhow::Result<()> {
		let deadline = tokio::time::Instant::now() + max_duration;
		loop {
			let present = entities::space::Entity::find()
				.filter(entities::space::Column::Uuid.is_in(uuids.to_vec()))
				.count(self.library_bob.db().conn())
				.await? as usize;
			if present == uuids.len() {
				return Ok(());
			}
			if tokio::time::Instant::now() >= deadline {
				anyhow::bail!(
					"Bob holds {present} of {} spaces after {max_duration:?}",
					uuids.len()
				);
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
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
						"space" | "space_group" | "space_item" | "volume" | "device"
					) =>
				{
					event_log.lock().await.push(event);
				}
				Event::ResourceDeleted { resource_type, .. }
					if matches!(
						resource_type.as_str(),
						"space" | "space_group" | "space_item" | "volume"
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
