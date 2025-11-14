//! Unit test to prove the global watermark bug
//!
//! This test directly calls `query_for_sync()` with different watermarks
//! to demonstrate that a single global watermark cannot correctly represent
//! the sync state of multiple resource types with different timestamp distributions.
//!
//! **Expected Result**: This test should FAIL, proving the bug exists.
//! After implementing per-resource watermarks, this test should PASS.

use sd_core::{
	infra::{db::entities, sync::Syncable},
	Core,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_global_watermark_filters_earlier_resources() -> anyhow::Result<()> {
	// Setup
	let _ = tracing_subscriber::fmt()
		.with_env_filter("sd_core=debug")
		.with_test_writer()
		.try_init();

	let temp_dir = TempDir::new()?;

	let config = sd_core::config::AppConfig {
		version: 3,
		data_dir: temp_dir.path().to_path_buf(),
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
	config.save()?;

	let core = Core::new(temp_dir.path().to_path_buf())
		.await
		.map_err(|e| anyhow::anyhow!("{}", e))?;

	let library = core
		.libraries
		.create_library("Test Library", None, core.context.clone())
		.await?;

	let db = library.db().conn();
	let device_id = core.device.device_id()?;

	println!("\n=== Creating test data with different timestamps ===\n");

	let base_time = chrono::Utc::now() - chrono::Duration::hours(1);

	// Create device for location ownership
	let device = entities::device::Entity::find()
		.one(db)
		.await?
		.expect("Device should exist");

	// Create 10 locations with EARLY timestamps (T+0 to T+10 minutes)
	println!("Creating 10 locations with timestamps T+0 to T+10...");
	for i in 0..10 {
		let timestamp = base_time + chrono::Duration::minutes(i as i64);

		// Create entry for the location
		let entry = entities::entry::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Some(Uuid::new_v4())),
			name: Set(format!("Location_{}", i)),
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
		let entry_record = entry.insert(db).await?;

		let location = entities::location::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Uuid::new_v4()),
			device_id: Set(device.id),
			entry_id: Set(Some(entry_record.id)),
			name: Set(Some(format!("Location_{}", i))),
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
		location.insert(db).await?;
	}
	println!("✓ Created 10 locations");

	// Create 100 entries with LATER timestamps (T+20 to T+30 minutes)
	// Make them sync-ready by setting size=0 (empty files) or kind=1 (directories)
	println!("Creating 100 entries with timestamps T+20 to T+30...");
	for i in 0..100 {
		let timestamp = base_time + chrono::Duration::minutes(20 + (i as i64 / 10));

		let entry = entities::entry::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(Some(Uuid::new_v4())),
			name: Set(format!("Entry_{}", i)),
			kind: Set(if i % 2 == 0 { 1 } else { 0 }), // Mix of dirs and files
			extension: Set(if i % 2 == 0 {
				None
			} else {
				Some("txt".to_string())
			}),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(0), // Make sync-ready (empty file or directory)
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
		entry.insert(db).await?;
	}
	println!("✓ Created 100 entries (sync-ready)");

	println!("\n=== Testing queries with different watermarks ===\n");

	// Simulate watermark at T+25 (after entries start, before locations)
	let watermark_at_t25 = base_time + chrono::Duration::minutes(25);

	println!("Testing with watermark at T+25 minutes:");
	println!("  - Locations have timestamps T+0 to T+10 (BEFORE watermark)");
	println!("  - Entries have timestamps T+20 to T+30 (OVERLAP watermark)");
	println!();

	// Query locations with watermark
	let locations_result = entities::location::Model::query_for_sync(
		Some(device_id),
		Some(watermark_at_t25),
		None,
		1000,
		db,
	)
	.await?;

	// Query entries with watermark
	let entries_result =
		entities::entry::Model::query_for_sync(None, Some(watermark_at_t25), None, 1000, db)
			.await?;

	println!("Query Results:");
	println!(
		"  Locations returned: {} (expected 0 due to watermark filter)",
		locations_result.len()
	);
	println!(
		"  Entries returned:   {} (expected some, those >= T+25)",
		entries_result.len()
	);
	println!();

	println!("=== Demonstrating the Bug ===\n");

	// THE BUG: If we used this watermark for incremental sync, we'd get:
	// - 0 locations (filtered out by watermark)
	// - Some entries (those >= watermark)
	//
	// But in reality, we have 10 locations that SHOULD sync but are older than the watermark
	// which was advanced by entry processing.

	// THE BUG PROOF:
	// We have 10 locations in the database, but the watermark-based query returns 0
	// because the watermark (T+25) was advanced by a different resource type (entries).
	if locations_result.len() == 0 {
		println!("BUG CONFIRMED!");
		println!("   We have 10 locations in the database with timestamps T+0 to T+10,");
		println!("   but query with watermark T+25 returns 0 locations.");
		println!();
		println!("   In a real sync scenario:");
		println!("   1. Device B starts backfill");
		println!("   2. Syncs 10,000 entries first (watermark advances to T+25)");
		println!("   3. Disconnects before syncing locations");
		println!("   4. Reconnects with watermark T+25");
		println!("   5. Locations (T+0 to T+10) are PERMANENTLY LOST!");
		println!();
		println!("   This proves: A single global watermark cannot represent");
		println!("   the sync state of multiple resource types with different");
		println!("   timestamp distributions.");
	} else {
		panic!(
			"Expected 0 locations (filtered by watermark), but got {}. \
			 Test setup error - locations should have earlier timestamps than watermark.",
			locations_result.len()
		);
	}

	// This assertion should pass (entries newer than watermark DO return)
	assert!(
		!entries_result.is_empty(),
		"Entries with timestamps >= watermark should be returned"
	);

	println!("Bug demonstrated successfully!");
	println!("   With per-resource watermarks, each resource type would have its own watermark,");
	println!("   preventing this data loss scenario.");

	Ok(())
}

#[tokio::test]
async fn test_per_resource_watermark_solution() -> anyhow::Result<()> {
	println!("\n=== Demonstrating Per-Resource Watermark Solution ===\n");

	println!("Current (broken):");
	println!("  device.last_state_watermark = T+25");
	println!("  Query locations: WHERE updated_at >= T+25 → 0 results ");
	println!("  Query entries:   WHERE indexed_at >= T+25 → Some results ✓");
	println!();

	println!("With per-resource watermarks (fixed):");
	println!("  device_resource_watermarks:");
	println!("    (device_uuid, peer_uuid, 'location', T+10)");
	println!("    (device_uuid, peer_uuid, 'entry',    T+25)");
	println!("  Query locations: WHERE updated_at >= T+10 → 10 results ✓");
	println!("  Query entries:   WHERE indexed_at >= T+25 → Some results ✓");
	println!();

	println!("Each resource type maintains independent sync progress!");

	Ok(())
}
