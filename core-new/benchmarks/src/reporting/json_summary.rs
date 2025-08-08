use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::metrics::ScenarioResult;
use super::Reporter;

#[derive(Debug, Default)]
pub struct JsonSummaryReporter;

impl Reporter for JsonSummaryReporter {
    fn name(&self) -> &'static str { "json" }

    fn render(&self, runs: &[ScenarioResult], dest: &Path) -> Result<()> {
        let payload = json!({
            "runs": runs,
        });
        std::fs::write(dest, serde_json::to_string_pretty(&payload)?)?;
        Ok(())
    }
}
