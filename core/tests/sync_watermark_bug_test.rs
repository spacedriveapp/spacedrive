//! Test to demonstrate the global watermark bug with >10k entries
//!
//! This test creates mixed resource types (locations, entries, volumes) with
//! timestamps spread over time. When syncing >10k entries, the global watermark
//! advances past earlier timestamps of other resource types, causing them to be
//! skipped during reconnection sync.
//!
//! **Expected Result**: This test should FAIL, proving the bug exists.
//! After implementing per-resource watermarks, this test should PASS.

mod helpers;

use helpers::MockTransport;
use sd_core::{
	domain::volume::VolumeFingerprint,
	infra::{
		db::entities,
		sync::{ChangeType, NetworkTransport},
	},
	library::Library,
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;
use uuid::Uuid;

/// Test setup with two cores
struct WatermarkTestSetup {
	_temp_dir_a: TempDir,
	_temp_dir_b: TempDir,
	core_a: Core,
	core_b: Core,
	library_a: Arc<Library>,
	library_b: Arc<Library>,
	device_a_id: Uuid,
	device_b_id: Uuid,
	transport_a: Arc<MockTransport>,
	transport_b: Arc<MockTransport>,
}

impl WatermarkTestSetup {
	async fn new() -> anyhow::Result<Self> {
		let _ = tracing_subscriber::fmt()
			.with_env_filter("sd_core=info,sync_watermark_bug_test=debug")
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
			logging: sd_core::config::LoggingConfig::default(),
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
			logging: sd_core::config::LoggingConfig::default(),
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
			.create_library_no_sync("Library A", None, core_a.context.clone())
			.await?;
		let library_b = core_b
			.libraries
			.create_library_no_sync("Library B", None, core_b.context.clone())
			.await?;

		// Register devices in each other's libraries
		Self::register_device(&library_a, device_b_id, "Device B").await?;
		Self::register_device(&library_b, device_a_id, "Device A").await?;

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

		// Register sync services with transports for request/response BEFORE backfill starts
		if let Some(sync_a) = library_a.sync_service() {
			transport_a
				.register_sync_service(device_a_id, Arc::downgrade(&sync_a))
				.await;
			transport_b
				.register_sync_service(device_a_id, Arc::downgrade(&sync_a))
				.await; // Register on both
		}
		if let Some(sync_b) = library_b.sync_service() {
			transport_a
				.register_sync_service(device_b_id, Arc::downgrade(&sync_b))
				.await; // Register on both
			transport_b
				.register_sync_service(device_b_id, Arc::downgrade(&sync_b))
				.await;
		}

		let setup = Self {
			_temp_dir_a: temp_dir_a,
			_temp_dir_b: temp_dir_b,
			core_a,
			core_b,
			library_a,
			library_b,
			device_a_id,
			device_b_id,
			transport_a,
			transport_b,
		};

		// Device A is the source - manually transition to Ready (skip automatic backfill)
		if let Some(sync_a) = setup.library_a.sync_service() {
			sync_a.peer_sync().transition_to_ready().await?;
		}

		// Device B - also transition to Ready to prevent automatic backfill
		// We'll manually trigger backfill after creating test data
		if let Some(sync_b) = setup.library_b.sync_service() {
			sync_b.peer_sync().transition_to_ready().await?;
		}

		tokio::time::sleep(Duration::from_millis(200)).await;

		Ok(setup)
	}

	async fn register_device(
		library: &Arc<Library>,
		device_id: Uuid,
		device_name: &str,
	) -> anyhow::Result<()> {
		use chrono::Utc;

		let device = entities::device::ActiveModel {
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
			last_state_watermark: Set(None),
			last_shared_watermark: Set(None),
			slug: Set(device_name.to_string()),
		};

		device.insert(library.db().conn()).await?;
		Ok(())
	}

	async fn pump_messages(&self) -> anyhow::Result<()> {
		let sync_a = self.library_a.sync_service().unwrap();
		let sync_b = self.library_b.sync_service().unwrap();

		self.transport_b.process_incoming_messages(sync_b).await?;
		self.transport_a.process_incoming_messages(sync_a).await?;

		Ok(())
	}

	async fn wait_for_sync(&self, duration: Duration) -> anyhow::Result<()> {
		let start = tokio::time::Instant::now();
		while start.elapsed() < duration {
			self.pump_messages().await?;
			tokio::time::sleep(Duration::from_millis(50)).await;
		}
		Ok(())
	}

	/// Manually trigger backfill from Device B (for testing)
	async fn trigger_manual_backfill(&self) -> anyhow::Result<()> {
		let sync_b = self.library_b.sync_service().unwrap();
		let backfill_manager = sync_b.backfill_manager();

		let peer_info = vec![sd_core::service::sync::state::PeerInfo {
			device_id: self.device_a_id,
			is_online: true,
			latency_ms: 10.0,
			has_complete_state: true,
			active_syncs: 0,
		}];

		// Directly call backfill (it will transition state internally)
		backfill_manager.start_backfill(peer_info).await?;
		Ok(())
	}

	/// Create locations with specific timestamps
	async fn create_location_with_timestamp(
		&self,
		name: &str,
		device_id: i32,
		timestamp: chrono::DateTime<chrono::Utc>,
	) -> anyhow::Result<entities::location::Model> {
		// Create entry for the location first
		let entry = entities::entry::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Some(Uuid::new_v4())),
			name: Set(name.to_string()),
			kind: Set(1), // Directory
			extension: Set(None),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(0),
			aggregate_size: Set(0),
			child_count: Set(0),
			file_count: Set(0),
			created_at: Set(timestamp),
			modified_at: Set(timestamp),
			accessed_at: Set(None),
			indexed_at: Set(Some(timestamp)),
			permissions: Set(None),
			inode: Set(None),
			parent_id: Set(None),
		};

		let entry_record = entry.insert(self.library_a.db().conn()).await?;

		let location = entities::location::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Uuid::new_v4()),
			device_id: Set(device_id),
			entry_id: Set(Some(entry_record.id)),
			name: Set(Some(name.to_string())),
			index_mode: Set("shallow".to_string()),
			scan_state: Set("completed".to_string()),
			last_scan_at: Set(Some(timestamp)),
			error_message: Set(None),
			total_file_count: Set(0),
			total_byte_size: Set(0),
			job_policies: Set(None),
			created_at: Set(timestamp),
			updated_at: Set(timestamp),
		};

		Ok(location.insert(self.library_a.db().conn()).await?)
	}

	/// Create entries with specific timestamps
	async fn create_entry_with_timestamp(
		&self,
		name: &str,
		kind: i32,
		timestamp: chrono::DateTime<chrono::Utc>,
	) -> anyhow::Result<entities::entry::Model> {
		let entry = entities::entry::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Some(Uuid::new_v4())),
			name: Set(name.to_string()),
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
			file_count: Set(if kind == 0 { 1 } else { 0 }),
			created_at: Set(timestamp),
			modified_at: Set(timestamp),
			accessed_at: Set(None),
			indexed_at: Set(Some(timestamp)),
			permissions: Set(None),
			inode: Set(None),
			parent_id: Set(None),
		};

		Ok(entry.insert(self.library_a.db().conn()).await?)
	}

	/// Create volumes with specific timestamps
	async fn create_volume_with_timestamp(
		&self,
		name: &str,
		device_uuid: Uuid,
		timestamp: chrono::DateTime<chrono::Utc>,
	) -> anyhow::Result<entities::volume::Model> {
		use sd_core::domain::volume::VolumeFingerprint;

		let fingerprint = VolumeFingerprint::new(name, 1_000_000_000, "ext4");

		let volume = entities::volume::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Uuid::new_v4()),
			device_id: Set(device_uuid),
			fingerprint: Set(fingerprint.to_string()),
			display_name: Set(Some(name.to_string())),
			tracked_at: Set(timestamp),
			last_seen_at: Set(timestamp),
			is_online: Set(true),
			total_capacity: Set(Some(1_000_000_000)),
			available_capacity: Set(Some(500_000_000)),
			read_speed_mbps: Set(None),
			write_speed_mbps: Set(None),
			last_speed_test_at: Set(None),
			file_system: Set(Some("ext4".to_string())),
			mount_point: Set(Some(format!("/mnt/{}", name))),
			is_removable: Set(Some(false)),
			is_network_drive: Set(Some(false)),
			device_model: Set(None),
			volume_type: Set(Some("local".to_string())),
			is_user_visible: Set(Some(true)),
			auto_track_eligible: Set(Some(true)),
			cloud_identifier: Set(None),
		};

		Ok(volume.insert(self.library_a.db().conn()).await?)
	}

	async fn count_locations(&self, library: &Arc<Library>) -> anyhow::Result<u64> {
		Ok(entities::location::Entity::find()
			.count(library.db().conn())
			.await?)
	}

	async fn count_entries(&self, library: &Arc<Library>) -> anyhow::Result<u64> {
		Ok(entities::entry::Entity::find()
			.count(library.db().conn())
			.await?)
	}

	async fn count_volumes(&self, library: &Arc<Library>) -> anyhow::Result<u64> {
		Ok(entities::volume::Entity::find()
			.count(library.db().conn())
			.await?)
	}

	/// Get the current watermark from Device B for Device A's data
	async fn get_device_watermark(&self) -> anyhow::Result<Option<chrono::DateTime<chrono::Utc>>> {
		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(self.device_a_id))
			.one(self.library_b.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Device A not found in Library B"))?;

		Ok(device.last_state_watermark)
	}
}

#[tokio::test]
#[ignore] // Remove this once we want to run it
async fn test_watermark_bug_with_10k_mixed_resources() -> anyhow::Result<()> {
	let setup = WatermarkTestSetup::new().await?;

	println!("\n=== Phase 1: Creating mixed resource types on Device A ===\n");

	let base_time = chrono::Utc::now() - chrono::Duration::hours(2);
	let device_a = entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(setup.device_a_id))
		.one(setup.library_a.db().conn())
		.await?
		.unwrap();

	// Create 50 locations with EARLY timestamps (00:00 - 00:05)
	println!("Creating 50 locations with timestamps at T+0 to T+5 minutes...");
	for i in 0..50 {
		let timestamp = base_time + chrono::Duration::minutes(i as i64 % 5);
		let location = setup
			.create_location_with_timestamp(&format!("Location_{}", i), device_a.id, timestamp)
			.await?;
		setup
			.library_a
			.sync_model_with_db(&location, ChangeType::Insert, setup.library_a.db().conn())
			.await?;
	}
	println!("✓ 50 locations created");

	// Skip volumes for now - they have a separate ON CONFLICT bug
	// The watermark bug will still manifest with just locations and entries
	println!("(Skipping volumes due to unrelated ON CONFLICT bug)");

	// Create 12,000 entries with LATER timestamps (00:20 onwards)
	// This will trigger multiple batches (batch size = 10,000)
	println!("Creating 12,000 entries with timestamps at T+20 minutes onwards...");
	println!("(This will be batched into multiple syncs)");
	for i in 0..12_000 {
		let timestamp =
			base_time + chrono::Duration::minutes(20) + chrono::Duration::seconds(i as i64);
		let entry = setup
			.create_entry_with_timestamp(&format!("Entry_{}", i), 0, timestamp)
			.await?;
		setup
			.library_a
			.sync_model_with_db(&entry, ChangeType::Insert, setup.library_a.db().conn())
			.await?;

		// Log progress
		if (i + 1) % 2000 == 0 {
			println!("  ... created {} entries", i + 1);
		}
	}
	println!("✓ 12,000 entries created");

	println!("\n=== Phase 2: Start initial backfill on Device B ===\n");

	// Trigger manual backfill
	println!("Starting backfill...");
	setup.trigger_manual_backfill().await?;

	// Pump messages to process first batch
	println!("Processing first batch...");
	for i in 0..50 {
		setup.pump_messages().await?;
		tokio::time::sleep(Duration::from_millis(100)).await;

		if i % 10 == 0 {
			let entry_count = setup.count_entries(&setup.library_b).await?;
			println!("  ... entries synced: {}", entry_count);
		}
	}

	// Check how much synced
	let locations_partial = setup.count_locations(&setup.library_b).await?;
	let entries_partial = setup.count_entries(&setup.library_b).await?;

	println!("\nAfter first backfill pass:");
	println!("  Locations: {}", locations_partial);
	println!("  Entries:   {}", entries_partial);

	// Get watermark after partial sync
	let watermark_after_partial = setup.get_device_watermark().await?;
	println!(
		"\nWatermark after partial sync: {:?}",
		watermark_after_partial
	);

	println!("\n=== Phase 3: Simulate disconnection and create MORE entries ===\n");

	// Create 3,000 MORE entries on Device A with even later timestamps
	println!("Creating 3,000 MORE entries on Device A (to advance watermark further)...");
	let later_time = chrono::Utc::now();
	for i in 0..3_000 {
		let timestamp = later_time + chrono::Duration::seconds(i as i64);
		let entry = setup
			.create_entry_with_timestamp(&format!("Late_Entry_{}", i), 0, timestamp)
			.await?;
		setup
			.library_a
			.sync_model_with_db(&entry, ChangeType::Insert, setup.library_a.db().conn())
			.await?;
	}
	println!("✓ Created 3,000 more entries");

	// Pump messages to sync SOME of these new entries (advancing watermark)
	println!("Syncing some of the new entries...");
	setup.wait_for_sync(Duration::from_secs(2)).await?;

	let entries_mid = setup.count_entries(&setup.library_b).await?;
	let watermark_mid = setup.get_device_watermark().await?;
	println!(
		"After syncing some new entries: {} total entries",
		entries_mid
	);
	println!("Watermark advanced to: {:?}", watermark_mid);

	println!("\n=== Phase 4: Reconnect and trigger catchup ===\n");

	// Now trigger catchup - this will use the watermark which has advanced
	// past the original locations timestamps!
	println!("Triggering watermark-based catchup...");
	let sync_b = setup.library_b.sync_service().unwrap();
	let backfill_mgr = sync_b.backfill_manager();

	// Get current watermark before catchup
	let (state_watermark_before, shared_watermark_before) =
		sync_b.peer_sync().get_watermarks().await;
	println!("Using watermark for catchup: {:?}", state_watermark_before);

	// Call catch_up_from_peer directly (watermark exchange doesn't trigger it due to TODO)
	backfill_mgr
		.catch_up_from_peer(
			setup.device_a_id,
			state_watermark_before,
			shared_watermark_before.map(|hlc| serde_json::to_string(&hlc).unwrap()),
		)
		.await?;

	// Pump messages to complete sync
	println!("Completing sync...");
	setup.wait_for_sync(Duration::from_secs(5)).await?;

	let final_watermark = setup.get_device_watermark().await?;
	println!("\nFinal watermark: {:?}", final_watermark);

	println!("\n=== Phase 5: Verify all resources synced ===\n");

	// Count what actually synced to Device B
	let locations_synced = setup.count_locations(&setup.library_b).await?;
	let entries_synced = setup.count_entries(&setup.library_b).await?;

	println!("Sync Results:");
	println!("  Locations: {} / 50 expected", locations_synced);
	println!(
		"  Entries:   {} / 15,000 expected (12k initial + 3k more)",
		entries_synced
	);

	println!("\n=== Assertions ===\n");

	// These assertions should FAIL with the current global watermark implementation
	// because locations have earlier timestamps than the watermark
	// which was advanced by the entry batches

	assert_eq!(
		locations_synced, 50,
		"BUG DETECTED: Only {} locations synced (expected 50). \
		 Locations have earlier timestamps than the watermark that was advanced by entries!",
		locations_synced
	);

	assert_eq!(
		entries_synced, 15_000,
		"Expected all 15,000 entries to sync (12k initial + 3k more)"
	);

	println!("All resource types synced correctly!");
	println!("(If you see this, the bug is fixed!)");

	Ok(())
}
