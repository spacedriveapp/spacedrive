use anyhow::Result;
use std::path::PathBuf;

use super::common::{run_jobs_and_collect_outputs, ScenarioBase};
use super::{hardware_hint_to_label, infer_hardware_label, Scenario};
use crate::core_boot::CoreBoot;
use crate::metrics::{collect_host_info, BenchmarkRun, Durations, RunMeta};
use crate::recipe::Recipe;
use sd_core::infra::job::output::JobOutput;

#[derive(Default)]
pub struct CoreIndexingScenario {
	base: ScenarioBase,
}

#[async_trait::async_trait]
impl Scenario for CoreIndexingScenario {
	fn name(&self) -> &'static str {
		"core_indexing"
	}

	fn describe(&self) -> &'static str {
		"Measure core indexing throughput (Discovery + Processing)"
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
				mode: sd_core::ops::indexing::IndexMode::Shallow,
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

				// --- IndexingDiscovery Result ---
				let discovery_secs = metrics.discovery_duration.as_secs_f64();
				let files_per_s_discovery = if discovery_secs > 0.0 {
					stats.files as f64 / discovery_secs
				} else {
					0.0
				};
				let dirs_per_s_discovery = if discovery_secs > 0.0 {
					stats.dirs as f64 / discovery_secs
				} else {
					0.0
				};
				results.push(BenchmarkRun::IndexingDiscovery {
					meta: meta.clone(),
					files: stats.files,
					files_per_s: files_per_s_discovery,
					dirs: stats.dirs,
					dirs_per_s: dirs_per_s_discovery,
					total_gb: stats.bytes as f64 / 1_000_000_000.0,
					errors: stats.errors,
					durations: Durations {
						discovery_s: Some(discovery_secs),
						processing_s: Some(metrics.processing_duration.as_secs_f64()),
						content_s: Some(metrics.content_duration.as_secs_f64()),
						total_s: Some(discovery_secs),
					},
				});

				// --- Processing Result ---
				let processing_secs = metrics.processing_duration.as_secs_f64();
				let files_per_s_processing = if processing_secs > 0.0 {
					stats.files as f64 / processing_secs
				} else {
					0.0
				};
				let dirs_per_s_processing = if processing_secs > 0.0 {
					stats.dirs as f64 / processing_secs
				} else {
					0.0
				};
				results.push(BenchmarkRun::Processing {
					meta,
					files: stats.files,
					files_per_s: files_per_s_processing,
					dirs: stats.dirs,
					dirs_per_s: dirs_per_s_processing,
					total_gb: stats.bytes as f64 / 1_000_000_000.0,
					errors: stats.errors,
					durations: Durations {
						discovery_s: Some(metrics.discovery_duration.as_secs_f64()),
						processing_s: Some(processing_secs),
						content_s: Some(metrics.content_duration.as_secs_f64()),
						total_s: Some(processing_secs),
					},
				});
			}
		}
		Ok(results)
	}

	fn set_hardware_hint(&mut self, hint: Option<String>) {
		self.base.hardware_hint = hint;
	}
}
