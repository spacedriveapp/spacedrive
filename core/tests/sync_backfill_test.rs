//! Initial Backfill Sync Test
//!
//! Tests the scenario where one device indexes completely before the second device connects.
//! This validates backfill behavior and content_id linkage without real-time sync complexity.

mod helpers;

use helpers::MockTransport;
use sd_core::{
	infra::{db::entities, sync::NetworkTransport},
	library::Library,
	service::Service,
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use std::sync::Arc;
use tokio::{fs, time::Duration};
use uuid::Uuid;

fn init_tracing(test_name: &str, snapshot_dir: &std::path::Path) -> anyhow::Result<()> {
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
				 sync_backfill_test=debug,\
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

fn create_test_config(data_dir: &std::path::Path) -> anyhow::Result<sd_core::config::AppConfig> {
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
		logging: logging_config,
		data_dir: data_dir.to_path_buf(),
		log_level: "debug".to_string(),
		telemetry_enabled: false,
		preferences: sd_core::config::Preferences::default(),
		job_logging: sd_core::config::JobLoggingConfig::default(),
		services: sd_core::config::ServiceConfig {
			networking_enabled: false,
			volume_monitoring_enabled: false,
			fs_watcher_enabled: false,
		},
	};

	config.save()?;

	Ok(config)
}

async fn wait_for_indexing(library: &Arc<Library>, _location_id: i32) -> anyhow::Result<()> {
	use sd_core::infra::job::JobStatus;

	let start_time = tokio::time::Instant::now();
	let timeout_duration = Duration::from_secs(120);

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
					break;
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

	device_model.insert(library.db().conn()).await?;
	Ok(())
}

/// Create a mock volume for testing
async fn create_test_volume(
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

#[tokio::test]
async fn test_initial_backfill_alice_indexes_first() -> anyhow::Result<()> {
	let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
	let test_root =
		std::path::PathBuf::from(home).join("Library/Application Support/spacedrive/sync_tests");

	let data_dir = test_root.join("data");
	fs::create_dir_all(&data_dir).await?;

	let temp_dir_alice = data_dir.join("alice_backfill");
	let temp_dir_bob = data_dir.join("bob_backfill");
	fs::create_dir_all(&temp_dir_alice).await?;
	fs::create_dir_all(&temp_dir_bob).await?;

	let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
	let snapshot_dir = test_root
		.join("snapshots")
		.join(format!("backfill_alice_first_{}", timestamp));
	fs::create_dir_all(&snapshot_dir).await?;

	init_tracing("backfill_alice_first", &snapshot_dir)?;

	tracing::info!(
		test_root = %test_root.display(),
		snapshot_dir = %snapshot_dir.display(),
		alice_dir = %temp_dir_alice.display(),
		bob_dir = %temp_dir_bob.display(),
		"Test directories initialized"
	);

	create_test_config(&temp_dir_alice)?;
	create_test_config(&temp_dir_bob)?;

	tracing::info!("=== Phase 1: Alice indexes location (Bob not connected yet) ===");

	let core_alice = Core::new(temp_dir_alice.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
	let device_alice_id = core_alice.device.device_id()?;
	let library_alice = core_alice
		.libraries
		.create_library_no_sync("Backfill Test Library", None, core_alice.context.clone())
		.await?;

	use sd_core::location::{create_location, IndexMode, LocationCreateArgs};

	let device_record = entities::device::Entity::find()
		.one(library_alice.db().conn())
		.await?
		.ok_or_else(|| anyhow::anyhow!("Device not found"))?;

	let downloads_path = std::env::var("HOME").unwrap() + "/Desktop";
	let location_args = LocationCreateArgs {
		path: std::path::PathBuf::from(&downloads_path),
		name: Some("Desktop".to_string()),
		index_mode: IndexMode::Content,
	};

	let location_db_id = create_location(
		library_alice.clone(),
		library_alice.event_bus(),
		location_args,
		device_record.id,
	)
	.await?;

	tracing::info!(
		location_id = location_db_id,
		"Location created on Alice, waiting for indexing"
	);

	wait_for_indexing(&library_alice, location_db_id).await?;

	let alice_entries_after_index = entities::entry::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let alice_content_after_index = entities::content_identity::Entity::find()
		.count(library_alice.db().conn())
		.await?;

	tracing::info!(
		entries = alice_entries_after_index,
		content_identities = alice_content_after_index,
		"Alice indexing complete"
	);

	// Add some volumes to Alice before Bob connects
	tracing::info!("Adding test volumes to Alice");
	create_test_volume(
		&library_alice,
		device_alice_id,
		"test-vol-1",
		"Alice Volume 1",
	)
	.await?;
	create_test_volume(
		&library_alice,
		device_alice_id,
		"test-vol-2",
		"Alice Volume 2",
	)
	.await?;

	let alice_volumes = entities::volume::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	tracing::info!(volumes = alice_volumes, "Alice has tracked volumes");

	tracing::info!("=== Phase 2: Bob connects and starts backfill ===");

	let core_bob = Core::new(temp_dir_bob.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
	let device_bob_id = core_bob.device.device_id()?;
	let library_bob = core_bob
		.libraries
		.create_library_no_sync("Backfill Test Library", None, core_bob.context.clone())
		.await?;

	register_device(&library_alice, device_bob_id, "Bob").await?;
	register_device(&library_bob, device_alice_id, "Alice").await?;

	let (transport_alice, transport_bob) = MockTransport::new_pair(device_alice_id, device_bob_id);

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

	tracing::info!("Sync services started - backfill should begin automatically");

	// Give sync loop a moment to start
	tokio::time::sleep(Duration::from_millis(500)).await;

	let bob_state = library_bob
		.sync_service()
		.unwrap()
		.peer_sync()
		.state()
		.await;
	let alice_state = library_alice
		.sync_service()
		.unwrap()
		.peer_sync()
		.state()
		.await;

	tracing::info!(
		bob_state = ?bob_state,
		alice_state = ?alice_state,
		"Initial sync states after startup"
	);

	// Check if Bob can see Alice as a connected partner
	let partners = transport_bob
		.get_connected_sync_partners(library_bob.id(), library_bob.db().conn())
		.await?;

	tracing::info!(
		partners = ?partners,
		alice_device = %device_alice_id,
		bob_device = %device_bob_id,
		"Bob's view of connected sync partners"
	);

	if partners.is_empty() {
		anyhow::bail!("Bob cannot see any connected partners! Backfill won't trigger.");
	}

	tracing::info!("=== Phase 3: Waiting for backfill to complete ===");

	let start = tokio::time::Instant::now();
	let max_duration = Duration::from_secs(60);
	let mut last_bob_entries = 0;
	let mut last_bob_content = 0;
	let mut stable_iterations = 0;
	let mut no_progress_iterations = 0;

	while start.elapsed() < max_duration {
		let bob_entries = entities::entry::Entity::find()
			.count(library_bob.db().conn())
			.await?;
		let bob_content = entities::content_identity::Entity::find()
			.count(library_bob.db().conn())
			.await?;

		let bob_state = library_bob
			.sync_service()
			.unwrap()
			.peer_sync()
			.state()
			.await;

		// Check if we're making progress
		if bob_entries == last_bob_entries && bob_content == last_bob_content {
			no_progress_iterations += 1;
			if no_progress_iterations >= 20 {
				tracing::warn!(
					bob_entries = bob_entries,
					alice_entries = alice_entries_after_index,
					bob_state = ?bob_state,
					elapsed_secs = start.elapsed().as_secs(),
					"No progress for 20 iterations - backfill may be stuck"
				);
			}
		} else {
			no_progress_iterations = 0;
		}

		// Check if sync is complete
		if bob_entries == alice_entries_after_index && bob_content == alice_content_after_index {
			stable_iterations += 1;
			if stable_iterations >= 5 {
				tracing::info!(
					duration_ms = start.elapsed().as_millis(),
					bob_entries = bob_entries,
					bob_content = bob_content,
					bob_state = ?bob_state,
					"Backfill complete and stable"
				);
				break;
			}
		} else {
			stable_iterations = 0;
		}

		if bob_entries != last_bob_entries || bob_content != last_bob_content {
			let entry_progress = if alice_entries_after_index > 0 {
				(bob_entries as f64 / alice_entries_after_index as f64 * 100.0)
			} else {
				0.0
			};
			let content_progress = if alice_content_after_index > 0 {
				(bob_content as f64 / alice_content_after_index as f64 * 100.0)
			} else {
				0.0
			};

			tracing::info!(
				bob_entries = bob_entries,
				bob_content = bob_content,
				alice_entries = alice_entries_after_index,
				alice_content = alice_content_after_index,
				entry_progress_pct = format!("{:.1}", entry_progress),
				content_progress_pct = format!("{:.1}", content_progress),
				bob_state = ?bob_state,
				elapsed_secs = start.elapsed().as_secs(),
				"Backfill in progress"
			);
		}

		last_bob_entries = bob_entries;
		last_bob_content = bob_content;

		tokio::time::sleep(Duration::from_millis(100)).await;
	}

	let bob_entries_final = entities::entry::Entity::find()
		.count(library_bob.db().conn())
		.await?;
	let bob_content_final = entities::content_identity::Entity::find()
		.count(library_bob.db().conn())
		.await?;
	let alice_volumes_final = entities::volume::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let bob_volumes_final = entities::volume::Entity::find()
		.count(library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_entries = alice_entries_after_index,
		bob_entries = bob_entries_final,
		alice_content = alice_content_after_index,
		bob_content = bob_content_final,
		alice_volumes = alice_volumes_final,
		bob_volumes = bob_volumes_final,
		"=== Final counts ==="
	);

	let entry_diff = (alice_entries_after_index as i64 - bob_entries_final as i64).abs();
	let content_diff = (alice_content_after_index as i64 - bob_content_final as i64).abs();

	assert!(
		entry_diff <= 5,
		"Entry count mismatch after backfill: Alice has {}, Bob has {} (diff: {})",
		alice_entries_after_index,
		bob_entries_final,
		entry_diff
	);

	assert!(
		content_diff <= 5,
		"Content identity count mismatch after backfill: Alice has {}, Bob has {} (diff: {})",
		alice_content_after_index,
		bob_content_final,
		content_diff
	);

	// Verify volume sync
	assert_eq!(
		alice_volumes_final, bob_volumes_final,
		"Volume count mismatch after backfill: Alice has {}, Bob has {}",
		alice_volumes_final, bob_volumes_final
	);

	tracing::info!(
		alice_volumes = alice_volumes_final,
		bob_volumes = bob_volumes_final,
		"Volume sync verification passed"
	);

	let bob_files_linked = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::ContentId.is_not_null())
		.count(library_bob.db().conn())
		.await?;
	let bob_total_files = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.count(library_bob.db().conn())
		.await?;

	let linkage_pct = if bob_total_files > 0 {
		(bob_files_linked * 100) / bob_total_files
	} else {
		0
	};

	tracing::info!(
		bob_files_linked = bob_files_linked,
		bob_total_files = bob_total_files,
		linkage_pct = linkage_pct,
		"Bob's content_id linkage after backfill"
	);

	assert!(
		linkage_pct >= 90,
		"Content_id linkage too low on Bob after backfill: {}% (expected >= 90%)",
		linkage_pct
	);

	Ok(())
}
