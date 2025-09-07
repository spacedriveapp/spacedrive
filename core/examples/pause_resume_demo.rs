//! Demonstration of job pause/resume functionality

use sd_core::{
	infra::{
		db::entities,
		job::types::{JobId, JobStatus},
	},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ActiveModelTrait, EntityTrait};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize logging
	tracing_subscriber::fmt::init();

	println!("=== Job Pause/Resume Demo ===\n");

	// Setup test environment
	let temp_dir = TempDir::new()?;
	let core = Core::new_with_config(temp_dir.path().to_path_buf()).await?;

	// Create library
	println!("1. Creating library...");
	let library = core
		.libraries
		.create_library("Demo Library", None, core.context.clone())
		.await?;

	// Create test location with files
	let test_location = temp_dir.path().join("test_location");
	tokio::fs::create_dir_all(&test_location).await?;

	println!("2. Creating test files...");
	for i in 0..50 {
		let file_path = test_location.join(format!("test_file_{}.txt", i));
		tokio::fs::write(&file_path, format!("Test content {}", i)).await?;
	}

	// Register device
	let db = library.db();
	let device = core.device.to_device()?;
	let device_model: entities::device::ActiveModel = device.into();
	let device_record = device_model.insert(db.conn()).await?;

	// Create location to trigger indexing
	println!("3. Creating location and starting indexing job...");
	let location_args = LocationCreateArgs {
		path: test_location.clone(),
		name: Some("Demo Location".to_string()),
		index_mode: IndexMode::Deep,
	};

	create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Get the indexing job
	let job_manager = library.jobs();
	sleep(Duration::from_millis(200)).await;

	let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;
	if running_jobs.is_empty() {
		println!("No running jobs found!");
		return Ok(());
	}

	let job_info = &running_jobs[0];
	let job_id = JobId(job_info.id);
	println!("   Found indexing job: {} ({})", job_info.name, job_id.0);

	// Let it run for a bit
	println!("\n4. Letting job run for 1 second...");
	sleep(Duration::from_secs(1)).await;

	// Check progress
	let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
	println!("   Progress: {:.1}%", job_info.progress);

	// Pause the job
	println!("\n5. Pausing the job...");
	job_manager.pause_job(job_id).await?;
	sleep(Duration::from_millis(200)).await;

	let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
	println!("   Job status: {:?}", job_info.status);
	println!("   Progress when paused: {:.1}%", job_info.progress);

	// Wait while paused
	println!("\n6. Waiting 2 seconds while paused...");
	sleep(Duration::from_secs(2)).await;

	let job_info_after_wait = job_manager.get_job_info(job_id.0).await?.unwrap();
	println!(
		"   Progress after waiting: {:.1}% (should be same)",
		job_info_after_wait.progress
	);
	assert_eq!(
		job_info.progress, job_info_after_wait.progress,
		"Progress should not change while paused"
	);

	// Resume the job
	println!("\n7. Resuming the job...");
	job_manager.resume_job(job_id).await?;

	// Monitor until completion
	println!("\n8. Waiting for job to complete...");
	let mut last_progress = job_info_after_wait.progress;
	loop {
		sleep(Duration::from_millis(500)).await;
		let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();

		if job_info.progress != last_progress {
			println!("   Progress: {:.1}%", job_info.progress);
			last_progress = job_info.progress;
		}

		match job_info.status {
			JobStatus::Completed => {
				println!("\n✅ Job completed successfully!");
				break;
			}
			JobStatus::Failed => {
				println!("\n❌ Job failed: {:?}", job_info.error_message);
				break;
			}
			_ => continue,
		}
	}

	// Check results
	use sea_orm::PaginatorTrait;
	let indexed_count = entities::entry::Entity::find().count(db.conn()).await?;

	println!("\n9. Results:");
	println!("   Files indexed: {}", indexed_count);
	println!("   Expected: 50");

	println!("\n✨ Demo completed successfully!");

	Ok(())
}
