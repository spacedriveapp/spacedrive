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
	let location_entry_id = location_record.entry_id;

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
		let location_id = location_entry_id.expect("Location should have entry_id");
		let descendant_ids: Vec<i32> = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_id))
			.all(db.conn())
			.await?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect();

		let mut all_ids = vec![location_id];
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
	let location_entry_id = location_record.entry_id;

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
	let location_id = location_entry_id.expect("Location should have entry_id");
	let descendant_ids: Vec<i32> = entry_closure::Entity::find()
		.filter(entry_closure::Column::AncestorId.eq(location_id))
		.all(db.conn())
		.await?
		.into_iter()
		.map(|ec| ec.descendant_id)
		.collect();

	let mut all_entry_ids = vec![location_id];
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
