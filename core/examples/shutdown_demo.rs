//! Demonstration of jobs being paused during shutdown

use sd_core::{infra::job::types::JobStatus, Core};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	// Initialize logging
	tracing_subscriber::fmt::init();

	println!("=== Job Shutdown Demo ===\n");

	// Create Core instance
	let data_dir = std::env::temp_dir().join("spacedrive-shutdown-demo");
	let core = Core::new(data_dir).await?;

	// Get open libraries
	let libraries = core.libraries.get_open_libraries().await;
	if libraries.is_empty() {
		println!("No open libraries found.");
		println!("\nTo test shutdown behavior:");
		println!("1. Create a library: spacedrive library create \"Test Library\"");
		println!("2. Start an indexing job: spacedrive location add /path/to/large/folder");
		println!("3. Run this demo while indexing is in progress");
		return Ok(());
	}

	// Check for running jobs across all libraries
	let mut total_running = 0;
	for library in &libraries {
		let job_manager = library.jobs();
		let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;

		if !running_jobs.is_empty() {
			println!(
				"Library {} has {} running jobs:",
				library.id(),
				running_jobs.len()
			);
			for job in &running_jobs {
				println!(
					"  - {} ({}): {:.1}% complete",
					job.name, job.id, job.progress
				);
			}
			total_running += running_jobs.len();
		}
	}

	if total_running == 0 {
		println!("No running jobs found. Start some jobs first to test shutdown behavior.");
		return Ok(());
	}

	println!("\n{} total running jobs found.", total_running);
	println!("\nShutting down in 3 seconds...");
	println!("All running jobs will be paused and can be resumed later.");

	for i in (1..=3).rev() {
		println!("{}...", i);
		sleep(Duration::from_secs(1)).await;
	}

	println!("\nInitiating shutdown...");
	let start = std::time::Instant::now();

	// Shutdown the core - this will pause all running jobs
	core.shutdown().await?;

	let elapsed = start.elapsed();
	println!(
		"\n✓ Shutdown completed in {:.2} seconds",
		elapsed.as_secs_f32()
	);
	println!("✓ All running jobs have been paused and their state saved");
	println!("\nThese jobs will automatically resume when Spacedrive restarts.");

	Ok(())
}
