use anyhow::Result;
use std::path::PathBuf;

use super::common::{run_jobs_and_collect_outputs, ScenarioBase};
use super::{hardware_hint_to_label, infer_hardware_label, Scenario};
use crate::core_boot::CoreBoot;
use crate::metrics::{collect_host_info, BenchmarkRun, Durations, RunMeta};
use crate::recipe::Recipe;
use sd_core::infra::job::output::JobOutput;

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
		"Generate content identities and report content-phase throughput"
	}

	async fn prepare(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<()> {
		use sd_core::infra::action::LibraryAction;
		let core = &boot.core;
		let context = core.context.clone();
		let library = core
			.libraries
			.create_library("Benchmarks", None, context.clone())
			.await?;
		self.base.library = Some(library.clone());

		for loc in &recipe.locations {
            let input = sd_core::ops::locations::add::action::LocationAddInput {
				path: sd_core::domain::addressing::SdPath::local(loc.path.clone()),
				name: Some(format!("bench:{}", recipe.name)),
				mode: sd_core::ops::indexing::IndexMode::Content,
                job_policies: None,
			};
			let action = sd_core::ops::locations::add::action::LocationAddAction::from_input(input)
				.map_err(|e| anyhow::anyhow!(e))?;
			let out = action
				.execute(library.clone(), context.clone())
				.await
				.map_err(|e| anyhow::anyhow!(e.to_string()))?;
			if let Some(job_id) = out.job_id {
				self.base.job_ids.push(job_id);
			}
		}
		Ok(())
	}

	async fn run(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<Vec<BenchmarkRun>> {
		let event_subscriber = boot.core.events.subscribe();
		let outputs = run_jobs_and_collect_outputs(&self.base.job_ids, event_subscriber).await?;
		let mut results = Vec::new();

		for (jid, output) in outputs {
			if let JobOutput::Indexed { stats, metrics } = output {
				let content_secs = metrics.content_duration.as_secs_f64();
				let files_per_s = if content_secs > 0.0 {
					stats.files as f64 / content_secs
				} else {
					0.0
				};
				let dirs_per_s = if content_secs > 0.0 {
					stats.dirs as f64 / content_secs
				} else {
					0.0
				};

				let location_paths: Vec<PathBuf> =
					recipe.locations.iter().map(|l| l.path.clone()).collect();
				let meta = RunMeta {
					id: jid,
					recipe_name: recipe.name.clone(),
					location_paths: location_paths.clone(),
					hardware_label: crate::metrics::derive_hardware_label_from_paths(
						&location_paths,
					)
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
				let durations = Durations {
					discovery_s: Some(metrics.discovery_duration.as_secs_f64()),
					processing_s: Some(metrics.processing_duration.as_secs_f64()),
					content_s: Some(content_secs),
					total_s: Some(content_secs),
				};
				results.push(BenchmarkRun::ContentIdentification {
					meta,
					files: stats.files,
					files_per_s,
					dirs: stats.dirs,
					dirs_per_s,
					total_gb: stats.bytes as f64 / 1_000_000_000.0,
					errors: stats.errors,
					durations,
				});
			}
		}
		Ok(results)
	}

	fn set_hardware_hint(&mut self, hint: Option<String>) {
		self.base.hardware_hint = hint;
	}
}
