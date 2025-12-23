//! Integration tests for file copy operations with event bus monitoring
//!
//! This test verifies that copy actions properly emit resource change events
//! and work correctly with both ephemeral and persistent indexing.

mod helpers;

use helpers::*;
use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	location::IndexMode,
	ops::files::copy::{
		input::CopyMethod,
		job::{CopyOptions, FileCopyJob, MoveMode},
	},
};
use tempfile::TempDir;
use tokio::{fs, time::Duration};

/// Helper to create test files with content
async fn create_test_file(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).await?;
	}
	fs::write(path, content).await
}

#[tokio::test]
async fn test_copy_with_persistent_index() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("copy_persistent")
		.build()
		.await?;

	let test_location = harness.create_test_location("test_copy").await?;
	let source_dir = test_location.path().join("source");
	let dest_dir = test_location.path().join("destination");

	tokio::fs::create_dir_all(&source_dir).await?;
	tokio::fs::create_dir_all(&dest_dir).await?;

	test_location
		.write_file("source/file1.txt", "Content 1")
		.await?;
	test_location
		.write_file("source/file2.txt", "Content 2")
		.await?;

	let _location = test_location
		.index("Test Copy Location", IndexMode::Shallow)
		.await?;

	// Wait for initial library setup to complete
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Start collecting events AFTER setup, BEFORE copy
	let mut collector = EventCollector::new(&harness.core.events);
	let collection_handle = {
		tokio::spawn(async move {
			collector.collect_events(Duration::from_secs(5)).await;
			collector
		})
	};

	tokio::time::sleep(Duration::from_millis(100)).await;

	let copy_job = FileCopyJob::new(
		SdPathBatch::new(vec![
			SdPath::local(source_dir.join("file1.txt")),
			SdPath::local(source_dir.join("file2.txt")),
		]),
		SdPath::local(dest_dir.clone()),
	)
	.with_options(CopyOptions {
		conflict_resolution: None,
		overwrite: false,
		copy_method: CopyMethod::Auto,
		verify_checksum: false,
		preserve_timestamps: true,
		delete_after_copy: false,
		move_mode: None,
	});

	tracing::info!("Dispatching copy job");
	let handle = harness.library.jobs().dispatch(copy_job).await?;

	handle.wait().await?;

	// Wait for watcher to detect the copied files
	tokio::time::sleep(Duration::from_secs(3)).await;

	let collector = collection_handle.await.unwrap();
	let stats = collector.analyze().await;

	// Assert watcher detected the copied files via ResourceChangedBatch
	let file_batch_count = stats
		.resource_changed_batch
		.get("file")
		.copied()
		.unwrap_or(0);
	assert!(
		file_batch_count >= 2,
		"Expected at least 2 file resources in ResourceChangedBatch from watcher, got {}",
		file_batch_count
	);

	assert!(
		tokio::fs::try_exists(dest_dir.join("file1.txt"))
			.await
			.unwrap_or(false),
		"Destination file1.txt should exist"
	);
	assert!(
		tokio::fs::try_exists(dest_dir.join("file2.txt"))
			.await
			.unwrap_or(false),
		"Destination file2.txt should exist"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_copy_with_ephemeral_index() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("copy_ephemeral")
		.build()
		.await?;

	// Create test directories in the harness test root
	let test_root = harness.temp_path();
	let source_dir = test_root.join("source");
	let dest_dir = test_root.join("destination");
	tokio::fs::create_dir_all(&source_dir).await?;
	tokio::fs::create_dir_all(&dest_dir).await?;

	create_test_file(&source_dir.join("file1.txt"), "Ephemeral content 1").await?;
	create_test_file(&source_dir.join("file2.txt"), "Ephemeral content 2").await?;

	// Index the destination directory in ephemeral mode first
	use sd_core::{
		domain::addressing::SdPath,
		ops::indexing::{IndexScope, IndexerJob, IndexerJobConfig},
	};

	let dest_sd_path = SdPath::local(dest_dir.clone());
	let indexer_config = IndexerJobConfig::ephemeral_browse(dest_sd_path, IndexScope::Current);
	let indexer_job = IndexerJob::new(indexer_config);

	tracing::info!("Indexing destination directory (ephemeral)");
	let index_handle = harness.library.jobs().dispatch(indexer_job).await?;
	index_handle.wait().await?;

	// Mark indexing complete and register for watching
	harness
		.core
		.context
		.ephemeral_cache()
		.mark_indexing_complete(&dest_dir);

	if let Some(watcher) = harness.core.context.get_fs_watcher().await {
		watcher.watch_ephemeral(dest_dir.clone()).await?;
		// Give the watcher time to settle
		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	// Wait for initial library setup to complete
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Start collecting events AFTER setup, BEFORE copy
	let mut collector = EventCollector::new(&harness.core.events);
	let collection_handle = {
		tokio::spawn(async move {
			collector.collect_events(Duration::from_secs(5)).await;
			collector
		})
	};

	tokio::time::sleep(Duration::from_millis(100)).await;

	let copy_job = FileCopyJob::new(
		SdPathBatch::new(vec![
			SdPath::local(source_dir.join("file1.txt")),
			SdPath::local(source_dir.join("file2.txt")),
		]),
		SdPath::local(dest_dir.clone()),
	)
	.with_options(CopyOptions {
		conflict_resolution: None,
		overwrite: false,
		copy_method: CopyMethod::Auto,
		verify_checksum: false,
		preserve_timestamps: true,
		delete_after_copy: false,
		move_mode: None,
	});

	tracing::info!("Dispatching ephemeral copy job");
	let handle = harness.library.jobs().dispatch(copy_job).await?;

	handle.wait().await?;

	// Give watcher time to process copy events
	tokio::time::sleep(Duration::from_secs(3)).await;

	let collector = collection_handle.await.unwrap();
	let stats = collector.analyze().await;

	// Assert watcher detected the copied files (2 files, each triggers CREATE + MODIFY = 4 events)
	let file_events = stats.resource_changed.get("file").copied().unwrap_or(0);
	assert!(
		file_events >= 2,
		"Expected at least 2 file ResourceChanged events from watcher, got {}",
		file_events
	);

	assert!(
		tokio::fs::try_exists(dest_dir.join("file1.txt"))
			.await
			.unwrap_or(false),
		"Destination file1.txt should exist"
	);
	assert!(
		tokio::fs::try_exists(dest_dir.join("file2.txt"))
			.await
			.unwrap_or(false),
		"Destination file2.txt should exist"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_copy_action_construction() {
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	let source_dir = test_root.join("source");
	let dest_dir = test_root.join("destination");
	fs::create_dir_all(&source_dir).await.unwrap();
	fs::create_dir_all(&dest_dir).await.unwrap();

	let source_file1 = source_dir.join("test1.txt");
	let source_file2 = source_dir.join("test2.txt");

	create_test_file(&source_file1, "Hello, World! This is test file 1.")
		.await
		.unwrap();
	create_test_file(&source_file2, "This is the content of test file 2.")
		.await
		.unwrap();

	let copy_job = FileCopyJob::new(
		SdPathBatch::new(vec![
			SdPath::local(source_file1.clone()),
			SdPath::local(source_file2.clone()),
		]),
		SdPath::local(dest_dir.clone()),
	)
	.with_options(CopyOptions {
		conflict_resolution: None,
		overwrite: false,
		copy_method: CopyMethod::Auto,
		verify_checksum: true,
		preserve_timestamps: true,
		delete_after_copy: false,
		move_mode: None,
	});

	assert_eq!(copy_job.sources.paths.len(), 2);
	assert_eq!(copy_job.options.overwrite, false);
	assert_eq!(copy_job.options.verify_checksum, true);
	assert_eq!(copy_job.options.preserve_timestamps, true);

	println!("Copy job construction test passed!");
}

#[tokio::test]
async fn test_move_action_construction() {
	let temp_dir = TempDir::new().unwrap();
	let source_file = temp_dir.path().join("source.txt");
	let dest_file = temp_dir.path().join("dest.txt");

	create_test_file(&source_file, "Move me!").await.unwrap();

	let move_job = FileCopyJob::new_move(
		SdPathBatch::new(vec![SdPath::local(source_file.clone())]),
		SdPath::local(dest_file.clone()),
		MoveMode::Move,
	);

	assert!(move_job.options.delete_after_copy);
	assert_eq!(move_job.options.move_mode, Some(MoveMode::Move));

	println!("Move job construction test passed!");
}

// Note: This test was removed because FileCopyJob doesn't use a builder pattern
// Jobs are constructed directly, and validation happens during execution
// #[tokio::test]
// async fn test_action_validation_logic() {
// 	let result = sd_core::ops::files::copy::action::FileCopyAction::builder()
// 		.destination("/tmp/dest")
// 		.build();
// 	assert!(result.is_err());
//
// 	println!("Action validation (builder) test passed!");
// }

#[test]
fn test_copy_options_defaults() {
	let options = CopyOptions::default();

	assert!(!options.overwrite);
	assert!(!options.verify_checksum);
	assert!(options.preserve_timestamps);
	assert!(!options.delete_after_copy);
	assert!(options.move_mode.is_none());

	println!("Copy options defaults test passed!");
}
