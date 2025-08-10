use std::path::Path;

use anyhow::Result;
use serde_json::json;

use super::Reporter;
use crate::metrics::BenchmarkRun;

#[derive(Debug, Default)]
pub struct JsonSummaryReporter;

impl Reporter for JsonSummaryReporter {
	fn name(&self) -> &'static str {
		"json"
	}

	fn render(&self, runs: &[BenchmarkRun], dest: &Path) -> Result<()> {
		let runs_enum: Vec<BenchmarkRun> = runs.to_vec();
		let payload = json!({
			"runs": runs_enum,
		});
		std::fs::write(dest, serde_json::to_string_pretty(&payload)?)?;
		Ok(())
	}
}
