//! Collapse consecutive log repetitions into groups.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use regex::Regex;

use crate::types::{LogGroup, ParsedLog, Template, VariableStat};

/// Collapse logs into groups based on templates.
pub fn collapse_logs(logs: &[ParsedLog], templates: &[Template]) -> Result<Vec<LogGroup>> {
	let mut groups = Vec::new();
	let mut current_group: Option<InProgressGroup> = None;

	// Pre-compile regex patterns
	let template_regexes: HashMap<u64, Regex> = templates
		.iter()
		.filter_map(|t| Regex::new(&t.regex_pattern).ok().map(|r| (t.id, r)))
		.collect();

	for (idx, log) in logs.iter().enumerate() {
		// Match log to template
		let template_id = match_template(log, templates, &template_regexes);

		match current_group.take() {
			Some(mut group) if group.template_id == template_id => {
				// Same template - add to current group
				group.add_instance(idx, log, templates);
				current_group = Some(group);
			}
			Some(group) => {
				// Different template - finalize current group and start new one
				groups.push(group.finalize());
				current_group = Some(InProgressGroup::new(template_id, idx, log, templates));
			}
			None => {
				// First log - start new group
				current_group = Some(InProgressGroup::new(template_id, idx, log, templates));
			}
		}
	}

	// Finalize last group
	if let Some(group) = current_group {
		groups.push(group.finalize());
	}

	Ok(groups)
}

/// Match a log to a template.
fn match_template(log: &ParsedLog, templates: &[Template], regexes: &HashMap<u64, Regex>) -> u64 {
	// Try to find a template that matches this log
	for template in templates {
		if template.module != log.module || template.level != log.level {
			continue;
		}

		if let Some(regex) = regexes.get(&template.id) {
			if regex.is_match(&log.message) {
				return template.id;
			}
		}
	}

	// No match found - return a default ID (0)
	0
}

/// In-progress group being built.
struct InProgressGroup {
	template_id: u64,
	indices: Vec<usize>,
	start_time: chrono::DateTime<chrono::Utc>,
	end_time: chrono::DateTime<chrono::Utc>,
	variable_values: HashMap<String, HashSet<String>>,
}

impl InProgressGroup {
	fn new(template_id: u64, idx: usize, log: &ParsedLog, templates: &[Template]) -> Self {
		let mut variable_values = HashMap::new();

		// Extract variables from first log
		if let Some(template) = templates.iter().find(|t| t.id == template_id) {
			extract_variables(log, template, &mut variable_values);
		}

		Self {
			template_id,
			indices: vec![idx],
			start_time: log.timestamp,
			end_time: log.timestamp,
			variable_values,
		}
	}

	fn add_instance(&mut self, idx: usize, log: &ParsedLog, templates: &[Template]) {
		self.indices.push(idx);
		self.end_time = log.timestamp;

		// Extract and track variable values
		if let Some(template) = templates.iter().find(|t| t.id == self.template_id) {
			extract_variables(log, template, &mut self.variable_values);
		}
	}

	fn finalize(self) -> LogGroup {
		let duration_ms = (self.end_time - self.start_time).num_milliseconds();

		let variable_stats = self
			.variable_values
			.into_iter()
			.map(|(name, values)| {
				let stat = if values.len() == 1 {
					VariableStat::Constant(values.into_iter().next().unwrap())
				} else if values.len() <= 10 {
					VariableStat::Unique(values)
				} else {
					VariableStat::Distribution {
						total: self.indices.len(),
						unique: values.len(),
					}
				};
				(name, stat)
			})
			.collect();

		// Sample indices: first 3 + last 3
		let sample_indices = if self.indices.len() <= 6 {
			self.indices.clone()
		} else {
			let mut samples = Vec::new();
			samples.extend_from_slice(&self.indices[..3]);
			samples.extend_from_slice(&self.indices[self.indices.len() - 3..]);
			samples
		};

		LogGroup {
			template_id: self.template_id,
			count: self.indices.len(),
			start_time: self.start_time,
			end_time: self.end_time,
			duration_ms,
			variable_stats,
			sample_indices,
		}
	}
}

/// Extract variable values from a log message.
fn extract_variables(
	log: &ParsedLog,
	template: &Template,
	variable_values: &mut HashMap<String, HashSet<String>>,
) {
	// Compile regex if needed
	let regex = match Regex::new(&template.regex_pattern) {
		Ok(r) => r,
		Err(_) => return,
	};

	// Try to extract captures
	if let Some(captures) = regex.captures(&log.message) {
		for (i, var) in template.variables.iter().enumerate() {
			if let Some(capture) = captures.get(i + 1) {
				variable_values
					.entry(var.name.clone())
					.or_default()
					.insert(capture.as_str().to_string());
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::{LogLevel, Variable, VariableType};
	use chrono::Utc;

	fn create_test_log(message: &str, offset_secs: i64) -> ParsedLog {
		ParsedLog {
			timestamp: Utc::now() + chrono::Duration::seconds(offset_secs),
			level: LogLevel::Debug,
			thread_id: None,
			module: "test::module".to_string(),
			message: message.to_string(),
			raw: String::new(),
			template_id: None,
		}
	}

	fn create_test_template(id: u64) -> Template {
		Template {
			id,
			module: "test::module".to_string(),
			level: LogLevel::Debug,
			static_parts: vec!["Recorded".to_string(), "ACK".to_string()],
			variables: vec![Variable {
				name: "peer_id".to_string(),
				position: 2,
				var_type: VariableType::Number,
			}],
			regex_pattern: r"Recorded\s*ACK\s*(\d+)".to_string(),
			example: "Recorded ACK 123".to_string(),
			total_count: 0,
			first_seen: None,
			last_seen: None,
		}
	}

	#[test]
	fn test_collapse_single_group() {
		let logs = vec![
			create_test_log("Recorded ACK 123", 0),
			create_test_log("Recorded ACK 456", 1),
			create_test_log("Recorded ACK 789", 2),
		];

		let templates = vec![create_test_template(1)];

		let groups = collapse_logs(&logs, &templates).unwrap();
		assert_eq!(groups.len(), 1);
		assert_eq!(groups[0].count, 3);
	}

	#[test]
	fn test_collapse_multiple_groups() {
		let logs = vec![
			create_test_log("Recorded ACK 123", 0),
			create_test_log("Different message", 1),
			create_test_log("Recorded ACK 456", 2),
		];

		let templates = vec![create_test_template(1)];

		let groups = collapse_logs(&logs, &templates).unwrap();
		// Should have at least 2 groups (ACK messages might be split by different message)
		assert!(groups.len() >= 2);
	}
}