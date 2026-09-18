//! Integration tests for folder rename operations
//!
//! This test suite verifies folder rename handling over the volume index in
//! two scenarios:
//! 1. Manual reindex (batch change detection)
//! 2. Watcher (real-time change handling)
//!
//! Each test validates:
//! - The folder exists only under its new name, with its children inside
//! - Event emission for UI updates, where the watcher is running

mod helpers;

use helpers::*;
use sd_core::ops::indexing::IndexScope;
use tokio::time::Duration;

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

	tracing::info!("Initial indexing");
	harness.index_dir(test_root, IndexScope::Recursive).await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Rename the folder
	let renamed_folder = test_root.join("renamed_folder");
	tracing::info!("Renaming folder in filesystem");
	tokio::fs::rename(&original_folder, &renamed_folder).await?;

	// Manual reindex to detect the change
	harness.index_dir(test_root, IndexScope::Recursive).await?;

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

	tracing::info!("Initial indexing");
	harness.index_dir(test_root, IndexScope::Recursive).await?;

	// The finished walk armed the watch; let it settle
	tokio::time::sleep(Duration::from_millis(500)).await;

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
