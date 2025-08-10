use std::path::Path;

use anyhow::Result;

use super::Reporter;
use crate::metrics::BenchmarkRun;

/// Generates CSV format with all individual runs
#[derive(Debug, Default)]
pub struct CsvReporter;

impl CsvReporter {
	fn phase_for_scenario(scenario: &str) -> &'static str {
		match scenario {
			"indexing_discovery" => "Discovery",
			"aggregation" => "Processing",
			"content_identification" => "Content Identification",
			_ => "Unknown",
		}
	}

	fn phase_rank(phase: &str) -> i32 {
		match phase {
			"Discovery" => 0,
			"Processing" => 1,
			"Content Identification" => 2,
			_ => 9,
		}
	}
}

impl Reporter for CsvReporter {
	fn name(&self) -> &'static str {
		"csv"
	}

	fn render(&self, runs: &[BenchmarkRun], dest: &Path) -> Result<()> {
		// Collect all data rows first so we can sort them
		let mut data_rows: Vec<(String, String, f64, f64, u64, u64, f64, u64, String, f64)> = Vec::new();

		// Process each run individually
		for run in runs {
			let (scenario, meta, files, files_per_s, dirs, dirs_per_s, total_gb, errors, durations) =
				match run {
					BenchmarkRun::IndexingDiscovery {
						meta,
						files,
						files_per_s,
						dirs,
						dirs_per_s,
						total_gb,
						errors,
						durations,
					} => (
						"indexing_discovery",
						meta,
						*files,
						*files_per_s,
						*dirs,
						*dirs_per_s,
						*total_gb,
						*errors,
						durations,
					),
					BenchmarkRun::Aggregation {
						meta,
						files,
						files_per_s,
						dirs,
						dirs_per_s,
						total_gb,
						errors,
						durations,
					} => (
						"aggregation",
						meta,
						*files,
						*files_per_s,
						*dirs,
						*dirs_per_s,
						*total_gb,
						*errors,
						durations,
					),
					BenchmarkRun::ContentIdentification {
						meta,
						files,
						files_per_s,
						dirs,
						dirs_per_s,
						total_gb,
						errors,
						durations,
					} => (
						"content_identification",
						meta,
						*files,
						*files_per_s,
						*dirs,
						*dirs_per_s,
						*total_gb,
						*errors,
						durations,
					),
				};

			let phase = Self::phase_for_scenario(scenario);
			let hardware = meta
				.hardware_label
				.clone()
				.unwrap_or_else(|| "Unknown".to_string());

			// Get phase-specific duration or fall back to total
			let phase_duration = match scenario {
				"indexing_discovery" => durations.discovery_s.unwrap_or(durations.total_s.unwrap_or(0.0)),
				"aggregation" => durations.processing_s.unwrap_or(durations.total_s.unwrap_or(0.0)),
				"content_identification" => durations.content_s.unwrap_or(durations.total_s.unwrap_or(0.0)),
				_ => durations.total_s.unwrap_or(0.0),
			};

			// Calculate GB/s using phase-specific duration
			let gb_per_s = if phase_duration > 0.0 {
				total_gb / phase_duration
			} else {
				0.0
			};

			data_rows.push((
				phase.to_string(),
				hardware,
				files_per_s,
				gb_per_s,
				files,
				dirs,
				total_gb,
				errors,
				meta.recipe_name.clone(),
				phase_duration,
			));
		}

		// Sort by phase rank, then hardware, then recipe name
		data_rows.sort_by(|a, b| {
			Self::phase_rank(&a.0)
				.cmp(&Self::phase_rank(&b.0))
				.then(a.1.cmp(&b.1))
				.then(a.8.cmp(&b.8))
		});

		// Build CSV output
		let mut rows =
			vec!["Phase,Hardware,Files_per_s,GB_per_s,Files,Dirs,GB,Errors,Recipe,Duration_s".to_string()];

		for (phase, hardware, files_per_s, gb_per_s, files, dirs, total_gb, errors, recipe, duration) in
			data_rows
		{
			rows.push(format!(
				"{},{},{:.1},{:.2},{},{},{:.2},{},{},{:.1}",
				phase, hardware, files_per_s, gb_per_s, files, dirs, total_gb, errors, recipe, duration
			));
		}

		let content = rows.join("\n") + "\n";
		std::fs::write(dest, content)?;
		Ok(())
	}
}
