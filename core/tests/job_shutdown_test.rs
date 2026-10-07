//! Test for job pausing during shutdown

use sd_core::{
	infra::action::LibraryAction,
	infra::event::Event,
	infra::job::types::{JobId, JobStatus},
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_jobs_paused_on_shutdown() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	// Setup test environment
	let temp_dir = TempDir::new()?;
	let core_dir = temp_dir.path().join("core");
	tokio::fs::create_dir_all(&core_dir).await?;

	let core = Core::new(core_dir).await?;

	// Create library
	let library = core
		.libraries
		.create_library("Test Shutdown Library", None, core.context.clone())
		.await?;

	// A source with many files, so its walk is still running at shutdown
	let source_dir = temp_dir.path().join("test_source");
	for dir in 0..100 {
		let subdir = source_dir.join(format!("subdir_{}", dir));
		tokio::fs::create_dir_all(&subdir).await?;
		for file in 0..20 {
			let file_path = subdir.join(format!("test_file_{}.txt", file));
			tokio::fs::write(&file_path, format!("Test content {} {}", dir, file)).await?;
		}
	}

	// Tracking the directory dispatches its walk
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: source_dir.clone(),
		name: Some("Test Source".to_string()),
		overrides: Default::default(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let walk_id = JobId(tracked.job_id.ok_or("tracking dispatched no walk")?);

	// Shut down the moment the walk is running, with most of the tree to go.
	// Shutdown pauses running jobs only, so a walk still queued proves nothing
	let job_manager = library.jobs();
	let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
	loop {
		let running = job_manager.list_jobs(Some(JobStatus::Running)).await?;
		if running.iter().any(|job| job.id == walk_id.0) {
			break;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"The walk never started running"
		);
		sleep(Duration::from_millis(5)).await;
	}

	// The job database closes with the core, so the pause is read off the bus
	let mut events = core.events.subscribe();
	println!("Shutting down core...");
	core.shutdown().await?;

	let walk_id = walk_id.0.to_string();
	let mut paused = false;
	while let Ok(event) = events.try_recv() {
		if matches!(&event, Event::JobPaused { job_id, .. } if *job_id == walk_id) {
			paused = true;
		}
	}
	assert!(paused, "The walk should be paused during shutdown");
	println!("✓ Walk {} was paused during shutdown", walk_id);

	Ok(())
}

#[tokio::test]
async fn test_shutdown_with_no_running_jobs() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
	// This test ensures shutdown works correctly when no jobs are running
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

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
