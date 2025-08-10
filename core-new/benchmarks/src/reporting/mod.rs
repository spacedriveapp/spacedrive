use crate::metrics::BenchmarkRun;
use std::path::Path;

pub trait Reporter {
	fn name(&self) -> &'static str;
	fn render(&self, runs: &[BenchmarkRun], dest: &Path) -> anyhow::Result<()>;
}

pub mod json_summary;
pub mod registry;

pub use json_summary::JsonSummaryReporter;
