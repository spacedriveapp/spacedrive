use anyhow::Result;
use std::path::Path;

use crate::core_boot::CoreBoot;
use crate::generator::DatasetGenerator;
use crate::metrics::BenchmarkRun;
use crate::recipe::Recipe;
use crate::reporting::Reporter;
use crate::scenarios::Scenario;

pub async fn run_scenario(
	boot: &CoreBoot,
	generator: &dyn DatasetGenerator,
	scenario: &mut dyn Scenario,
	reporters: &[Box<dyn Reporter>],
	recipe: &Recipe,
	report_dest: Option<&Path>,
) -> Result<Vec<BenchmarkRun>> {
	generator.generate(recipe).await?;
	scenario.prepare(boot, recipe).await?;
	let results = scenario.run(boot, recipe).await?;

	if let Some(dest) = report_dest {
		// Ensure parent exists to prevent silent failure
		if let Some(parent) = dest.parent() {
			std::fs::create_dir_all(parent).ok();
		}
		for r in reporters {
			r.render(&results, dest)?;
		}
	}

	Ok(results)
}

pub mod monitor;
