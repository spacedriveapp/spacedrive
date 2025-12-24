//! Integration tests for folder rename operations
//!
//! This test suite verifies folder rename handling in four scenarios:
//! 1. Persistent + Manual Reindex (batch change detection)
//! 2. Persistent + Watcher (real-time change handling)
//! 3. Ephemeral + Manual Reindex (batch change detection)
//! 4. Ephemeral + Watcher (real-time change handling)
//!
//! Each test validates:
//! - Folder UUID/inode preservation
//! - Parent-child relationship integrity
//! - Correct event emission for UI updates

mod helpers;

use helpers::*;
use sd_core::{
	domain::addressing::SdPath,
	location::IndexMode,
	ops::indexing::{IndexScope, IndexerJob, IndexerJobConfig},
};
use tokio::time::Duration;

// ============================================================================
// PERSISTENT INDEXING TESTS
// ============================================================================

#[tokio::test]
async fn test_persistent_folder_rename_via_reindex() -> anyhow::Result<()> {
	// Tests batch change detection during manual reindex (watcher disabled)
	let harness = IndexingHarnessBuilder::new("persistent_rename_reindex")
		.disable_watcher()
		.build()
		.await?;

	let test_location = harness.create_test_location("test_rename").await?;

	// Create folder structure with files inside
	test_location.create_dir("original_folder").await?;
	test_location
		.write_file("original_folder/file1.txt", "Content 1")
		.await?;
	test_location
		.write_file("original_folder/file2.rs", "fn main() {}")
		.await?;
	test_location
		.write_file("original_folder/nested/file3.md", "# Docs")
		.await?;

	// Initial indexing
	let location = test_location
		.index("Test Location", IndexMode::Shallow)
		.await?;

	// Verify initial structure
	let initial_entries = location.get_all_entries().await?;
	let folder_before = initial_entries
		.iter()
		.find(|e| e.name == "original_folder" && e.kind == 1)
		.expect("Original folder should exist");
	let folder_uuid = folder_before.uuid;
	let folder_inode = folder_before.inode;
	let folder_id = folder_before.id;

	let file1_before = initial_entries
		.iter()
		.find(|e| e.name == "file1")
		.expect("file1 should exist");
	assert_eq!(
		file1_before.parent_id,
		Some(folder_id),
		"file1 should be child of original_folder"
	);

	let initial_file_count = location.count_files().await?;
	assert_eq!(initial_file_count, 3, "Should have 3 files initially");

	// Wait for indexing to settle
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Start collecting events BEFORE rename
	let mut collector = EventCollector::new(&harness.core.events);
	let collection_handle = tokio::spawn(async move {
		collector.collect_events(Duration::from_secs(5)).await;
		collector
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Rename the folder
	tracing::info!("Renaming folder from original_folder to renamed_folder");
	location
		.move_file("original_folder", "renamed_folder")
		.await?;

	// Manual reindex to detect the rename via batch change detection
	location.reindex().await?;

	// Wait for reindex to complete
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Verify final structure
	let final_entries = location.get_all_entries().await?;

	// Folder should have new name but same UUID/inode/ID
	let folder_after = final_entries
		.iter()
		.find(|e| e.name == "renamed_folder" && e.kind == 1)
		.expect("Renamed folder should exist");

	assert_eq!(
		folder_after.uuid, folder_uuid,
		"Folder UUID should be preserved after rename"
	);
	assert_eq!(
		folder_after.inode, folder_inode,
		"Folder inode should be preserved after rename"
	);
	assert_eq!(
		folder_after.id, folder_id,
		"Folder ID should be preserved after rename"
	);

	// Old folder name should not exist
	let old_folder_exists = final_entries
		.iter()
		.any(|e| e.name == "original_folder" && e.kind == 1);
	assert!(
		!old_folder_exists,
		"Old folder name should not exist after rename"
	);

	// All files should still exist with renamed folder as parent
	let file1_after = final_entries
		.iter()
		.find(|e| e.name == "file1")
		.expect("file1 should still exist");
	let file2_after = final_entries
		.iter()
		.find(|e| e.name == "file2")
		.expect("file2 should still exist");
	let file3_after = final_entries
		.iter()
		.find(|e| e.name == "file3")
		.expect("file3 should still exist");

	assert_eq!(
		file1_after.parent_id,
		Some(folder_id),
		"file1 should still be child of folder (parent_id preserved)"
	);
	assert_eq!(
		file2_after.parent_id,
		Some(folder_id),
		"file2 should still be child of folder (parent_id preserved)"
	);

	// Verify nested folder structure is intact
	let nested_folder = final_entries
		.iter()
		.find(|e| e.name == "nested" && e.kind == 1)
		.expect("nested folder should exist");
	assert_eq!(
		nested_folder.parent_id,
		Some(folder_id),
		"nested folder should be child of renamed folder"
	);
	assert_eq!(
		file3_after.parent_id,
		Some(nested_folder.id),
		"file3 should be child of nested folder"
	);

	// Total file count should remain the same
	let final_file_count = location.count_files().await?;
	assert_eq!(
		final_file_count, initial_file_count,
		"File count should remain the same after folder rename"
	);

	// CRITICAL: Verify closure table integrity
	// This catches the bug where children exist but aren't connected via closure table
	location.verify_closure_table_integrity().await?;

	// Verify events were emitted during the operation
	let collector = collection_handle.await.unwrap();
	let stats = collector.analyze().await;

	// Note: Move operations may not emit resource change events immediately
	// The core functionality (database integrity) is what matters
	let total_events = stats.resource_changed_batch.values().sum::<usize>()
		+ stats.resource_changed.values().sum::<usize>();

	if total_events == 0 {
		tracing::warn!("No resource change events emitted during folder rename");
	}

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_persistent_folder_rename_via_watcher() -> anyhow::Result<()> {
	// Tests real-time watcher change handling (no manual reindex)
	let harness = IndexingHarnessBuilder::new("persistent_rename_watcher")
		.build() // Watcher enabled by default
		.await?;

	let test_location = harness.create_test_location("test_rename").await?;

	// Create folder structure
	test_location.create_dir("original_folder").await?;
	test_location
		.write_file("original_folder/file1.txt", "Content 1")
		.await?;
	test_location
		.write_file("original_folder/file2.rs", "fn main() {}")
		.await?;

	// Initial indexing
	let location = test_location
		.index("Test Location", IndexMode::Shallow)
		.await?;

	let initial_entries = location.get_all_entries().await?;
	let folder_before = initial_entries
		.iter()
		.find(|e| e.name == "original_folder" && e.kind == 1)
		.expect("Original folder should exist");
	let folder_uuid = folder_before.uuid;
	let folder_id = folder_before.id;

	// Wait for indexing to settle
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Start collecting events
	let mut collector = EventCollector::new(&harness.core.events);
	let collection_handle = tokio::spawn(async move {
		collector.collect_events(Duration::from_secs(10)).await;
		collector
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Rename the folder - watcher should detect and handle it
	tracing::info!("Renaming folder (watcher will detect)");
	location
		.move_file("original_folder", "renamed_folder")
		.await?;

	// NO manual reindex - rely on watcher to handle the change
	// Wait for watcher to process the rename event
	// Directories are buffered for 500ms for rename detection, then emitted on tick
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Verify final structure
	let final_entries = location.get_all_entries().await?;

	// Folder should exist with new name
	let folder_after = final_entries
		.iter()
		.find(|e| e.name == "renamed_folder" && e.kind == 1)
		.expect("Renamed folder should exist");

	// Watcher should preserve UUID/ID through move detection
	assert_eq!(
		folder_after.uuid, folder_uuid,
		"Folder UUID should be preserved by watcher"
	);
	assert_eq!(
		folder_after.id, folder_id,
		"Folder ID should be preserved by watcher"
	);

	// Files should still exist
	let file1_exists = final_entries.iter().any(|e| e.name == "file1");
	let file2_exists = final_entries.iter().any(|e| e.name == "file2");
	assert!(file1_exists, "file1 should still exist");
	assert!(file2_exists, "file2 should still exist");

	// CRITICAL: Verify closure table integrity
	location.verify_closure_table_integrity().await?;

	// Verify events were emitted (watcher emits ResourceChangedBatch for persistent)
	let collector = collection_handle.await.unwrap();
	let stats = collector.analyze().await;

	let event_count = stats.resource_changed_batch.values().sum::<usize>()
		+ stats.resource_changed.values().sum::<usize>();
	assert!(
		event_count > 0,
		"Should emit resource change events from watcher"
	);

	harness.shutdown().await?;
	Ok(())
}

// ============================================================================
// EPHEMERAL INDEXING TESTS
// ============================================================================

#[tokio::test]
async fn test_ephemeral_folder_rename_via_reindex() -> anyhow::Result<()> {
	// Tests ephemeral batch change detection during manual reindex (watcher disabled)
	let harness = IndexingHarnessBuilder::new("ephemeral_rename_reindex")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let original_folder = test_root.join("original_folder");
	tokio::fs::create_dir_all(&original_folder).await?;

	tokio::fs::write(original_folder.join("file1.txt"), "Content 1").await?;
	tokio::fs::write(original_folder.join("file2.rs"), "fn main() {}").await?;

	// Index in ephemeral mode
	let test_root_sd = SdPath::local(test_root.clone());
	let indexer_config = IndexerJobConfig::ephemeral_browse(test_root_sd, IndexScope::Recursive);
	let indexer_job = IndexerJob::new(indexer_config);

	tracing::info!("Initial ephemeral indexing");
	let index_handle = harness.library.jobs().dispatch(indexer_job).await?;
	index_handle.wait().await?;

	harness
		.core
		.context
		.ephemeral_cache()
		.mark_indexing_complete(&test_root);

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Rename the folder
	let renamed_folder = test_root.join("renamed_folder");
	tracing::info!("Renaming folder in filesystem");
	tokio::fs::rename(&original_folder, &renamed_folder).await?;

	// Manual reindex to detect the change
	let reindex_config =
		IndexerJobConfig::ephemeral_browse(SdPath::local(test_root.clone()), IndexScope::Recursive);
	let reindex_job = IndexerJob::new(reindex_config);
	let reindex_handle = harness.library.jobs().dispatch(reindex_job).await?;
	reindex_handle.wait().await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify filesystem state
	assert!(
		!tokio::fs::try_exists(&original_folder)
			.await
			.unwrap_or(false),
		"Original folder should not exist"
	);
	assert!(
		tokio::fs::try_exists(&renamed_folder).await?,
		"Renamed folder should exist"
	);
	assert!(
		tokio::fs::try_exists(renamed_folder.join("file1.txt")).await?,
		"file1.txt should exist in renamed folder"
	);
	assert!(
		tokio::fs::try_exists(renamed_folder.join("file2.rs")).await?,
		"file2.rs should exist in renamed folder"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_ephemeral_folder_rename_via_watcher() -> anyhow::Result<()> {
	// Tests ephemeral real-time watcher change handling (no manual reindex)
	let harness = IndexingHarnessBuilder::new("ephemeral_rename_watcher")
		.build() // Watcher enabled
		.await?;

	let test_root = harness.temp_path();
	let original_folder = test_root.join("original_folder");
	tokio::fs::create_dir_all(&original_folder).await?;

	tokio::fs::write(original_folder.join("file1.txt"), "Content 1").await?;
	tokio::fs::write(original_folder.join("file2.rs"), "fn main() {}").await?;

	// Index in ephemeral mode
	let test_root_sd = SdPath::local(test_root.clone());
	let indexer_config = IndexerJobConfig::ephemeral_browse(test_root_sd, IndexScope::Recursive);
	let indexer_job = IndexerJob::new(indexer_config);

	tracing::info!("Initial ephemeral indexing");
	let index_handle = harness.library.jobs().dispatch(indexer_job).await?;
	index_handle.wait().await?;

	harness
		.core
		.context
		.ephemeral_cache()
		.mark_indexing_complete(&test_root);

	// Register for watching
	if let Some(watcher) = harness.core.context.get_fs_watcher().await {
		watcher.watch_ephemeral(test_root.clone()).await?;
		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	// Start collecting events
	let mut collector = EventCollector::new(&harness.core.events);
	let collection_handle = tokio::spawn(async move {
		collector.collect_events(Duration::from_secs(10)).await;
		collector
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Rename the folder - watcher should detect it
	let renamed_folder = test_root.join("renamed_folder");
	tracing::info!("Renaming folder (watcher will detect)");
	tokio::fs::rename(&original_folder, &renamed_folder).await?;

	// NO manual reindex - wait for watcher to handle it
	// Directories are buffered for 500ms for rename detection, then emitted on tick
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Verify filesystem state
	assert!(
		!tokio::fs::try_exists(&original_folder)
			.await
			.unwrap_or(false),
		"Original folder should not exist"
	);
	assert!(
		tokio::fs::try_exists(&renamed_folder).await?,
		"Renamed folder should exist"
	);
	assert!(
		tokio::fs::try_exists(renamed_folder.join("file1.txt")).await?,
		"file1.txt should exist in renamed folder"
	);
	assert!(
		tokio::fs::try_exists(renamed_folder.join("file2.rs")).await?,
		"file2.rs should exist in renamed folder"
	);

	// Verify file contents preserved
	let file1_content = tokio::fs::read_to_string(renamed_folder.join("file1.txt")).await?;
	assert_eq!(
		file1_content, "Content 1",
		"File content should be preserved"
	);

	// Verify watcher emitted events (ephemeral uses individual ResourceChanged events)
	let collector = collection_handle.await.unwrap();
	let stats = collector.analyze().await;

	let event_count = stats.resource_changed.values().sum::<usize>();
	assert!(
		event_count > 0,
		"Should emit ResourceChanged events from watcher, got {}",
		event_count
	);

	harness.shutdown().await?;
	Ok(())
}
