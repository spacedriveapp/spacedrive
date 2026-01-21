//! Watcher integration test for moving folders into managed locations
//!
//! Verifies that when a folder tree is moved from outside a managed location into it,
//! the filesystem watcher correctly:
//! - Detects the new folder and its contents
//! - Creates entries with proper parent-child relationships
//! - Avoids creating duplicate entries
//! - Maintains correct hierarchy (subfolders point to their parent, not the location root)

mod helpers;

use helpers::IndexingHarnessBuilder;
use sd_core::{infra::db::entities, location::IndexMode};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tokio::{fs, time::Duration};

/// Verifies watcher correctly handles moving external folders into managed locations
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_watcher_detects_external_folder_move() -> anyhow::Result<()> {
	// Build harness with watcher enabled
	let harness = IndexingHarnessBuilder::new("move_folder_reindex")
		.build()
		.await?;

	// Create managed location directory
	let managed_location = harness.create_test_location("ManagedLocation").await?;

	// Create folder structure OUTSIDE the managed location
	let outside_dir = harness.temp_path().join("outside");
	fs::create_dir_all(&outside_dir).await?;

	let test_folder = outside_dir.join("TestFolder");
	fs::create_dir_all(&test_folder).await?;

	let subfolder1 = test_folder.join("SubFolder1");
	fs::create_dir_all(&subfolder1).await?;
	fs::write(subfolder1.join("file1.txt"), b"test content 1").await?;
	fs::write(subfolder1.join("file2.txt"), b"test content 2").await?;

	let subfolder2 = test_folder.join("SubFolder2");
	fs::create_dir_all(&subfolder2).await?;
	fs::write(subfolder2.join("file3.txt"), b"test content 3").await?;
	fs::write(subfolder2.join("file4.txt"), b"test content 4").await?;
	fs::write(test_folder.join("root_file.txt"), b"root content").await?;

	// Add location to library and index it (this registers with watcher)
	let location = managed_location
		.index("ManagedLocation", IndexMode::Shallow)
		.await?;

	// Wait for indexing to settle
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Move TestFolder INTO the managed location
	let destination = location.path.join("TestFolder");
	fs::rename(&test_folder, &destination).await?;

	// Wait for watcher to detect and process the move (matches file_move_test pattern)
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Verify database integrity
	let conn = harness.library.db().conn();

	// Find TestFolder entry
	let test_folder_entry = entities::entry::Entity::find()
		.filter(entities::entry::Column::Name.eq("TestFolder"))
		.one(conn)
		.await?
		.expect("TestFolder should exist in database");

	// Find subfolders
	let subfolder1 = entities::entry::Entity::find()
		.filter(entities::entry::Column::Name.eq("SubFolder1"))
		.one(conn)
		.await?
		.expect("SubFolder1 should exist");

	let subfolder2 = entities::entry::Entity::find()
		.filter(entities::entry::Column::Name.eq("SubFolder2"))
		.one(conn)
		.await?
		.expect("SubFolder2 should exist");

	// CRITICAL ASSERTION: Subfolders should have TestFolder as parent, not the location root
	assert_eq!(
		subfolder1.parent_id,
		Some(test_folder_entry.id),
		"SubFolder1 should have TestFolder (ID: {}) as parent, but has {:?}",
		test_folder_entry.id,
		subfolder1.parent_id
	);

	assert_eq!(
		subfolder2.parent_id,
		Some(test_folder_entry.id),
		"SubFolder2 should have TestFolder (ID: {}) as parent, but has {:?}",
		test_folder_entry.id,
		subfolder2.parent_id
	);

	// Verify no duplicate entries
	let all_entries = entities::entry::Entity::find().all(conn).await?;
	let mut name_counts: std::collections::HashMap<String, usize> =
		std::collections::HashMap::new();
	for entry in &all_entries {
		*name_counts.entry(entry.name.clone()).or_insert(0) += 1;
	}

	for (name, count) in name_counts.iter() {
		assert_eq!(
			*count, 1,
			"Entry '{}' appears {} times (should be 1)",
			name, count
		);
	}

	harness.shutdown().await?;
	Ok(())
}
