//! Template detection and matching.

use std::collections::HashMap;

use anyhow::Result;

use super::{lcs, tokenizer, types};
use crate::types::{LogLevel, ParsedLog, Template, Variable};

/// Detect templates from parsed logs.
pub fn detect_templates(logs: &[ParsedLog]) -> Result<Vec<Template>> {
	// Group logs by module and level
	let mut groups: HashMap<(String, LogLevel), Vec<&ParsedLog>> = HashMap::new();

	for log in logs {
		groups
			.entry((log.module.clone(), log.level))
			.or_default()
			.push(log);
	}

	let mut templates = Vec::new();
	let mut template_id = 0u64;

	for ((module, level), group_logs) in groups {
		// Further group by message similarity
		let message_groups = group_by_similarity(group_logs);

		for msg_group in message_groups {
			let template = create_template(template_id, &module, level, &msg_group)?;
			templates.push(template);
			template_id += 1;
		}
	}

	Ok(templates)
}

/// Group logs by message similarity.
///
/// This is a simple approach: we'll compare each message's tokenized form
/// and group those that have the same structure.
fn group_by_similarity(logs: Vec<&ParsedLog>) -> Vec<Vec<&ParsedLog>> {
	let mut groups: Vec<Vec<&ParsedLog>> = Vec::new();

	for log in logs {
		let tokens = tokenizer::tokenize(&log.message);

		// Find a matching group
		let mut found_group = false;
		for group in &mut groups {
			if group.is_empty() {
				continue;
			}

			let group_tokens = tokenizer::tokenize(&group[0].message);

			// Check if token patterns are similar (same length, similar structure)
			if tokens.len() == group_tokens.len() {
				// Count how many positions match
				let matches = tokens
					.iter()
					.zip(&group_tokens)
					.filter(|(a, b)| a == b)
					.count();

				// If > 70% match, consider them similar
				if matches as f64 / tokens.len() as f64 > 0.7 {
					group.push(log);
					found_group = true;
					break;
				}
			}
		}

		if !found_group {
			groups.push(vec![log]);
		}
	}

	groups
}

/// Create a template from a group of similar logs.
fn create_template(
	id: u64,
	module: &str,
	level: LogLevel,
	logs: &[&ParsedLog],
) -> Result<Template> {
	if logs.is_empty() {
		anyhow::bail!("Cannot create template from empty log group");
	}

	// If only one log, create a literal template
	if logs.len() == 1 {
		return Ok(Template {
			id,
			module: module.to_string(),
			level,
			static_parts: vec![logs[0].message.clone()],
			variables: Vec::new(),
			regex_pattern: regex::escape(&logs[0].message),
			example: logs[0].message.clone(),
			total_count: 1,
			first_seen: Some(logs[0].timestamp),
			last_seen: Some(logs[0].timestamp),
		});
	}

	// Tokenize all messages
	let token_sequences: Vec<Vec<_>> = logs
		.iter()
		.map(|log| tokenizer::tokenize(&log.message))
		.collect();

	// Find common and variable positions
	let common_positions = lcs::find_common_positions(&token_sequences);
	let variable_positions = lcs::find_variable_positions(&token_sequences);

	// Build static parts
	let first_tokens = &token_sequences[0];
	let mut static_parts = Vec::new();
	let mut variables = Vec::new();

	// Extract values for each variable position
	for &var_pos in &variable_positions {
		let values: Vec<&str> = token_sequences
			.iter()
			.filter_map(|seq| seq.get(var_pos).map(|t| t.as_str()))
			.collect();

		let var_type = types::infer_variable_type(&values);

		variables.push(Variable {
			name: format!("var{}", variables.len()),
			position: var_pos,
			var_type,
		});
	}

	// Build static parts between variables
	for &pos in &common_positions {
		if let Some(token) = first_tokens.get(pos) {
			static_parts.push(token.as_str().to_string());
		}
	}

	// Build regex pattern
	let regex_pattern = build_regex_pattern(&token_sequences[0], &variable_positions);

	let first_seen = logs.iter().map(|l| l.timestamp).min();
	let last_seen = logs.iter().map(|l| l.timestamp).max();

	Ok(Template {
		id,
		module: module.to_string(),
		level,
		static_parts,
		variables,
		regex_pattern,
		example: logs[0].message.clone(),
		total_count: logs.len(),
		first_seen,
		last_seen,
	})
}

/// Build regex pattern for matching logs to this template.
fn build_regex_pattern(tokens: &[crate::types::Token], variable_positions: &[usize]) -> String {
	let mut pattern = String::new();

	for (i, token) in tokens.iter().enumerate() {
		if variable_positions.contains(&i) {
			// Variable position - match any non-whitespace
			pattern.push_str(r"\S+");
		} else {
			// Static part - match literally
			pattern.push_str(&regex::escape(token.as_str()));
		}

		// Add space between tokens (except last)
		if i < tokens.len() - 1 {
			pattern.push_str(r"\s*");
		}
	}

	pattern
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;

	fn create_test_log(message: &str) -> ParsedLog {
		ParsedLog {
			timestamp: Utc::now(),
			level: LogLevel::Debug,
			thread_id: None,
			module: "test::module".to_string(),
			message: message.to_string(),
			raw: String::new(),
			template_id: None,
		}
	}

	#[test]
	fn test_detect_templates_single() {
		let logs = vec![create_test_log("Recorded ACK from peer")];

		let templates = detect_templates(&logs).unwrap();
		assert_eq!(templates.len(), 1);
		assert_eq!(templates[0].variables.len(), 0); // No variables in single log
	}

	#[test]
	fn test_detect_templates_similar() {
		let logs = vec![
			create_test_log("Recorded ACK from peer peer=123"),
			create_test_log("Recorded ACK from peer peer=456"),
		];

		let templates = detect_templates(&logs).unwrap();
		assert_eq!(templates.len(), 1);
		assert!(templates[0].variables.len() > 0); // Should detect variable
	}

	#[test]
	fn test_detect_templates_different() {
		let logs = vec![
			create_test_log("Starting sync service"),
			create_test_log("Recorded ACK from peer"),
		];

		let templates = detect_templates(&logs).unwrap();
		assert_eq!(templates.len(), 2); // Different patterns
	}
}





