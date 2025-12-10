//! Phase-based summary - group logs by time phases and show template counts

use crate::{LogAnalyzer, LogLevel};
use anyhow::Result;
use std::collections::HashMap;

/// Generate phase-based summary showing template counts per time window
pub fn generate_phase_summary(analyzer: &LogAnalyzer, phase_duration_secs: u64) -> Result<String> {
	let mut output = String::new();

	output.push_str("# PHASE-BASED SUMMARY\n");
	output.push_str(&format!(
		"# Aggregates log activity into {}s time windows\n",
		phase_duration_secs
	));
	output.push_str("# Format: [COUNT×] module: message\n\n");

	let groups = analyzer.groups();
	if groups.is_empty() {
		return Ok(output);
	}

	// Find time range
	let start_time = groups.first().unwrap().start_time;
	let end_time = groups.last().unwrap().end_time;

	// Create phases
	let total_duration = end_time.signed_duration_since(start_time).num_seconds() as u64;
	let num_phases = (total_duration / phase_duration_secs) + 1;

	let templates = analyzer.templates();

	for phase_idx in 0..num_phases {
		let phase_start =
			start_time + chrono::Duration::seconds((phase_idx * phase_duration_secs) as i64);
		let phase_end =
			start_time + chrono::Duration::seconds(((phase_idx + 1) * phase_duration_secs) as i64);

		// Collect template counts in this phase
		let mut template_counts: HashMap<u64, usize> = HashMap::new();
		let mut warning_count = 0;
		let mut error_count = 0;
		let mut total_events = 0;

		for group in groups {
			if group.start_time >= phase_start && group.start_time < phase_end {
				*template_counts.entry(group.template_id).or_insert(0) += group.count;
				total_events += group.count;

				// Get template to check level
				if let Some(template) = templates.iter().find(|t| t.id == group.template_id) {
					match template.level {
						LogLevel::Warn => warning_count += group.count,
						LogLevel::Error => error_count += group.count,
						_ => {}
					}
				}
			}
		}

		if template_counts.is_empty() {
			continue;
		}

		// Render phase header with stats
		let duration = (phase_end - phase_start).num_seconds();
		output.push_str(&format!(
			"\n## {} → {} ({}s, {} events",
			phase_start.format("%H:%M:%S"),
			phase_end.format("%H:%M:%S"),
			duration,
			total_events
		));

		if warning_count > 0 || error_count > 0 {
			output.push_str(&format!(
				", {} warnings, {} errors",
				warning_count, error_count
			));
		}
		output.push_str(")\n\n");

		// Sort by frequency
		let mut sorted: Vec<_> = template_counts.iter().collect();
		sorted.sort_by(|a, b| b.1.cmp(a.1));

		// Group by module for better readability
		let mut by_module: HashMap<String, Vec<(u64, usize)>> = HashMap::new();
		for (&template_id, &count) in &sorted {
			let template = templates.iter().find(|t| t.id == template_id).unwrap();
			let module_base = template
				.module
				.split("::")
				.take(3)
				.collect::<Vec<_>>()
				.join("::");
			by_module
				.entry(module_base)
				.or_default()
				.push((template_id, count));
		}

		// Show key operations at top level
		output.push_str("### Key Operations\n\n");
		for (&template_id, &count) in sorted.iter().take(10) {
			let template = templates.iter().find(|t| t.id == template_id).unwrap();
			let module_short = template
				.module
				.split("::")
				.last()
				.unwrap_or(&template.module);

			// Highlight important sync operations
			let marker = match template.level {
				LogLevel::Error => "",
				LogLevel::Warn => "️ ",
				LogLevel::Info if count > 100 => "",
				_ => "  ",
			};

			output.push_str(&format!(
				"{} [{:>5}×] {}: {}\n",
				marker,
				count,
				module_short,
				truncate(&template.example, 90)
			));
		}

		// Show module breakdown
		output.push_str("\n### By Module\n\n");
		let mut modules_sorted: Vec<_> = by_module.iter().collect();
		modules_sorted.sort_by(|a, b| {
			let a_total: usize = a.1.iter().map(|(_, c)| c).sum();
			let b_total: usize = b.1.iter().map(|(_, c)| c).sum();
			b_total.cmp(&a_total)
		});

		for (module, template_list) in modules_sorted.iter().take(8) {
			let total: usize = template_list.iter().map(|(_, c)| c).sum();
			output.push_str(&format!("  {}: {} events\n", module, total));
		}
	}

	Ok(output)
}

fn truncate(s: &str, max_len: usize) -> String {
	if s.len() <= max_len {
		s.to_string()
	} else {
		format!("{}...", &s[..max_len - 3])
	}
}
