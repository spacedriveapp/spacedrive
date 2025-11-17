//! Sync Integration Test
//!
//! This test validates the full end-to-end sync flow using mock transport:
//! 1. Set up two Core instances with separate libraries
//! 2. Connect them via mock transport (simulating network layer)
//! 3. Initialize sync services on both cores
//! 4. Create test data on Core A (locations, entries, tags)
//! 5. Monitor sync events on both cores
//! 6. Validate data appears correctly in Core B's database
//!
//! ## Running Tests
//!
//! For most reliable results, run tests serially to avoid resource contention:
//! ```bash
//! cargo test -p sd-core --test sync_integration_test -- --test-threads=1
//! ```
//!
//! Tests should pass when run in parallel, but may occasionally timeout under heavy load
//! due to competing background tasks from 20+ concurrent Core instances.
//!

mod helpers;

use helpers::MockTransport;
use sd_core::{
	infra::{
		db::entities,
		sync::{ChangeType, NetworkTransport, Syncable},
	},
	library::Library,
	Core,
};
use sea_orm::{
	ActiveModelBehavior, ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
	Set,
};
use std::{collections::HashMap, sync::Arc};
use tempfile::TempDir;
use tokio::{sync::Mutex, time::Duration};
use uuid::Uuid;

/// Test setup with two cores and mock transport
struct SyncTestSetup {
	temp_dir_a: TempDir,
	temp_dir_b: TempDir,
	core_a: Core,
	core_b: Core,
	library_a: Arc<Library>,
	library_b: Arc<Library>,
	device_a_id: Uuid,
	device_b_id: Uuid,
	transport_a: Arc<MockTransport>,
	transport_b: Arc<MockTransport>,
}

impl SyncTestSetup {
	/// Create a new sync test setup with two cores
	async fn new() -> anyhow::Result<Self> {
		let _ = tracing_subscriber::fmt()
			.with_env_filter("sd_core=debug,sync_integration_test=debug")
			.with_test_writer()
			.try_init();

		let temp_dir_a = TempDir::new()?;
		let temp_dir_b = TempDir::new()?;

		let config_a = sd_core::config::AppConfig {
			version: 3,
			data_dir: temp_dir_a.path().to_path_buf(),
			log_level: "info".to_string(),
			telemetry_enabled: false,
			preferences: sd_core::config::Preferences::default(),
			job_logging: sd_core::config::JobLoggingConfig::default(),
			services: sd_core::config::ServiceConfig {
				networking_enabled: false,
				volume_monitoring_enabled: false,
				location_watcher_enabled: false,
			},
		};
		config_a.save()?;

		let config_b = sd_core::config::AppConfig {
			version: 3,
			data_dir: temp_dir_b.path().to_path_buf(),
			log_level: "info".to_string(),
			telemetry_enabled: false,
			preferences: sd_core::config::Preferences::default(),
			job_logging: sd_core::config::JobLoggingConfig::default(),
			services: sd_core::config::ServiceConfig {
				networking_enabled: false,
				volume_monitoring_enabled: false,
				location_watcher_enabled: false,
			},
		};
		config_b.save()?;

		let core_a = Core::new(temp_dir_a.path().to_path_buf())
			.await
			.map_err(|e| anyhow::anyhow!("{}", e))?;
		let device_a_id = core_a.device.device_id()?;

		let core_b = Core::new(temp_dir_b.path().to_path_buf())
			.await
			.map_err(|e| anyhow::anyhow!("{}", e))?;
		let device_b_id = core_b.device.device_id()?;

		let library_a = core_a
			.libraries
			.create_library_no_sync("Test Library A", None, core_a.context.clone())
			.await?;
		let library_b = core_b
			.libraries
			.create_library_no_sync("Test Library B", None, core_b.context.clone())
			.await?;

		Self::register_device_in_library(&library_a, device_b_id, "Device B").await?;
		Self::register_device_in_library(&library_b, device_a_id, "Device A").await?;

		let (transport_a, transport_b) = MockTransport::new_pair(device_a_id, device_b_id);

		library_a
			.init_sync_service(
				device_a_id,
				transport_a.clone() as Arc<dyn NetworkTransport>,
			)
			.await?;
		library_b
			.init_sync_service(
				device_b_id,
				transport_b.clone() as Arc<dyn NetworkTransport>,
			)
			.await?;

		let setup = Self {
			temp_dir_a,
			temp_dir_b,
			core_a,
			core_b,
			library_a,
			library_b,
			device_a_id,
			device_b_id,
			transport_a,
			transport_b,
		};

		setup.wait_for_ready().await?;
		Ok(setup)
	}

	/// Register a device in a library's device table
	async fn register_device_in_library(
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
			sync_enabled: Set(true), // Enable sync by default
			last_sync_at: Set(None),
			last_state_watermark: Set(None),
			last_shared_watermark: Set(None),
			slug: Set(device_name.to_string()),
		};

		device_model.insert(library.db().conn()).await?;
		Ok(())
	}

	/// Process messages in both directions
	async fn pump_messages(&self) -> anyhow::Result<()> {
		let sync_a = self.library_a.sync_service().unwrap();
		let sync_b = self.library_b.sync_service().unwrap();

		self.transport_b.process_incoming_messages(sync_b).await?;
		self.transport_a.process_incoming_messages(sync_a).await?;

		Ok(())
	}

	/// Wait for sync to complete with message pumping
	async fn wait_for_sync(&self, duration: Duration) -> anyhow::Result<()> {
		let start = tokio::time::Instant::now();
		while start.elapsed() < duration {
			self.pump_messages().await?;
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
		Ok(())
	}

	/// Wait for sync service to reach Ready state
	async fn wait_for_ready(&self) -> anyhow::Result<()> {
		let start = tokio::time::Instant::now();
		let timeout = Duration::from_secs(30);

		while start.elapsed() < timeout {
			self.pump_messages().await?;

			let state_a = self
				.library_a
				.sync_service()
				.unwrap()
				.peer_sync()
				.state()
				.await;
			let state_b = self
				.library_b
				.sync_service()
				.unwrap()
				.peer_sync()
				.state()
				.await;

			if state_a.is_ready() && state_b.is_ready() {
				return Ok(());
			}

			tokio::time::sleep(Duration::from_millis(200)).await;
		}

		Err(anyhow::anyhow!("timeout waiting for ready state"))
	}

	/// Create a tag with sensible defaults (uses ActiveModel::new())
	async fn create_tag(
		&self,
		canonical_name: impl Into<String>,
		device_id: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<entities::tag::Model> {
		let mut tag = entities::tag::ActiveModel::new();
		tag.canonical_name = Set(canonical_name.into());
		tag.created_by_device = Set(Some(device_id));
		tag.namespace = Set(Some("photos".to_string()));

		let record = tag.insert(library.db().conn()).await?;
		Ok(record)
	}

	/// Create a tag with a specific UUID (for tests that need predictable UUIDs)
	async fn create_tag_with_uuid(
		&self,
		uuid: Uuid,
		canonical_name: impl Into<String>,
		device_id: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<entities::tag::Model> {
		let mut tag = entities::tag::ActiveModel::new();
		tag.uuid = Set(uuid);
		tag.canonical_name = Set(canonical_name.into());
		tag.created_by_device = Set(Some(device_id));
		tag.namespace = Set(Some("photos".to_string()));

		let record = tag.insert(library.db().conn()).await?;
		Ok(record)
	}

	/// Create an entry with minimal required fields
	async fn create_entry(
		&self,
		name: impl Into<String>,
		kind: i32,
		library: &Arc<Library>,
		parent_id: Option<i32>,
	) -> anyhow::Result<entities::entry::Model> {
		let entry = entities::entry::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Some(Uuid::new_v4())),
			name: Set(name.into()),
			kind: Set(kind),
			extension: Set(if kind == 0 {
				Some("txt".to_string())
			} else {
				None
			}),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(if kind == 0 { 1024 } else { 0 }),
			aggregate_size: Set(if kind == 0 { 1024 } else { 0 }),
			child_count: Set(0),
			file_count: Set(0),
			created_at: Set(chrono::Utc::now()),
			modified_at: Set(chrono::Utc::now()),
			accessed_at: Set(None),
			permissions: Set(None),
			inode: Set(None),
			parent_id: Set(parent_id),
		};

		let record = entry.insert(library.db().conn()).await?;
		Ok(record)
	}

	/// Create an entry with a specific UUID
	async fn create_entry_with_uuid(
		&self,
		uuid: Uuid,
		name: impl Into<String>,
		kind: i32,
		library: &Arc<Library>,
		parent_id: Option<i32>,
	) -> anyhow::Result<entities::entry::Model> {
		let entry = entities::entry::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Some(uuid)),
			name: Set(name.into()),
			kind: Set(kind),
			extension: Set(if kind == 0 {
				Some("txt".to_string())
			} else {
				None
			}),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(if kind == 0 { 1024 } else { 0 }),
			aggregate_size: Set(if kind == 0 { 1024 } else { 0 }),
			child_count: Set(0),
			file_count: Set(0),
			created_at: Set(chrono::Utc::now()),
			modified_at: Set(chrono::Utc::now()),
			accessed_at: Set(None),
			permissions: Set(None),
			inode: Set(None),
			parent_id: Set(parent_id),
		};

		let record = entry.insert(library.db().conn()).await?;
		Ok(record)
	}

	/// Find a device by UUID
	async fn find_device(
		&self,
		device_uuid: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<entities::device::Model> {
		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_uuid))?;
		Ok(device)
	}

	/// Find a tag by UUID
	async fn find_tag(
		&self,
		tag_uuid: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<Option<entities::tag::Model>> {
		let tag = entities::tag::Entity::find()
			.filter(entities::tag::Column::Uuid.eq(tag_uuid))
			.one(library.db().conn())
			.await?;
		Ok(tag)
	}

	/// Set a device as online
	async fn set_device_online(
		&self,
		device_uuid: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<()> {
		use chrono::Utc;
		let now = Utc::now();
		entities::device::Entity::update_many()
			.col_expr(
				entities::device::Column::IsOnline,
				sea_orm::sea_query::Expr::value(true),
			)
			.col_expr(
				entities::device::Column::LastSeenAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.col_expr(
				entities::device::Column::UpdatedAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.exec(library.db().conn())
			.await?;
		Ok(())
	}

	/// Set a device as offline
	async fn set_device_offline(
		&self,
		device_uuid: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<()> {
		use chrono::Utc;
		let now = Utc::now();
		entities::device::Entity::update_many()
			.col_expr(
				entities::device::Column::IsOnline,
				sea_orm::sea_query::Expr::value(false),
			)
			.col_expr(
				entities::device::Column::LastSeenAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.col_expr(
				entities::device::Column::UpdatedAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.exec(library.db().conn())
			.await?;
		Ok(())
	}

	/// Assert that a tag exists and return it
	async fn assert_tag_exists(
		&self,
		tag_uuid: Uuid,
		library: &Arc<Library>,
	) -> anyhow::Result<entities::tag::Model> {
		let tag = entities::tag::Entity::find()
			.filter(entities::tag::Column::Uuid.eq(tag_uuid))
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("tag {} not found", tag_uuid))?;
		Ok(tag)
	}

	/// Assert that the tag count matches expected
	async fn assert_tag_count(
		&self,
		expected: usize,
		library: &Arc<Library>,
	) -> anyhow::Result<()> {
		let count = entities::tag::Entity::find()
			.all(library.db().conn())
			.await?
			.len();
		assert_eq!(
			count, expected,
			"expected {} tags, found {}",
			expected, count
		);
		Ok(())
	}

	/// Get SharedChange messages from A to B
	async fn get_shared_change_messages(&self) -> Vec<sd_core::infra::sync::SharedChangeEntry> {
		self.transport_a
			.get_messages_between(self.device_a_id, self.device_b_id)
			.await
			.into_iter()
			.filter_map(|msg| {
				if let sd_core::service::network::protocol::sync::messages::SyncMessage::SharedChange {
					entry,
					..
				} = msg
				{
					Some(entry)
				} else {
					None
				}
			})
			.collect()
	}
}

#[tokio::test]
async fn test_sync_location_device_owned_state_based() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let location_uuid = Uuid::new_v4();
	let entry_uuid = Uuid::new_v4();

	let device_a = setup
		.find_device(setup.device_a_id, &setup.library_a)
		.await?;
	let entry = setup
		.create_entry_with_uuid(entry_uuid, "Test Location", 1, &setup.library_a, None)
		.await?;

	// Manually create entry on device B to satisfy FK dependency
	setup
		.create_entry_with_uuid(entry_uuid, "Test Location", 1, &setup.library_b, None)
		.await?;

	let location = entities::location::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(location_uuid),
		device_id: Set(device_a.id),
		entry_id: Set(entry.id),
		name: Set(Some("Test Location".to_string())),
		index_mode: Set("shallow".to_string()),
		scan_state: Set("completed".to_string()),
		last_scan_at: Set(Some(chrono::Utc::now())),
		error_message: Set(None),
		total_file_count: Set(0),
		total_byte_size: Set(0),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};

	let location_record = location.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model_with_db(
			&location_record,
			ChangeType::Insert,
			setup.library_a.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let location_on_b = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_uuid))
		.one(setup.library_b.db().conn())
		.await?;

	assert!(
		location_on_b.is_some(),
		"location failed to sync to device B"
	);
	assert_eq!(location_on_b.unwrap().uuid, location_uuid);

	Ok(())
}

#[tokio::test]
async fn test_sync_tag_shared_hlc_based() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let tag = setup
		.create_tag("Vacation", setup.device_a_id, &setup.library_a)
		.await?;
	setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let messages = setup.get_shared_change_messages().await;
	assert!(!messages.is_empty(), "no SharedChange messages sent");

	let synced = setup.assert_tag_exists(tag.uuid, &setup.library_b).await?;
	assert_eq!(synced.canonical_name, "Vacation");
	assert_eq!(synced.namespace, Some("photos".to_string()));

	Ok(())
}

#[tokio::test]
async fn test_sync_entry_with_location() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let location_uuid = Uuid::new_v4();
	let device_a = setup
		.find_device(setup.device_a_id, &setup.library_a)
		.await?;

	let location_entry = setup
		.create_entry("Test Location", 1, &setup.library_a, None)
		.await?;
	setup
		.library_a
		.sync_model_with_db(
			&location_entry,
			ChangeType::Insert,
			setup.library_a.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_millis(500)).await?;

	let location = entities::location::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(location_uuid),
		device_id: Set(device_a.id),
		entry_id: Set(location_entry.id),
		name: Set(Some("Test Location".to_string())),
		index_mode: Set("shallow".to_string()),
		scan_state: Set("completed".to_string()),
		last_scan_at: Set(Some(chrono::Utc::now())),
		error_message: Set(None),
		total_file_count: Set(0),
		total_byte_size: Set(0),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};
	location.insert(setup.library_a.db().conn()).await?;

	let entry = setup
		.create_entry(
			"test_file.txt",
			0,
			&setup.library_a,
			Some(location_entry.id),
		)
		.await?;
	setup
		.library_a
		.sync_model_with_db(&entry, ChangeType::Insert, setup.library_a.db().conn())
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let messages = setup
		.transport_a
		.get_messages_between(setup.device_a_id, setup.device_b_id)
		.await;
	let state_changes: Vec<_> =
		messages
			.iter()
			.filter_map(|msg| {
				if let sd_core::service::network::protocol::sync::messages::SyncMessage::StateChange {
				model_type, ..
			} = msg {
				Some(model_type.clone())
			} else {
				None
			}
			})
			.collect();

	assert!(!state_changes.is_empty(), "no state change messages sent");
	assert!(
		state_changes.iter().any(|m| m == "entry"),
		"no entry state change"
	);

	Ok(())
}

#[tokio::test]
async fn test_sync_backfill_includes_pre_sync_data() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let pre_sync_tags: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
	for (i, &uuid) in pre_sync_tags.iter().enumerate() {
		setup
			.create_tag_with_uuid(
				uuid,
				format!("PreSync {}", i),
				setup.device_a_id,
				&setup.library_a,
			)
			.await?;
	}

	let post_sync_tags: Vec<Uuid> = (0..2).map(|_| Uuid::new_v4()).collect();
	for (i, &uuid) in post_sync_tags.iter().enumerate() {
		let tag = setup
			.create_tag_with_uuid(
				uuid,
				format!("PostSync {}", i),
				setup.device_a_id,
				&setup.library_a,
			)
			.await?;
		setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	}

	setup.assert_tag_count(5, &setup.library_a).await?;

	let log_entries = setup
		.library_a
		.sync_service()
		.unwrap()
		.peer_sync()
		.peer_log()
		.get_since(None)
		.await?;
	assert_eq!(log_entries.len(), 2, "peer log should have 2 entries");

	let full_state = setup
		.library_a
		.sync_service()
		.unwrap()
		.peer_sync()
		.get_full_shared_state()
		.await?;
	let tags_in_state = full_state["tag"].as_array().unwrap();
	assert_eq!(tags_in_state.len(), 5, "backfill missing tags");

	let state_uuids: Vec<Uuid> = tags_in_state
		.iter()
		.map(|t| Uuid::parse_str(t["uuid"].as_str().unwrap()).unwrap())
		.collect();

	for uuid in pre_sync_tags.iter().chain(post_sync_tags.iter()) {
		assert!(state_uuids.contains(uuid), "backfill missing tag {}", uuid);
	}

	Ok(())
}

#[tokio::test]
async fn test_sync_transitive_three_devices() -> anyhow::Result<()> {
	let _ = tracing_subscriber::fmt()
		.with_env_filter("sd_core=debug")
		.with_test_writer()
		.try_init();

	let (temp_dir_a, temp_dir_b, temp_dir_c) = (TempDir::new()?, TempDir::new()?, TempDir::new()?);

	let mk_config = |dir: &TempDir| sd_core::config::AppConfig {
		version: 3,
		data_dir: dir.path().to_path_buf(),
		log_level: "info".to_string(),
		telemetry_enabled: false,
		preferences: sd_core::config::Preferences::default(),
		job_logging: sd_core::config::JobLoggingConfig::default(),
		services: sd_core::config::ServiceConfig {
			networking_enabled: false,
			volume_monitoring_enabled: false,
			location_watcher_enabled: false,
		},
	};

	mk_config(&temp_dir_a).save()?;
	mk_config(&temp_dir_b).save()?;
	mk_config(&temp_dir_c).save()?;

	let core_a = Core::new(temp_dir_a.path().to_path_buf())
		.await
		.map_err(|e| anyhow::anyhow!("{}", e))?;
	let core_b = Core::new(temp_dir_b.path().to_path_buf())
		.await
		.map_err(|e| anyhow::anyhow!("{}", e))?;
	let core_c = Core::new(temp_dir_c.path().to_path_buf())
		.await
		.map_err(|e| anyhow::anyhow!("{}", e))?;

	let (device_a_id, device_b_id, device_c_id) = (
		core_a.device.device_id()?,
		core_b.device.device_id()?,
		core_c.device.device_id()?,
	);

	let library_a = core_a
		.libraries
		.create_library_no_sync("Shared Library", None, core_a.context.clone())
		.await?;
	let library_b = core_b
		.libraries
		.create_library_no_sync("Shared Library", None, core_b.context.clone())
		.await?;
	let library_c = core_c
		.libraries
		.create_library_no_sync("Shared Library", None, core_c.context.clone())
		.await?;

	SyncTestSetup::register_device_in_library(&library_a, device_b_id, "Device B").await?;
	SyncTestSetup::register_device_in_library(&library_a, device_c_id, "Device C").await?;
	SyncTestSetup::register_device_in_library(&library_b, device_a_id, "Device A").await?;
	SyncTestSetup::register_device_in_library(&library_b, device_c_id, "Device C").await?;
	SyncTestSetup::register_device_in_library(&library_c, device_a_id, "Device A").await?;
	SyncTestSetup::register_device_in_library(&library_c, device_b_id, "Device B").await?;

	let queues = Arc::new(Mutex::new(HashMap::new()));
	let history = Arc::new(Mutex::new(Vec::new()));

	let transport_a = MockTransport::new(
		device_a_id,
		vec![device_b_id],
		queues.clone(),
		history.clone(),
	);
	let transport_b = MockTransport::new(
		device_b_id,
		vec![device_a_id, device_c_id],
		queues.clone(),
		history.clone(),
	);
	let transport_c = MockTransport::new(
		device_c_id,
		vec![device_b_id],
		queues.clone(),
		history.clone(),
	);

	library_a
		.init_sync_service(device_a_id, transport_a.clone())
		.await?;
	library_b
		.init_sync_service(device_b_id, transport_b.clone())
		.await?;
	library_c
		.init_sync_service(device_c_id, transport_c.clone())
		.await?;

	let start = tokio::time::Instant::now();
	let timeout = Duration::from_secs(30);

	loop {
		transport_a
			.process_incoming_messages(library_a.sync_service().unwrap())
			.await?;
		transport_b
			.process_incoming_messages(library_b.sync_service().unwrap())
			.await?;
		transport_c
			.process_incoming_messages(library_c.sync_service().unwrap())
			.await?;

		let state_a = library_a.sync_service().unwrap().peer_sync().state().await;
		let state_b = library_b.sync_service().unwrap().peer_sync().state().await;
		let state_c = library_c.sync_service().unwrap().peer_sync().state().await;

		if state_a.is_ready() && state_b.is_ready() && state_c.is_ready() {
			break;
		}

		if start.elapsed() > timeout {
			library_a
				.sync_service()
				.unwrap()
				.peer_sync()
				.transition_to_ready()
				.await?;
			library_b
				.sync_service()
				.unwrap()
				.peer_sync()
				.transition_to_ready()
				.await?;
			library_c
				.sync_service()
				.unwrap()
				.peer_sync()
				.transition_to_ready()
				.await?;
			break;
		}

		tokio::time::sleep(Duration::from_millis(200)).await;
	}

	let tag_uuid = Uuid::new_v4();
	let mut tag_model = entities::tag::ActiveModel::new();
	tag_model.uuid = Set(tag_uuid);
	tag_model.canonical_name = Set("A's Tag".to_string());
	tag_model.created_by_device = Set(Some(device_a_id));
	tag_model.namespace = Set(Some("photos".to_string()));

	let tag = tag_model.insert(library_a.db().conn()).await?;
	library_a.sync_model(&tag, ChangeType::Insert).await?;

	tokio::time::sleep(Duration::from_millis(100)).await;
	transport_b
		.process_incoming_messages(library_b.sync_service().unwrap())
		.await?;
	tokio::time::sleep(Duration::from_millis(500)).await;

	let tag_on_b = entities::tag::Entity::find()
		.filter(entities::tag::Column::Uuid.eq(tag_uuid))
		.one(library_b.db().conn())
		.await?;
	assert!(tag_on_b.is_some(), "tag failed to sync to device B");

	use sd_core::service::network::protocol::sync::messages::SyncMessage;
	let request = SyncMessage::SharedChangeRequest {
		library_id: library_c.id(),
		since_hlc: None,
		limit: 1000,
	};

	transport_c.send_sync_message(device_b_id, request).await?;

	tokio::time::sleep(Duration::from_millis(100)).await;
	transport_b
		.process_incoming_messages(library_b.sync_service().unwrap())
		.await?;
	tokio::time::sleep(Duration::from_millis(100)).await;
	transport_c
		.process_incoming_messages(library_c.sync_service().unwrap())
		.await?;

	let tag_on_c = entities::tag::Entity::find()
		.filter(entities::tag::Column::Uuid.eq(tag_uuid))
		.one(library_c.db().conn())
		.await?;
	assert!(tag_on_c.is_some(), "tag failed to propagate from A->B->C");

	let synced = tag_on_c.unwrap();
	assert_eq!(synced.canonical_name, "A's Tag");
	assert_eq!(synced.namespace, Some("photos".to_string()));

	Ok(())
}

#[tokio::test]
async fn test_connection_state_tracking() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let device_b_on_a = setup
		.find_device(setup.device_b_id, &setup.library_a)
		.await?;
	let device_a_on_b = setup
		.find_device(setup.device_a_id, &setup.library_b)
		.await?;
	assert_eq!(device_b_on_a.is_online, false);
	assert_eq!(device_a_on_b.is_online, false);

	setup
		.set_device_online(setup.device_b_id, &setup.library_a)
		.await?;
	setup
		.set_device_online(setup.device_a_id, &setup.library_b)
		.await?;

	let device_b_on_a = setup
		.find_device(setup.device_b_id, &setup.library_a)
		.await?;
	let device_a_on_b = setup
		.find_device(setup.device_a_id, &setup.library_b)
		.await?;
	assert_eq!(device_b_on_a.is_online, true);
	assert_eq!(device_a_on_b.is_online, true);

	setup
		.set_device_offline(setup.device_b_id, &setup.library_a)
		.await?;
	setup
		.set_device_offline(setup.device_a_id, &setup.library_b)
		.await?;

	let device_b_on_a = setup
		.find_device(setup.device_b_id, &setup.library_a)
		.await?;
	let device_a_on_b = setup
		.find_device(setup.device_a_id, &setup.library_b)
		.await?;
	assert_eq!(device_b_on_a.is_online, false);
	assert_eq!(device_a_on_b.is_online, false);

	Ok(())
}

#[tokio::test]
async fn test_watermark_reconnection_sync() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	for i in 0..3 {
		let tag = setup
			.create_tag(
				format!("Initial {}", i),
				setup.device_a_id,
				&setup.library_a,
			)
			.await?;
		setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	}
	setup.wait_for_sync(Duration::from_secs(2)).await?;
	setup.assert_tag_count(3, &setup.library_b).await?;

	let last_hlc = setup
		.get_shared_change_messages()
		.await
		.into_iter()
		.map(|e| e.hlc)
		.last()
		.expect("no shared change messages");

	tokio::time::sleep(Duration::from_millis(100)).await;

	let mut new_tags = Vec::new();
	for i in 0..2 {
		let tag = setup
			.create_tag(format!("New {}", i), setup.device_a_id, &setup.library_a)
			.await?;
		setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
		new_tags.push(tag.uuid);
	}

	let messages_before = setup
		.transport_a
		.get_messages_between(setup.device_a_id, setup.device_b_id)
		.await
		.len();

	use sd_core::service::network::protocol::sync::messages::SyncMessage;
	let request = SyncMessage::SharedChangeRequest {
		library_id: setup.library_b.id(),
		since_hlc: Some(last_hlc),
		limit: 1000,
	};

	tokio::spawn({
		let transport_b = setup.transport_b.clone();
		let device_a_id = setup.device_a_id;
		async move {
			transport_b
				.send_sync_message(device_a_id, request)
				.await
				.unwrap();
		}
	});

	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let all_messages = setup
		.transport_a
		.get_messages_between(setup.device_a_id, setup.device_b_id)
		.await;
	let responses: Vec<_> = all_messages
		.iter()
		.skip(messages_before)
		.filter_map(|msg| {
			if let SyncMessage::SharedChangeResponse {
				entries,
				current_state,
				..
			} = msg
			{
				Some((entries, current_state))
			} else {
				None
			}
		})
		.collect();

	assert!(!responses.is_empty(), "no response received");
	assert!(
		responses.iter().all(|(_, state)| state.is_none()),
		"incremental sync included full state"
	);

	let total_entries: usize = responses.iter().map(|(e, _)| e.len()).sum();
	assert_eq!(total_entries, 2, "wrong number of incremental changes");

	setup.assert_tag_count(5, &setup.library_b).await?;

	Ok(())
}

#[tokio::test]
async fn test_automatic_watermark_exchange_on_reconnection() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	for i in 0..3 {
		let tag = setup
			.create_tag(
				format!("Initial {}", i),
				setup.device_a_id,
				&setup.library_a,
			)
			.await?;
		setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	}
	setup.wait_for_sync(Duration::from_secs(2)).await?;
	setup.assert_tag_count(3, &setup.library_b).await?;

	setup
		.set_device_offline(setup.device_b_id, &setup.library_a)
		.await?;

	tokio::time::sleep(Duration::from_millis(100)).await;

	for i in 0..2 {
		let tag = setup
			.create_tag(format!("New {}", i), setup.device_a_id, &setup.library_a)
			.await?;
		setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	}

	let sync_b = setup.library_b.sync_service().unwrap();
	setup
		.set_device_online(setup.device_b_id, &setup.library_a)
		.await?;
	setup
		.set_device_online(setup.device_a_id, &setup.library_b)
		.await?;

	sync_b
		.peer_sync()
		.exchange_watermarks_and_catchup(setup.device_a_id)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	use sd_core::service::network::protocol::sync::messages::SyncMessage;
	let messages_b_to_a = setup
		.transport_b
		.get_messages_between(setup.device_b_id, setup.device_a_id)
		.await;
	let messages_a_to_b = setup
		.transport_a
		.get_messages_between(setup.device_a_id, setup.device_b_id)
		.await;

	assert!(
		messages_b_to_a
			.iter()
			.any(|m| matches!(m, SyncMessage::WatermarkExchangeRequest { .. })),
		"no watermark request"
	);
	assert!(
		messages_a_to_b
			.iter()
			.any(|m| matches!(m, SyncMessage::WatermarkExchangeResponse { .. })),
		"no watermark response"
	);
	assert!(
		messages_b_to_a
			.iter()
			.any(|m| matches!(m, SyncMessage::SharedChangeRequest { .. })),
		"no shared change request"
	);

	setup.assert_tag_count(5, &setup.library_b).await?;

	Ok(())
}

#[tokio::test]
async fn test_concurrent_tag_updates_hlc_conflict_resolution() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let tag_uuid = Uuid::new_v4();
	let tag_a = setup
		.create_tag_with_uuid(tag_uuid, "Version A", setup.device_a_id, &setup.library_a)
		.await?;
	let tag_b = setup
		.create_tag_with_uuid(tag_uuid, "Version B", setup.device_b_id, &setup.library_b)
		.await?;

	tokio::time::sleep(Duration::from_millis(50)).await;
	setup
		.library_a
		.sync_model(&tag_a, ChangeType::Insert)
		.await?;

	tokio::time::sleep(Duration::from_millis(100)).await;
	setup
		.library_b
		.sync_model(&tag_b, ChangeType::Insert)
		.await?;

	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let tag_on_a = setup.assert_tag_exists(tag_uuid, &setup.library_a).await?;
	let tag_on_b = setup.assert_tag_exists(tag_uuid, &setup.library_b).await?;

	assert_eq!(
		tag_on_a.canonical_name, tag_on_b.canonical_name,
		"tags did not converge"
	);
	assert_eq!(tag_on_b.canonical_name, "Version B", "wrong version won");

	Ok(())
}

#[tokio::test]
async fn test_sync_update_and_delete_operations() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	let tag_uuid = Uuid::new_v4();
	let mut tag_model = entities::tag::ActiveModel::new();
	tag_model.uuid = Set(tag_uuid);
	tag_model.canonical_name = Set("Original".to_string());
	tag_model.description = Set(Some("Original description".to_string()));
	tag_model.created_by_device = Set(Some(setup.device_a_id));
	tag_model.namespace = Set(Some("photos".to_string()));

	let tag = tag_model.insert(setup.library_a.db().conn()).await?;
	setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let tag_on_b = setup.assert_tag_exists(tag_uuid, &setup.library_b).await?;
	assert_eq!(tag_on_b.canonical_name, "Original");
	assert_eq!(
		tag_on_b.description,
		Some("Original description".to_string())
	);

	let mut tag_model: entities::tag::ActiveModel = tag.into();
	tag_model.canonical_name = Set("Updated".to_string());
	tag_model.description = Set(Some("Updated description".to_string()));
	tag_model.updated_at = Set(chrono::Utc::now());

	let updated_tag = tag_model.update(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model(&updated_tag, ChangeType::Update)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let tag_on_b = setup.assert_tag_exists(tag_uuid, &setup.library_b).await?;
	assert_eq!(tag_on_b.canonical_name, "Updated");
	assert_eq!(
		tag_on_b.description,
		Some("Updated description".to_string())
	);

	entities::tag::Entity::delete_by_id(updated_tag.id)
		.exec(setup.library_a.db().conn())
		.await?;
	setup
		.library_a
		.sync_model(&updated_tag, ChangeType::Delete)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let tag_on_b = setup.find_tag(tag_uuid, &setup.library_b).await?;
	assert!(tag_on_b.is_none(), "tag should be deleted");

	Ok(())
}

// ========== Many-to-Many Sync Tests ==========

#[tokio::test]
async fn test_sync_collection_entry_m2m() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	// Create a collection on device A
	let collection = entities::collection::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Uuid::new_v4()),
		name: Set("Vacation Photos".to_string()),
		description: Set(Some("Summer 2025 trip".to_string())),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};
	let collection_record = collection.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model(&collection_record, ChangeType::Insert)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Create entries on both devices
	let entry1 = setup
		.create_entry("photo1.jpg", 0, &setup.library_a, None)
		.await?;
	let entry2 = setup
		.create_entry("photo2.jpg", 0, &setup.library_a, None)
		.await?;

	setup
		.library_a
		.sync_model_with_db(&entry1, ChangeType::Insert, setup.library_a.db().conn())
		.await?;
	setup
		.library_a
		.sync_model_with_db(&entry2, ChangeType::Insert, setup.library_a.db().conn())
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Find the synced collection on device B to get its local ID
	let collection_on_b = entities::collection::Entity::find()
		.filter(entities::collection::Column::Uuid.eq(collection_record.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("collection should be synced");

	// Find synced entries on device B
	let entry1_on_b = entities::entry::Entity::find()
		.filter(entities::entry::Column::Uuid.eq(entry1.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("entry1 should be synced");

	let entry2_on_b = entities::entry::Entity::find()
		.filter(entities::entry::Column::Uuid.eq(entry2.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("entry2 should be synced");

	// Add entries to collection on device A
	let collection_entry1 = entities::collection_entry::ActiveModel {
		collection_id: Set(collection_record.id),
		entry_id: Set(entry1.id),
		added_at: Set(chrono::Utc::now()),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
		updated_at: Set(chrono::Utc::now()),
	};
	let ce1_record = collection_entry1
		.insert(setup.library_a.db().conn())
		.await?;
	setup
		.library_a
		.sync_model_with_db(&ce1_record, ChangeType::Insert, setup.library_a.db().conn())
		.await?;

	let collection_entry2 = entities::collection_entry::ActiveModel {
		collection_id: Set(collection_record.id),
		entry_id: Set(entry2.id),
		added_at: Set(chrono::Utc::now()),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
		updated_at: Set(chrono::Utc::now()),
	};
	let ce2_record = collection_entry2
		.insert(setup.library_a.db().conn())
		.await?;
	setup
		.library_a
		.sync_model_with_db(&ce2_record, ChangeType::Insert, setup.library_a.db().conn())
		.await?;

	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify collection entries synced to device B with correct FK mapping
	let entries_on_b = entities::collection_entry::Entity::find()
		.filter(entities::collection_entry::Column::CollectionId.eq(collection_on_b.id))
		.all(setup.library_b.db().conn())
		.await?;

	assert_eq!(entries_on_b.len(), 2, "both collection entries should sync");

	let entry_ids: Vec<i32> = entries_on_b.iter().map(|e| e.entry_id).collect();
	assert!(
		entry_ids.contains(&entry1_on_b.id),
		"entry1 should be in collection"
	);
	assert!(
		entry_ids.contains(&entry2_on_b.id),
		"entry2 should be in collection"
	);

	// Verify UUIDs match
	let uuids_on_b: Vec<Uuid> = entries_on_b.iter().map(|e| e.uuid).collect();
	assert!(
		uuids_on_b.contains(&ce1_record.uuid),
		"ce1 uuid should match"
	);
	assert!(
		uuids_on_b.contains(&ce2_record.uuid),
		"ce2 uuid should match"
	);

	Ok(())
}

#[tokio::test]
async fn test_sync_tag_relationship_m2m() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	// Create parent and child tags on device A
	let parent_tag = setup
		.create_tag("Animals", setup.device_a_id, &setup.library_a)
		.await?;
	let child_tag = setup
		.create_tag("Cats", setup.device_a_id, &setup.library_a)
		.await?;

	setup
		.library_a
		.sync_model(&parent_tag, ChangeType::Insert)
		.await?;
	setup
		.library_a
		.sync_model(&child_tag, ChangeType::Insert)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify tags synced to device B
	let parent_on_b = entities::tag::Entity::find()
		.filter(entities::tag::Column::Uuid.eq(parent_tag.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("parent tag should sync");

	let child_on_b = entities::tag::Entity::find()
		.filter(entities::tag::Column::Uuid.eq(child_tag.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("child tag should sync");

	// Create relationship on device A
	let relationship = entities::tag_relationship::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		parent_tag_id: Set(parent_tag.id),
		child_tag_id: Set(child_tag.id),
		relationship_type: Set("parent_child".to_string()),
		strength: Set(1.0),
		created_at: Set(chrono::Utc::now()),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
		updated_at: Set(chrono::Utc::now()),
	};

	let relationship_record = relationship.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model_with_db(
			&relationship_record,
			ChangeType::Insert,
			setup.library_a.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify relationship synced to device B with correct FK mapping
	let relationship_on_b = entities::tag_relationship::Entity::find()
		.filter(entities::tag_relationship::Column::Uuid.eq(relationship_record.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("tag relationship should sync");

	assert_eq!(
		relationship_on_b.parent_tag_id, parent_on_b.id,
		"parent FK should map correctly"
	);
	assert_eq!(
		relationship_on_b.child_tag_id, child_on_b.id,
		"child FK should map correctly"
	);
	assert_eq!(relationship_on_b.relationship_type, "parent_child");
	assert_eq!(relationship_on_b.strength, 1.0);

	Ok(())
}

#[tokio::test]
async fn test_sync_user_metadata_tag_content_scoped_shared() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	// Create tag
	let tag = setup
		.create_tag("Favorite", setup.device_a_id, &setup.library_a)
		.await?;
	setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Create content identity (shared resource)
	let content_uuid = Uuid::new_v4();
	let content_identity = entities::content_identity::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Some(content_uuid)),
		content_hash: Set("abc123".to_string()),
		integrity_hash: Set(None),
		mime_type_id: Set(None),
		kind_id: Set(1), // Generic kind
		text_content: Set(None),
		total_size: Set(1024),
		entry_count: Set(0),
		first_seen_at: Set(chrono::Utc::now()),
		last_verified_at: Set(chrono::Utc::now()),
	};
	let content_record = content_identity.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model(&content_record, ChangeType::Insert)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Create content-scoped user metadata
	let user_metadata = entities::user_metadata::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Uuid::new_v4()),
		entry_uuid: Set(None),
		content_identity_uuid: Set(Some(content_uuid)),
		notes: Set(Some("Great photo!".to_string())),
		favorite: Set(true),
		hidden: Set(false),
		custom_data: Set(serde_json::json!({})),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};
	let metadata_record = user_metadata.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model(&metadata_record, ChangeType::Insert)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Get synced entities on device B
	let tag_on_b = entities::tag::Entity::find()
		.filter(entities::tag::Column::Uuid.eq(tag.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("tag should sync");

	let metadata_on_b = entities::user_metadata::Entity::find()
		.filter(entities::user_metadata::Column::Uuid.eq(metadata_record.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("user_metadata should sync");

	// Tag the content-scoped metadata on device A
	let metadata_tag = entities::user_metadata_tag::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		user_metadata_id: Set(metadata_record.id),
		tag_id: Set(tag.id),
		applied_context: Set(None),
		applied_variant: Set(None),
		confidence: Set(1.0),
		source: Set("user".to_string()),
		instance_attributes: Set(None),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
		device_uuid: Set(setup.device_a_id),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
	};
	let metadata_tag_record = metadata_tag.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model_with_db(
			&metadata_tag_record,
			ChangeType::Insert,
			setup.library_a.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify user_metadata_tag synced to device B (content-scoped = shared)
	let metadata_tag_on_b = entities::user_metadata_tag::Entity::find()
		.filter(entities::user_metadata_tag::Column::Uuid.eq(metadata_tag_record.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("content-scoped user_metadata_tag should sync");

	assert_eq!(
		metadata_tag_on_b.user_metadata_id, metadata_on_b.id,
		"user_metadata FK should map"
	);
	assert_eq!(metadata_tag_on_b.tag_id, tag_on_b.id, "tag FK should map");
	assert_eq!(metadata_tag_on_b.source, "user");
	assert_eq!(metadata_tag_on_b.confidence, 1.0);

	Ok(())
}

#[tokio::test]
async fn test_sync_user_metadata_tag_entry_scoped_ownership_enforcement() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	// Create tag
	let tag = setup
		.create_tag("Work", setup.device_a_id, &setup.library_a)
		.await?;
	setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Create location and entry on device A
	let device_a = setup
		.find_device(setup.device_a_id, &setup.library_a)
		.await?;
	let location_entry = setup
		.create_entry("Documents", 1, &setup.library_a, None)
		.await?;
	setup
		.library_a
		.sync_model_with_db(
			&location_entry,
			ChangeType::Insert,
			setup.library_a.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_secs(1)).await?;

	let location = entities::location::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Uuid::new_v4()),
		device_id: Set(device_a.id),
		entry_id: Set(location_entry.id),
		name: Set(Some("Work Drive".to_string())),
		index_mode: Set("shallow".to_string()),
		scan_state: Set("completed".to_string()),
		last_scan_at: Set(Some(chrono::Utc::now())),
		error_message: Set(None),
		total_file_count: Set(0),
		total_byte_size: Set(0),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};
	location.insert(setup.library_a.db().conn()).await?;

	// Create entry-scoped user metadata on the location entry itself (simplifies ownership check)
	let user_metadata = entities::user_metadata::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Uuid::new_v4()),
		entry_uuid: Set(location_entry.uuid),
		content_identity_uuid: Set(None),
		notes: Set(Some("Important document".to_string())),
		favorite: Set(false),
		hidden: Set(false),
		custom_data: Set(serde_json::json!({})),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};
	let metadata_record = user_metadata.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model(&metadata_record, ChangeType::Insert)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Tag the entry-scoped metadata on device A (owning device)
	let metadata_tag_a = entities::user_metadata_tag::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		user_metadata_id: Set(metadata_record.id),
		tag_id: Set(tag.id),
		applied_context: Set(None),
		applied_variant: Set(None),
		confidence: Set(1.0),
		source: Set("user".to_string()),
		instance_attributes: Set(None),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
		device_uuid: Set(setup.device_a_id),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
	};
	let metadata_tag_a_record = metadata_tag_a.insert(setup.library_a.db().conn()).await?;
	setup
		.library_a
		.sync_model_with_db(
			&metadata_tag_a_record,
			ChangeType::Insert,
			setup.library_a.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify it synced (device A owns the entry)
	let tag_on_b_count = entities::user_metadata_tag::Entity::find()
		.filter(entities::user_metadata_tag::Column::Uuid.eq(metadata_tag_a_record.uuid))
		.count(setup.library_b.db().conn())
		.await?;
	assert_eq!(tag_on_b_count, 1, "tag from owning device should sync");

	// Now try to tag from device B (non-owning device)
	let metadata_on_b = entities::user_metadata::Entity::find()
		.filter(entities::user_metadata::Column::Uuid.eq(metadata_record.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("metadata should be synced");

	let tag_on_b = entities::tag::Entity::find()
		.filter(entities::tag::Column::Uuid.eq(tag.uuid))
		.one(setup.library_b.db().conn())
		.await?
		.expect("tag should be synced");

	let metadata_tag_b = entities::user_metadata_tag::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		user_metadata_id: Set(metadata_on_b.id),
		tag_id: Set(tag_on_b.id),
		applied_context: Set(None),
		applied_variant: Set(None),
		confidence: Set(0.8),
		source: Set("ai".to_string()),
		instance_attributes: Set(None),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
		device_uuid: Set(setup.device_b_id), // Device B trying to tag
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
	};
	let metadata_tag_b_record = metadata_tag_b.insert(setup.library_b.db().conn()).await?;
	setup
		.library_b
		.sync_model_with_db(
			&metadata_tag_b_record,
			ChangeType::Insert,
			setup.library_b.db().conn(),
		)
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify device B's tag was REJECTED on device A (ownership enforcement)
	let rejected_tag_on_a = entities::user_metadata_tag::Entity::find()
		.filter(entities::user_metadata_tag::Column::Uuid.eq(metadata_tag_b_record.uuid))
		.one(setup.library_a.db().conn())
		.await?;

	assert!(
		rejected_tag_on_a.is_none(),
		"entry-scoped tag from non-owning device should be rejected"
	);

	Ok(())
}

#[tokio::test]
async fn test_m2m_dependency_ordering() -> anyhow::Result<()> {
	let setup = SyncTestSetup::new().await?;

	// Create all dependencies in correct order
	let tag = setup
		.create_tag("Photo", setup.device_a_id, &setup.library_a)
		.await?;
	let collection = entities::collection::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(Uuid::new_v4()),
		name: Set("Gallery".to_string()),
		description: Set(None),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};
	let collection_record = collection.insert(setup.library_a.db().conn()).await?;

	let entry = setup
		.create_entry("image.jpg", 0, &setup.library_a, None)
		.await?;

	// Sync dependencies
	setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
	setup
		.library_a
		.sync_model(&collection_record, ChangeType::Insert)
		.await?;
	setup
		.library_a
		.sync_model_with_db(&entry, ChangeType::Insert, setup.library_a.db().conn())
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Now create M2M relationships
	let collection_entry = entities::collection_entry::ActiveModel {
		collection_id: Set(collection_record.id),
		entry_id: Set(entry.id),
		added_at: Set(chrono::Utc::now()),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
		updated_at: Set(chrono::Utc::now()),
	};
	let ce_record = collection_entry.insert(setup.library_a.db().conn()).await?;

	let parent_tag = setup
		.create_tag("Media", setup.device_a_id, &setup.library_a)
		.await?;
	setup
		.library_a
		.sync_model(&parent_tag, ChangeType::Insert)
		.await?;
	setup.wait_for_sync(Duration::from_secs(1)).await?;

	let tag_relationship = entities::tag_relationship::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		parent_tag_id: Set(parent_tag.id),
		child_tag_id: Set(tag.id),
		relationship_type: Set("parent_child".to_string()),
		strength: Set(1.0),
		created_at: Set(chrono::Utc::now()),
		uuid: Set(Uuid::new_v4()),
		version: Set(1),
		updated_at: Set(chrono::Utc::now()),
	};
	let tr_record = tag_relationship.insert(setup.library_a.db().conn()).await?;

	// Sync M2M relationships
	setup
		.library_a
		.sync_model_with_db(&ce_record, ChangeType::Insert, setup.library_a.db().conn())
		.await?;
	setup
		.library_a
		.sync_model_with_db(&tr_record, ChangeType::Insert, setup.library_a.db().conn())
		.await?;
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	// Verify both M2M relationships synced successfully
	let ce_on_b = entities::collection_entry::Entity::find()
		.filter(entities::collection_entry::Column::Uuid.eq(ce_record.uuid))
		.one(setup.library_b.db().conn())
		.await?;
	assert!(
		ce_on_b.is_some(),
		"collection_entry should sync after dependencies"
	);

	let tr_on_b = entities::tag_relationship::Entity::find()
		.filter(entities::tag_relationship::Column::Uuid.eq(tr_record.uuid))
		.one(setup.library_b.db().conn())
		.await?;
	assert!(
		tr_on_b.is_some(),
		"tag_relationship should sync after dependencies"
	);

	Ok(())
}
