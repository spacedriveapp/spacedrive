//! Markdown report generation.

use anyhow::Result;

use crate::sequence::SequencePattern;
use crate::types::{LogGroup, ParsedLog, Template};

/// Generate a markdown report from analysis.
pub fn generate_markdown_report(
	logs: &[ParsedLog],
	templates: &[Template],
	groups: &[LogGroup],
	sequences: &[SequencePattern],
) -> Result<String> {
	let mut report = String::new();

	// Header
	report.push_str("# Log Analysis Report\n\n");

	// Summary statistics
	report.push_str("## Summary\n\n");
	report.push_str(&format!("- **Total log lines:** {}\n", logs.len()));
	report.push_str(&format!("- **Unique templates:** {}\n", templates.len()));
	report.push_str(&format!("- **Collapsed groups:** {}\n", groups.len()));
	report.push_str(&format!("- **Detected sequences:** {}\n", sequences.len()));

	if !logs.is_empty() {
		let stats = crate::sequence::calculate_compression(logs.len(), groups.len(), sequences);
		report.push_str(&format!(
			"- **Final compressed count:** {}\n",
			stats.final_count
		));
		report.push_str(&format!(
			"- **Compression ratio:** {:.1}%\n",
			stats.compression_ratio * 100.0
		));
	}

	if !logs.is_empty() {
		report.push_str(&format!(
			"- **Time range:** {} to {}\n",
			logs.first().unwrap().timestamp.format("%Y-%m-%d %H:%M:%S"),
			logs.last().unwrap().timestamp.format("%Y-%m-%d %H:%M:%S")
		));
	}

	report.push('\n');

	// Top templates by frequency
	report.push_str("## Top Templates by Frequency\n\n");

	let mut sorted_templates = templates.to_vec();
	sorted_templates.sort_by(|a, b| b.total_count.cmp(&a.total_count));

	for (i, template) in sorted_templates.iter().take(10).enumerate() {
		report.push_str(&format!("### {}. Template #{}\n\n", i + 1, template.id));
		report.push_str(&format!("- **Count:** {}\n", template.total_count));
		report.push_str(&format!("- **Module:** `{}`\n", template.module));
		report.push_str(&format!("- **Level:** {}\n", template.level.as_str()));
		report.push_str(&format!("- **Example:** `{}`\n", template.example));

		if !template.variables.is_empty() {
			report.push_str("- **Variables:**\n");
			for var in &template.variables {
				report.push_str(&format!("  - `{}` ({})\n", var.name, var.var_type.as_str()));
			}
		}

		report.push('\n');
	}

	// Top groups by count
	report.push_str("## Top Collapsed Groups\n\n");

	let mut sorted_groups = groups.to_vec();
	sorted_groups.sort_by(|a, b| b.count.cmp(&a.count));

	for (i, group) in sorted_groups.iter().take(10).enumerate() {
		let template = templates.iter().find(|t| t.id == group.template_id);

		report.push_str(&format!(
			"### {}. Group (Template #{})\n\n",
			i + 1,
			group.template_id
		));
		report.push_str(&format!("- **Count:** {} instances\n", group.count));
		report.push_str(&format!("- **Duration:** {}ms\n", group.duration_ms));
		report.push_str(&format!(
			"- **Time range:** {} to {}\n",
			group.start_time.format("%H:%M:%S%.3f"),
			group.end_time.format("%H:%M:%S%.3f")
		));

		if let Some(template) = template {
			report.push_str(&format!("- **Template:** `{}`\n", template.example));
		}

		if !group.variable_stats.is_empty() {
			report.push_str("- **Variable Statistics:**\n");
			for (name, stat) in &group.variable_stats {
				report.push_str(&format!("  - `{}`: {}\n", name, stat.format()));
			}
		}

		report.push('\n');
	}

	// Detected sequences
	if !sequences.is_empty() {
		report.push_str("## Detected Sequences\n\n");
		report.push_str("Repeating patterns of log groups:\n\n");

		let mut sorted_sequences = sequences.to_vec();
		sorted_sequences.sort_by(|a, b| b.repetitions.cmp(&a.repetitions));

		for (i, seq) in sorted_sequences.iter().take(10).enumerate() {
			report.push_str(&format!(
				"### {}. Sequence #{} ({}× repetitions)\n\n",
				i + 1,
				seq.id,
				seq.repetitions
			));
			report.push_str(&format!(
				"- **Pattern length:** {} steps\n",
				seq.template_sequence.len()
			));
			report.push_str(&format!(
				"- **Total groups collapsed:** {}\n",
				seq.group_indices.len()
			));
			report.push_str("- **Template sequence:**\n");
			for (step, &template_id) in seq.template_sequence.iter().enumerate() {
				if let Some(template) = templates.iter().find(|t| t.id == template_id) {
					report.push_str(&format!(
						"  {}. Template #{}: `{}`\n",
						step + 1,
						template_id,
						template.example
					));
				}
			}
			report.push('\n');
		}
	}

	Ok(report)
}
