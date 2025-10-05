//! Common utilities and structures for benchmark scenarios
use anyhow::{anyhow, Result};
use sd_core::infra::event::{Event, EventSubscriber};
use sd_core::infra::job::output::JobOutput;
use sd_core::library::Library;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Base state for scenarios that run and monitor jobs.
#[derive(Default)]
pub struct ScenarioBase {
	pub job_ids: Vec<Uuid>,
	pub library: Option<Arc<Library>>,
	pub hardware_hint: Option<String>,
}

/// Waits for jobs to complete and collects their output via the event bus.
pub async fn run_jobs_and_collect_outputs(
	job_ids: &[Uuid],
	mut event_subscriber: EventSubscriber,
) -> Result<HashMap<Uuid, JobOutput>> {
	let mut outputs = HashMap::new();
	let job_id_set: HashSet<Uuid> = job_ids.iter().cloned().collect();
	let mut completed_jobs = HashSet::new();

	println!("Waiting for {} job(s) to complete...", job_ids.len());

	let timeout = Duration::from_secs(30 * 60); // 30 minute timeout
	let start = std::time::Instant::now();

	while completed_jobs.len() < job_ids.len() {
		if start.elapsed() > timeout {
			return Err(anyhow!("Benchmark timed out while waiting for jobs"));
		}

		match tokio::time::timeout(timeout, event_subscriber.recv()).await {
			Ok(Ok(Event::JobCompleted { job_id, output, .. })) => {
				if let Ok(id) = Uuid::parse_str(&job_id) {
					if job_id_set.contains(&id) && !completed_jobs.contains(&id) {
						outputs.insert(id, output);
						completed_jobs.insert(id);
						println!(
							"Job {} completed. ({}/{})",
							id,
							completed_jobs.len(),
							job_ids.len()
						);
					}
				}
			}
			Ok(Ok(Event::JobFailed { job_id, error, .. })) => {
				if let Ok(id) = Uuid::parse_str(&job_id) {
					if job_id_set.contains(&id) {
						return Err(anyhow!("Job {} failed: {}", id, error));
					}
				}
			}
			Ok(Err(_)) => { /* Channel lagged, just continue */ }
			Err(_) => return Err(anyhow!("Timeout waiting for job completion event")),
			_ => { /* Ignore other events */ }
		}
	}

	println!("All jobs completed successfully.");
	Ok(outputs)
}
