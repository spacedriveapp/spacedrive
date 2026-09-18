use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::common::{run_jobs_and_collect_outputs, ScenarioBase};
use super::{hardware_hint_to_label, infer_hardware_label, Scenario};
use crate::core_boot::CoreBoot;
use crate::metrics::{collect_host_info, BenchmarkRun, Durations, RunMeta};
use crate::recipe::Recipe;
use sd_core::infra::event::{Event, EventSubscriber};
use sd_core::infra::job::output::JobOutput;
use sd_core::infra::job::traits::Job;
use sd_core::ops::indexing::content_identity::{ContentIdentityJob, ContentIdentityOutput};

const CONTENT_JOB: &str = ContentIdentityJob::NAME;

#[derive(Default)]
pub struct ContentIdentificationScenario {
	base: ScenarioBase,
}

#[async_trait::async_trait]
impl Scenario for ContentIdentificationScenario {
	fn name(&self) -> &'static str {
		"content_identification"
	}

	fn describe(&self) -> &'static str {
		"Track a source and report the throughput of the content identification pass behind its walk"
	}

	async fn prepare(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<()> {
		self.base.track_recipe_sources(boot, recipe).await
	}

	async fn run(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<Vec<BenchmarkRun>> {
		// Both subscriptions open before either wait, so the passes queued
		// behind the walks cannot finish unobserved
		let walk_events = boot.core.events.subscribe();
		let pass_events = boot.core.events.subscribe();

		let walks = run_jobs_and_collect_outputs(&self.base.job_ids, walk_events).await?;
		let total_bytes: u64 = walks
			.values()
			.filter_map(|output| match output {
				JobOutput::Indexed { stats, .. } => Some(stats.bytes),
				_ => None,
			})
			.sum();

		let passes = collect_content_passes(pass_events, self.base.job_ids.len()).await?;
		let Some(first) = passes.first() else {
			return Ok(Vec::new());
		};
		let content_secs = passes
			.iter()
			.map(|pass| pass.finished)
			.max()
			.unwrap_or(first.finished)
			.duration_since(
				passes
					.iter()
					.map(|pass| pass.started)
					.min()
					.unwrap_or(first.started),
			)
			.as_secs_f64();
		let identified: u64 = passes.iter().map(|pass| pass.output.identified).sum();
		let unreadable: u64 = passes.iter().map(|pass| pass.output.unreadable).sum();

		let location_paths: Vec<PathBuf> =
			recipe.locations.iter().map(|l| l.path.clone()).collect();
		let meta = RunMeta {
			id: first.job_id,
			recipe_name: recipe.name.clone(),
			location_paths: location_paths.clone(),
			hardware_label: crate::metrics::derive_hardware_label_from_paths(&location_paths)
				.or_else(|| {
					self.base
						.hardware_hint
						.as_ref()
						.and_then(|h| hardware_hint_to_label(h))
				})
				.or_else(|| infer_hardware_label(&recipe.name)),
			timestamp_utc: Some(chrono::Utc::now().to_rfc3339()),
			host: collect_host_info(),
		};

		// A pass identifies files; directories carry no content to hash
		Ok(vec![BenchmarkRun::ContentIdentification {
			meta,
			files: identified,
			files_per_s: if content_secs > 0.0 {
				identified as f64 / content_secs
			} else {
				0.0
			},
			dirs: 0,
			dirs_per_s: 0.0,
			total_gb: total_bytes as f64 / 1_000_000_000.0,
			errors: unreadable,
			durations: Durations {
				discovery_s: None,
				processing_s: None,
				content_s: Some(content_secs),
				total_s: Some(content_secs),
			},
		}])
	}

	fn set_hardware_hint(&mut self, hint: Option<String>) {
		self.base.hardware_hint = hint;
	}
}

/// One content identification pass, timed by its own start and completion.
struct ContentPass {
	job_id: Uuid,
	started: Instant,
	finished: Instant,
	output: ContentIdentityOutput,
}

/// Wait for the content identification passes tracking queued behind the
/// walks, timing each from its start event to its completion.
async fn collect_content_passes(
	mut events: EventSubscriber,
	expected: usize,
) -> Result<Vec<ContentPass>> {
	let deadline = Instant::now() + Duration::from_secs(30 * 60);
	let mut started: HashMap<String, Instant> = HashMap::new();
	let mut passes = Vec::with_capacity(expected);

	while passes.len() < expected {
		let remaining = deadline
			.checked_duration_since(Instant::now())
			.ok_or_else(|| anyhow!("Timed out waiting for content identification"))?;
		match tokio::time::timeout(remaining, events.recv()).await {
			Ok(Ok(Event::JobStarted {
				job_id, job_type, ..
			})) if job_type == CONTENT_JOB => {
				started.insert(job_id, Instant::now());
			}
			Ok(Ok(Event::JobCompleted {
				job_id,
				job_type,
				output,
				..
			})) if job_type == CONTENT_JOB => {
				let finished = Instant::now();
				let Some(started) = started.remove(&job_id) else {
					continue;
				};
				let JobOutput::Custom(value) = output else {
					continue;
				};
				passes.push(ContentPass {
					job_id: Uuid::parse_str(&job_id)?,
					started,
					finished,
					output: serde_json::from_value(value)?,
				});
			}
			Ok(Ok(Event::JobFailed {
				job_type, error, ..
			})) if job_type == CONTENT_JOB => {
				return Err(anyhow!("Content identification failed: {error}"));
			}
			Ok(Err(_)) => {}
			Err(_) => return Err(anyhow!("Timed out waiting for content identification")),
			_ => {}
		}
	}

	Ok(passes)
}
