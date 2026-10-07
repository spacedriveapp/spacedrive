//! Demonstration of job pause/resume functionality

use sd_core::{
	infra::{
		action::LibraryAction,
		job::types::{JobId, JobStatus},
	},
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	// Initialize logging
	tracing_subscriber::fmt::init();

	println!("=== Job Pause/Resume Demo ===\n");

	// Setup test environment
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Create library
	println!("1. Creating library...");
	let library = core
		.libraries
		.create_library("Demo Library", None, core.context.clone())
		.await?;

	// Create a test source with files
	let test_source = temp_dir.path().join("test_source");
	tokio::fs::create_dir_all(&test_source).await?;

	println!("2. Creating test files...");
	for i in 0..50 {
		let file_path = test_source.join(format!("test_file_{}.txt", i));
		tokio::fs::write(&file_path, format!("Test content {}", i)).await?;
	}

	// Tracking the directory dispatches its walk
	println!("3. Tracking the source and starting its walk...");
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: test_source.clone(),
		name: Some("Demo Source".to_string()),
		overrides: Default::default(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?;

	let job_manager = library.jobs();
	let Some(job_id) = tracked.job_id.map(JobId) else {
		println!("Tracking dispatched no walk!");
		return Ok(());
	};
	// Held from the start, so the output survives the job leaving the running set
	let Some(walk) = job_manager.get_job(job_id).await else {
		println!("The walk finished before it could be paused");
		return Ok(());
	};
	println!("   Found walk job: {}", job_id.0);

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
				println!("\nJob completed successfully!");
				break;
			}
			JobStatus::Failed => {
				println!("\nJob failed: {:?}", job_info.error_message);
				break;
			}
			_ => continue,
		}
	}

	// Check results
	let indexed = walk.wait().await?.as_indexed();

	println!("\n9. Results:");
	println!(
		"   Files indexed: {}",
		indexed.map_or(0, |output| output.total_files)
	);
	println!("   Expected: 50");

	println!("\nDemo completed successfully!");

	Ok(())
}
