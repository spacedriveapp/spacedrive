//! Test for job pausing during shutdown

use sd_core::{
	infra::db::entities,
	infra::job::types::{JobId, JobStatus},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::ActiveModelTrait;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_jobs_paused_on_shutdown() -> Result<(), Box<dyn std::error::Error>> {
	// Setup test environment
	let temp_dir = TempDir::new()?;
	let core_dir = temp_dir.path().join("core");
	tokio::fs::create_dir_all(&core_dir).await?;

	let core = Core::new_with_config(core_dir).await?;

	// Create library
	let library = core
		.libraries
		.create_library("Test Shutdown Library", None, core.context.clone())
		.await?;

	// Create test location with many files to ensure job runs long enough
	let test_location_dir = temp_dir.path().join("test_location");
	tokio::fs::create_dir_all(&test_location_dir).await?;

	// Create enough files to ensure indexing takes some time
	for i in 0..200 {
		let file_path = test_location_dir.join(format!("test_file_{}.txt", i));
		tokio::fs::write(&file_path, format!("Test content {}", i)).await?;

		// Create some subdirectories with files
		if i % 20 == 0 {
			let subdir = test_location_dir.join(format!("subdir_{}", i));
			tokio::fs::create_dir_all(&subdir).await?;
			for j in 0..10 {
				let subfile = subdir.join(format!("subfile_{}.txt", j));
				tokio::fs::write(&subfile, format!("Subcontent {} {}", i, j)).await?;
			}
		}
	}

	// Register device
	let db = library.db();
	let device = core.device.to_device()?;
	let device_model: entities::device::ActiveModel = device.into();
	let device_record = device_model.insert(db.conn()).await?;

	// Create location to trigger indexing
	let location_args = LocationCreateArgs {
		path: test_location_dir.clone(),
		name: Some("Test Location".to_string()),
		index_mode: IndexMode::Deep,
	};

	create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Wait for indexing to start
	sleep(Duration::from_millis(500)).await;

	// Verify we have running jobs
	let job_manager = library.jobs();
	let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;
	assert!(
		!running_jobs.is_empty(),
		"Should have at least one running job"
	);

	let job_ids: Vec<JobId> = running_jobs.iter().map(|j| JobId(j.id)).collect();
	println!("Found {} running jobs before shutdown", job_ids.len());

	// Shutdown the core, which should pause all jobs
	println!("Shutting down core...");
	core.shutdown().await?;

	// Check that jobs were paused
	for job_id in &job_ids {
		let job_info = job_manager.get_job_info(job_id.0).await?;
		if let Some(info) = job_info {
			assert_eq!(
				info.status,
				JobStatus::Paused,
				"Job {} should be paused after shutdown",
				job_id.0
			);
			println!("✓ Job {} was paused during shutdown", job_id.0);
		}
	}

	Ok(())
}

#[tokio::test]
async fn test_shutdown_with_no_running_jobs() -> Result<(), Box<dyn std::error::Error>> {
	// This test ensures shutdown works correctly when no jobs are running
	let temp_dir = TempDir::new()?;
	let core = Core::new_with_config(temp_dir.path().to_path_buf()).await?;

	let library = core
		.libraries
		.create_library("Empty Library", None, core.context.clone())
		.await?;

	// Verify no running jobs
	let job_manager = library.jobs();
	let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;
	assert!(running_jobs.is_empty());

	// Shutdown should complete without errors
	core.shutdown().await?;
	println!("✓ Shutdown completed successfully with no running jobs");

	Ok(())
}
