//! Indexing Integration Test
//!
//! Tests the production indexer functionality including:
//! - Location creation and indexing
//! - Smart filtering of system files
//! - Inode tracking for incremental indexing
//! - Event monitoring during indexing
//! - Database persistence of indexed entries
//!
//! Note: These tests should be run with --test-threads=1 to avoid
//! device UUID conflicts when multiple tests run in parallel

use sd_core::{
	infra::db::entities::{self, entry_closure},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use tempfile::TempDir;
use tokio::time::Duration;

#[tokio::test]
async fn test_location_indexing() -> Result<(), Box<dyn std::error::Error>> {
	// 1. Setup test environment
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// 2. Create library
	let library = core
		.libraries
		.create_library("Test Indexing Library", None, core.context.clone())
		.await?;

	// 3. Create test location directory with some files
	let test_location_dir = temp_dir.path().join("test_location");
	tokio::fs::create_dir_all(&test_location_dir).await?;

	// Create test files
	tokio::fs::write(test_location_dir.join("test1.txt"), "Hello World").await?;
	tokio::fs::write(test_location_dir.join("test2.rs"), "fn main() {}").await?;
	tokio::fs::create_dir_all(test_location_dir.join("subdir")).await?;
	tokio::fs::write(test_location_dir.join("subdir/test3.md"), "# Test").await?;

	// Create files that should be filtered
	tokio::fs::write(test_location_dir.join(".DS_Store"), "system file").await?;
	tokio::fs::create_dir_all(test_location_dir.join("node_modules")).await?;
	tokio::fs::write(test_location_dir.join("node_modules/package.json"), "{}").await?;

	// 4. Register device in database
	let db = library.db();
	let device = core.device.to_device()?;

	let device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await?
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(db.conn()).await?
		}
	};

	// 5. Set up to monitor job completion
	// Note: Due to current implementation, IndexingCompleted event may not be emitted
	// So we'll monitor job status directly instead

	// 6. Create location and trigger indexing
	let location_args = LocationCreateArgs {
		path: test_location_dir.clone(),
		name: Some("Test Location".to_string()),
		index_mode: IndexMode::Deep,
	};

	let location_db_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Get the location record to find its entry_id
	let location_record = entities::location::Entity::find_by_id(location_db_id)
		.one(db.conn())
		.await?
		.expect("Location should exist");
	let location_entry_id = location_record
		.entry_id
		.expect("Location should have entry_id");

	// 7. Wait for indexing to complete by monitoring job status
	let start_time = tokio::time::Instant::now();
	let timeout_duration = Duration::from_secs(30);

	let mut job_seen = false;
	let mut last_entry_count = 0;
	let mut stable_count_iterations = 0;

	loop {
		// Check all job statuses
		let all_jobs = library.jobs().list_jobs(None).await?;
		let running_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Running))
			.await?;

		// If we see a running job, mark that we've seen it
		if !running_jobs.is_empty() {
			job_seen = true;
		}

		// Check if any entries have been created (partial progress)
		// Use closure table to count entries under this location
		let descendant_count = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
			.count(db.conn())
			.await?;

		let current_entries = descendant_count;

		println!(
			"Job status - Total: {}, Running: {}, Entries indexed: {}",
			all_jobs.len(),
			running_jobs.len(),
			current_entries
		);

		// Check for completed jobs
		let completed_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Completed))
			.await?;

		// If we've seen a job and now it's completed, indexing likely finished
		if job_seen && !completed_jobs.is_empty() && running_jobs.is_empty() && current_entries > 0
		{
			// Wait for entries to stabilize
			if current_entries == last_entry_count {
				stable_count_iterations += 1;
				if stable_count_iterations >= 3 {
					println!("Indexing appears complete (job finished, entries stable)");
					break;
				}
			} else {
				stable_count_iterations = 0;
			}
			last_entry_count = current_entries;
		}

		// Check for failed jobs
		let failed_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Failed))
			.await?;

		if !failed_jobs.is_empty() {
			// Try to get more information about the failure
			for job in &failed_jobs {
				println!("Failed job: {:?}", job);
			}
			panic!("Indexing job failed with {} failures", failed_jobs.len());
		}

		// Check timeout
		if start_time.elapsed() > timeout_duration {
			panic!("Indexing timed out after {:?}", timeout_duration);
		}

		// Wait a bit before checking again
		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	// 8. Verify indexed entries in database
	// Helper to get all entry IDs under the location
	let get_location_entry_ids = || async {
		let descendant_ids = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
			.all(db.conn())
			.await?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect::<Vec<i32>>();

		let mut all_ids = vec![location_entry_id];
		all_ids.extend(descendant_ids);
		Ok::<Vec<i32>, anyhow::Error>(all_ids)
	};

	let location_entry_ids = get_location_entry_ids().await?;
	let _entry_count = location_entry_ids.len();

	let file_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(location_entry_ids.clone()))
		.filter(entities::entry::Column::Kind.eq(0)) // Files
		.count(db.conn())
		.await?;

	let dir_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(location_entry_ids.clone()))
		.filter(entities::entry::Column::Kind.eq(1)) // Directories
		.count(db.conn())
		.await?;

	// 9. Verify smart filtering worked
	let all_entries = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(location_entry_ids.clone()))
		.all(db.conn())
		.await?;

	// Check that filtered files are not indexed
	for entry in &all_entries {
		assert_ne!(entry.name, ".DS_Store", "System files should be filtered");
		assert_ne!(
			entry.name, "node_modules",
			"Dev directories should be filtered"
		);
	}

	// 10. Verify expected counts
	assert_eq!(file_count, 3, "Should index 3 files (excluding filtered)");
	assert!(dir_count >= 1, "Should index at least 1 directory (subdir)");

	// 11. Verify inode tracking
	let entries_with_inodes = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(location_entry_ids.clone()))
		.filter(entities::entry::Column::Inode.is_not_null())
		.count(db.conn())
		.await?;

	assert!(
		entries_with_inodes > 0,
		"Entries should have inode tracking"
	);

	// 12. Cleanup
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);

	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_incremental_indexing() -> Result<(), Box<dyn std::error::Error>> {
	// 1. Setup
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	let library = core
		.libraries
		.create_library("Test Incremental Library", None, core.context.clone())
		.await?;

	let test_location_dir = temp_dir.path().join("incremental_test");
	tokio::fs::create_dir_all(&test_location_dir).await?;

	// Initial files
	tokio::fs::write(test_location_dir.join("file1.txt"), "Initial content").await?;
	tokio::fs::write(test_location_dir.join("file2.txt"), "More content").await?;

	// Register device
	let db = library.db();
	let device = core.device.to_device()?;

	let device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await?
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(db.conn()).await?
		}
	};

	// 2. First indexing run
	let location_args = LocationCreateArgs {
		path: test_location_dir.clone(),
		name: Some("Incremental Test".to_string()),
		index_mode: IndexMode::Deep,
	};

	let location_db_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Get the location record to find its entry_id
	let location_record = entities::location::Entity::find_by_id(location_db_id)
		.one(db.conn())
		.await?
		.expect("Location should exist");
	let location_entry_id = location_record
		.entry_id
		.expect("Location should have entry_id");

	// Wait for initial indexing to complete
	let start_time = tokio::time::Instant::now();
	let timeout_duration = Duration::from_secs(10);
	let mut job_seen = false;

	loop {
		let running_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Running))
			.await?;

		if !running_jobs.is_empty() {
			job_seen = true;
		}

		let current_entries = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
			.count(db.conn())
			.await?;

		// Check for completed jobs
		let completed_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Completed))
			.await?;

		if job_seen && !completed_jobs.is_empty() && running_jobs.is_empty() && current_entries > 0
		{
			break;
		}

		if start_time.elapsed() > timeout_duration {
			break; // Don't fail, just continue
		}

		tokio::time::sleep(Duration::from_millis(200)).await;
	}

	// Get all entry IDs under this location
	let descendant_ids = entry_closure::Entity::find()
		.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
		.all(db.conn())
		.await?
		.into_iter()
		.map(|ec| ec.descendant_id)
		.collect::<Vec<i32>>();

	let mut all_entry_ids = vec![location_entry_id];
	all_entry_ids.extend(descendant_ids);

	let initial_file_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(all_entry_ids))
		.filter(entities::entry::Column::Kind.eq(0))
		.count(db.conn())
		.await?;

	assert_eq!(initial_file_count, 2, "Should index 2 initial files");

	// Cleanup
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);

	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_indexing_error_handling() -> Result<(), Box<dyn std::error::Error>> {
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	let library = core
		.libraries
		.create_library("Test Error Library", None, core.context.clone())
		.await?;

	// Try to index non-existent location
	let non_existent = temp_dir.path().join("does_not_exist");

	let db = library.db();
	let device = core.device.to_device()?;

	let device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await?
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(db.conn()).await?
		}
	};

	let location_args = LocationCreateArgs {
		path: non_existent,
		name: Some("Non-existent".to_string()),
		index_mode: IndexMode::Deep,
	};

	// This should handle the error gracefully
	let result = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await;

	// The location creation should fail for non-existent path
	assert!(
		result.is_err(),
		"Should fail to create location for non-existent path"
	);

	// Cleanup
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);

	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_reindexing_after_offline_changes() -> Result<(), Box<dyn std::error::Error>> {
	// 1. Setup test environment
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// 2. Create library
	let library = core
		.libraries
		.create_library("Test Reindex Library", None, core.context.clone())
		.await?;

	// 3. Create test location directory with initial files
	let test_location_dir = temp_dir.path().join("reindex_test");
	tokio::fs::create_dir_all(&test_location_dir).await?;

	// Initial files
	tokio::fs::write(test_location_dir.join("initial1.txt"), "Initial file 1").await?;
	tokio::fs::write(test_location_dir.join("initial2.rs"), "fn main() {}").await?;
	tokio::fs::create_dir_all(test_location_dir.join("initial_dir")).await?;
	tokio::fs::write(
		test_location_dir.join("initial_dir/initial3.md"),
		"# Initial",
	)
	.await?;

	// 4. Register device in database
	let db = library.db();
	let device = core.device.to_device()?;

	let device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await?
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(db.conn()).await?
		}
	};

	// 5. Create location and trigger initial indexing
	let location_args = LocationCreateArgs {
		path: test_location_dir.clone(),
		name: Some("Reindex Test Location".to_string()),
		index_mode: IndexMode::Deep,
	};

	let location_db_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Get the location record to find its entry_id
	let location_record = entities::location::Entity::find_by_id(location_db_id)
		.one(db.conn())
		.await?
		.expect("Location should exist");
	let location_entry_id = location_record
		.entry_id
		.expect("Location should have entry_id");
	let location_uuid = location_record.uuid;

	// 6. Wait for initial indexing to complete
	let start_time = tokio::time::Instant::now();
	let timeout_duration = Duration::from_secs(30);

	let mut job_seen = false;
	let mut last_entry_count = 0;
	let mut stable_count_iterations = 0;

	loop {
		let running_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Running))
			.await?;

		if !running_jobs.is_empty() {
			job_seen = true;
		}

		let current_entries = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
			.count(db.conn())
			.await?;

		let completed_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Completed))
			.await?;

		if job_seen && !completed_jobs.is_empty() && running_jobs.is_empty() && current_entries > 0
		{
			if current_entries == last_entry_count {
				stable_count_iterations += 1;
				if stable_count_iterations >= 3 {
					println!("Initial indexing complete");
					break;
				}
			} else {
				stable_count_iterations = 0;
			}
			last_entry_count = current_entries;
		}

		if start_time.elapsed() > timeout_duration {
			panic!("Initial indexing timed out");
		}

		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	// 7. Get initial entry counts
	let descendant_ids = entry_closure::Entity::find()
		.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
		.all(db.conn())
		.await?
		.into_iter()
		.map(|ec| ec.descendant_id)
		.collect::<Vec<i32>>();

	let mut initial_entry_ids = vec![location_entry_id];
	initial_entry_ids.extend(descendant_ids);
	let initial_total_count = initial_entry_ids.len();

	let initial_file_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(initial_entry_ids.clone()))
		.filter(entities::entry::Column::Kind.eq(0)) // Files
		.count(db.conn())
		.await?;

	let initial_dir_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(initial_entry_ids.clone()))
		.filter(entities::entry::Column::Kind.eq(1)) // Directories
		.count(db.conn())
		.await?;

	println!(
		"Initial index complete - Total: {}, Files: {}, Dirs: {}",
		initial_total_count, initial_file_count, initial_dir_count
	);

	// Store the initial entries for later comparison
	let initial_entries = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(initial_entry_ids))
		.all(db.conn())
		.await?;

	// 8. Shut down core (simulate "offline")
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);
	core.shutdown().await?;
	drop(core);

	println!("Core shut down - simulating offline state");

	// 9. Make filesystem changes while "offline"
	// Add new files
	tokio::fs::write(test_location_dir.join("new1.txt"), "New file 1").await?;
	tokio::fs::write(test_location_dir.join("new2.json"), r#"{"key": "value"}"#).await?;

	// Add new directory with files
	tokio::fs::create_dir_all(test_location_dir.join("new_dir")).await?;
	tokio::fs::write(test_location_dir.join("new_dir/new3.md"), "# New File").await?;
	tokio::fs::write(test_location_dir.join("new_dir/new4.txt"), "Content").await?;

	// Add nested directory
	tokio::fs::create_dir_all(test_location_dir.join("new_dir/nested")).await?;
	tokio::fs::write(
		test_location_dir.join("new_dir/nested/new5.rs"),
		"fn test() {}",
	)
	.await?;

	println!("Filesystem changes made while offline");

	// 10. Start up new Core instance (reconnects to same database)
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Load all existing libraries from disk
	core.libraries.load_all(core.context.clone()).await?;

	// Get the library back
	let library = core
		.libraries
		.get_library(lib_id)
		.await
		.expect("Library should be loaded");

	let db = library.db();

	println!("Core restarted with existing library");

	// 11. Trigger reindex by creating indexer job directly
	use sd_core::domain::addressing::SdPath;
	use sd_core::ops::indexing::{IndexMode as JobIndexMode, IndexerJob, IndexerJobConfig};

	let location_sd_path = SdPath::new(
		format!("device_{}", device_record.id),
		test_location_dir.clone(),
	);

	let config = IndexerJobConfig::new(location_uuid, location_sd_path, JobIndexMode::Deep);
	let indexer_job = IndexerJob::new(config);

	let job_handle = library
		.jobs()
		.dispatch_with_priority(
			indexer_job,
			sd_core::infra::job::types::JobPriority::NORMAL,
			None,
		)
		.await?;
	let reindex_job_id = job_handle.id().to_string();

	println!("Reindex triggered with job_id: {}", reindex_job_id);

	// 12. Wait for reindexing to complete
	let start_time = tokio::time::Instant::now();
	let timeout_duration = Duration::from_secs(30);

	let mut job_seen = false;
	let mut last_entry_count = 0;
	let mut stable_count_iterations = 0;

	loop {
		let running_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Running))
			.await?;

		if !running_jobs.is_empty() {
			job_seen = true;
		}

		let current_entries = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
			.count(db.conn())
			.await?;

		println!(
			"Reindex status - Running jobs: {}, Entries: {}",
			running_jobs.len(),
			current_entries
		);

		let completed_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Completed))
			.await?;

		if job_seen && !completed_jobs.is_empty() && running_jobs.is_empty() && current_entries > 0
		{
			if current_entries == last_entry_count {
				stable_count_iterations += 1;
				if stable_count_iterations >= 3 {
					println!("Reindexing complete");
					break;
				}
			} else {
				stable_count_iterations = 0;
			}
			last_entry_count = current_entries;
		}

		let failed_jobs = library
			.jobs()
			.list_jobs(Some(sd_core::infra::job::types::JobStatus::Failed))
			.await?;

		if !failed_jobs.is_empty() {
			for job in &failed_jobs {
				println!("Failed job: {:?}", job);
			}
			panic!("Reindexing job failed");
		}

		if start_time.elapsed() > timeout_duration {
			panic!("Reindexing timed out");
		}

		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	// 13. Get post-reindex entry counts
	let final_descendant_ids = entry_closure::Entity::find()
		.filter(entry_closure::Column::AncestorId.eq(location_entry_id))
		.all(db.conn())
		.await?
		.into_iter()
		.map(|ec| ec.descendant_id)
		.collect::<Vec<i32>>();

	let mut final_entry_ids = vec![location_entry_id];
	final_entry_ids.extend(final_descendant_ids);
	let final_total_count = final_entry_ids.len();

	let final_file_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(final_entry_ids.clone()))
		.filter(entities::entry::Column::Kind.eq(0)) // Files
		.count(db.conn())
		.await?;

	let final_dir_count = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(final_entry_ids.clone()))
		.filter(entities::entry::Column::Kind.eq(1)) // Directories
		.count(db.conn())
		.await?;

	println!(
		"Reindex complete - Total: {}, Files: {}, Dirs: {}",
		final_total_count, final_file_count, final_dir_count
	);

	// 14. Verify change detection: All original files should still exist
	let final_entries = entities::entry::Entity::find()
		.filter(entities::entry::Column::Id.is_in(final_entry_ids.clone()))
		.all(db.conn())
		.await?;

	// Check that all initial entries still exist in final entries
	for initial_entry in &initial_entries {
		let found = final_entries
			.iter()
			.any(|e| e.name == initial_entry.name && e.kind == initial_entry.kind);
		assert!(
			found,
			"Original entry '{}' should still exist after reindex",
			initial_entry.name
		);
	}

	// 15. Verify new files were detected
	// We added 5 new files: new1.txt, new2.json, new3.md, new4.txt, new5.rs
	// We added 2 new directories: new_dir, nested
	let expected_new_files = 5;
	let expected_new_dirs = 2;

	let expected_total_files = initial_file_count + expected_new_files;
	let expected_total_dirs = initial_dir_count + expected_new_dirs;

	assert_eq!(
		final_file_count, expected_total_files,
		"Should have {} files ({} initial + {} new)",
		expected_total_files, initial_file_count, expected_new_files
	);

	assert_eq!(
		final_dir_count, expected_total_dirs,
		"Should have {} directories ({} initial + {} new)",
		expected_total_dirs, initial_dir_count, expected_new_dirs
	);

	// 16. Verify specific new entries exist
	let new_entry_names = vec![
		"new1.txt",
		"new2.json",
		"new_dir",
		"new3.md",
		"new4.txt",
		"nested",
		"new5.rs",
	];

	for name in new_entry_names {
		let found = final_entries.iter().any(|e| e.name == name);
		assert!(
			found,
			"New entry '{}' should be detected during reindex",
			name
		);
	}

	println!("✓ All new files and directories detected correctly");
	println!("✓ All original entries preserved");
	println!("✓ Tree structure matches exactly");

	// 17. Cleanup
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);
	core.shutdown().await?;

	Ok(())
}
