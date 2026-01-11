//! Real-Time Sync Integration Test
//!
//! Automated testing of sync scenarios with two devices (Alice & Bob).
//! Each test run captures complete snapshots for analysis.
//!
//! ## Features
//! - Pre-paired devices (Alice & Bob)
//! - Indexes Spacedrive source code for deterministic testing
//! - Event-driven architecture
//! - Captures sync logs, databases, and event bus events
//! - Timestamped snapshot folders for each run
//!
//! ## Running Tests
//! ```bash
//! cargo test -p sd-core --test sync_realtime_test -- --test-threads=1 --nocapture
//! ```

mod helpers;

use helpers::TwoDeviceHarnessBuilder;
use sd_core::infra::db::entities;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use tokio::time::Duration;

//
// TEST SCENARIOS
//

/// Test: Location indexed on Alice, syncs to Bob in real-time
#[tokio::test]
async fn test_realtime_sync_alice_to_bob() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("realtime_alice_to_bob")
		.await?
		.collect_events(true)
		.collect_sync_events(true)
		.build()
		.await?;

	// Phase 1: Add location on Alice
	tracing::info!("=== Phase 1: Adding location on Alice ===");

	// Use Spacedrive source code for deterministic testing across all environments
	let test_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let location_uuid = harness
		.add_and_index_location_alice(test_path.to_str().unwrap(), "spacedrive")
		.await?;

	tracing::info!(
		location_uuid = %location_uuid,
		"Location and root entry created on Alice"
	);

	// Small delay for initial sync
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Phase 2: Sync to Bob
	tracing::info!("=== Phase 2: Syncing to Bob ===");

	let sync_result = harness.wait_for_sync(Duration::from_secs(120)).await;

	// Always capture snapshot
	tracing::info!("=== Phase 3: Capturing snapshot ===");
	harness.capture_snapshot("final_state").await?;

	// Check sync result
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

	// Assertions
	let entry_diff = (entries_alice as i64 - entries_bob as i64).abs();
	assert!(
		entry_diff <= 5,
		"Entry count mismatch: Alice {}, Bob {} (diff: {})",
		entries_alice,
		entries_bob,
		entry_diff
	);

	let content_diff = (content_ids_alice as i64 - content_ids_bob as i64).abs();
	assert!(
		content_diff <= 5,
		"Content identity mismatch: Alice {}, Bob {} (diff: {})",
		content_ids_alice,
		content_ids_bob,
		content_diff
	);

	// Check content_id linkage
	let orphaned_content_bob = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::Size.gt(0))
		.filter(entities::entry::Column::ContentId.is_null())
		.count(harness.library_bob.db().conn())
		.await?;

	let total_files = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::Size.gt(0))
		.count(harness.library_bob.db().conn())
		.await?;

	let max_allowed_orphaned_content = ((total_files as f64) * 0.05).ceil() as u64;

	assert!(
		orphaned_content_bob <= max_allowed_orphaned_content,
		"Too many files without content_id on Bob: {}/{} ({:.1}%)",
		orphaned_content_bob,
		total_files,
		(orphaned_content_bob as f64 / total_files as f64) * 100.0
	);

	// CRITICAL: Check for orphaned parent_id entries (the actual sync bug)
	// Files and subdirectories should NEVER have NULL parent_id (except location roots)
	let orphaned_hierarchy_bob = entities::entry::Entity::find()
		.filter(entities::entry::Column::ParentId.is_null())
		.filter(entities::entry::Column::Kind.eq(0)) // Files only
		.count(harness.library_bob.db().conn())
		.await?;

	assert_eq!(
		orphaned_hierarchy_bob, 0,
		"SYNC BUG: Found {} files with NULL parent_id on Bob (should be 0). \
		This indicates batch FK remapping failed because children arrived before parents.",
		orphaned_hierarchy_bob
	);

	// Also check for duplicate entries (same name/content, different UUID)
	// This would indicate re-creation of entries that already exist
	use sea_orm::sea_query::Query;
	use sea_orm::FromQueryResult;

	#[derive(Debug, FromQueryResult)]
	struct DuplicateCount {
		dup_count: i64,
	}

	let duplicate_check = sea_orm::Statement::from_sql_and_values(
		sea_orm::DbBackend::Sqlite,
		r#"
			SELECT COUNT(*) as dup_count
			FROM (
				SELECT e1.name, e1.extension, e1.volume_id, COUNT(*) as cnt
				FROM entries e1
				JOIN content_identities ci ON e1.content_id = ci.id
				WHERE e1.kind = 0
				GROUP BY e1.name, e1.extension, e1.volume_id, ci.content_hash
				HAVING COUNT(*) > 1
			)
		"#,
		vec![],
	);

	let dup_result = DuplicateCount::find_by_statement(duplicate_check)
		.one(harness.library_bob.db().conn())
		.await?;

	if let Some(result) = dup_result {
		assert_eq!(
			result.dup_count, 0,
			"SYNC BUG: Found {} sets of duplicate files on Bob (same content, different UUIDs). \
			This indicates entries were re-created instead of properly FK-mapped.",
			result.dup_count
		);
	}

	Ok(())
}

/// Test: Location indexed on Bob, syncs to Alice (reverse direction)
#[tokio::test]
async fn test_realtime_sync_bob_to_alice() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("realtime_bob_to_alice")
		.await?
		.build()
		.await?;

	// Add location on Bob (reverse direction)
	// Use Spacedrive crates directory for deterministic testing
	let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let crates_path = project_root.join("crates");
	harness
		.add_and_index_location_bob(crates_path.to_str().unwrap(), "crates")
		.await?;

	// Wait for sync
	harness.wait_for_sync(Duration::from_secs(60)).await?;

	// Capture snapshot
	harness.capture_snapshot("final_state").await?;

	// Verify bidirectional sync works
	let entries_alice = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let entries_bob = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	let diff = (entries_alice as i64 - entries_bob as i64).abs();
	assert!(
		diff <= 5,
		"Bidirectional sync failed: Alice {}, Bob {} (diff: {})",
		entries_alice,
		entries_bob,
		diff
	);

	// Check for orphaned parent_id on Alice (reverse direction)
	let orphaned_alice = entities::entry::Entity::find()
		.filter(entities::entry::Column::ParentId.is_null())
		.filter(entities::entry::Column::Kind.eq(0))
		.count(harness.library_alice.db().conn())
		.await?;

	assert_eq!(
		orphaned_alice, 0,
		"SYNC BUG: Found {} files with NULL parent_id on Alice (should be 0)",
		orphaned_alice
	);

	Ok(())
}

/// Test: Concurrent indexing on both devices
#[tokio::test]
async fn test_concurrent_indexing() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("concurrent_indexing")
		.await?
		.build()
		.await?;

	// Add different locations on both devices simultaneously
	// Use Spacedrive source code for deterministic testing
	let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let core_path = project_root.join("core");
	let apps_path = project_root.join("apps");

	// Start indexing on both
	let alice_task = harness.add_and_index_location_alice(core_path.to_str().unwrap(), "core");
	let bob_task = harness.add_and_index_location_bob(apps_path.to_str().unwrap(), "apps");

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

	// Check for orphaned parent_id on both devices
	let orphaned_alice = entities::entry::Entity::find()
		.filter(entities::entry::Column::ParentId.is_null())
		.filter(entities::entry::Column::Kind.eq(0))
		.count(harness.library_alice.db().conn())
		.await?;
	let orphaned_bob = entities::entry::Entity::find()
		.filter(entities::entry::Column::ParentId.is_null())
		.filter(entities::entry::Column::Kind.eq(0))
		.count(harness.library_bob.db().conn())
		.await?;

	assert_eq!(
		orphaned_alice, 0,
		"SYNC BUG: Found {} files with NULL parent_id on Alice",
		orphaned_alice
	);
	assert_eq!(
		orphaned_bob, 0,
		"SYNC BUG: Found {} files with NULL parent_id on Bob",
		orphaned_bob
	);

	Ok(())
}

/// Test: Content identity linkage syncs correctly
#[tokio::test]
async fn test_content_identity_linkage() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("content_identity_linkage")
		.await?
		.build()
		.await?;

	// Index on Alice
	// Use Spacedrive docs directory for deterministic testing
	let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let docs_path = project_root.join("docs");
	harness
		.add_and_index_location_alice(docs_path.to_str().unwrap(), "docs")
		.await?;

	// Wait for content identification to complete
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Sync
	harness.wait_for_sync(Duration::from_secs(60)).await?;

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

	// Check for orphaned parent_id
	let orphaned_bob = entities::entry::Entity::find()
		.filter(entities::entry::Column::ParentId.is_null())
		.filter(entities::entry::Column::Kind.eq(0))
		.count(harness.library_bob.db().conn())
		.await?;

	assert_eq!(
		orphaned_bob, 0,
		"SYNC BUG: Found {} files with NULL parent_id on Bob",
		orphaned_bob
	);

	Ok(())
}
