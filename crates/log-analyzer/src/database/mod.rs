//! Database storage and queries.

mod schema;

use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::sequence::SequencePattern;
use crate::types::{LogGroup, ParsedLog, Template};

/// Store analysis to SQLite database.
pub fn store_analysis(
	path: &Path,
	templates: &[Template],
	logs: &[ParsedLog],
	groups: &[LogGroup],
	sequences: &[SequencePattern],
) -> Result<()> {
	let mut conn = Connection::open(path)?;
	schema::create_schema(&mut conn)?;

	let tx = conn.transaction()?;

	// Insert templates
	for template in templates {
		tx.execute(
			"INSERT INTO templates (id, module, level, template_text, variable_schema, first_seen, last_seen, total_count) 
			 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
			params![
				template.id as i64,
				template.module,
				template.level.as_str(),
				template.example,
				serde_json::to_string(&template.variables)?,
				template.first_seen.map(|t| t.to_rfc3339()),
				template.last_seen.map(|t| t.to_rfc3339()),
				template.total_count as i64,
			],
		)?;
	}

	// Insert log instances
	for (idx, log) in logs.iter().enumerate() {
		let template_id = log.template_id.unwrap_or(0) as i64;

		tx.execute(
			"INSERT INTO log_instances (id, template_id, timestamp, thread_id, variables, raw_message) 
			 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
			params![
				idx as i64,
				template_id,
				log.timestamp.to_rfc3339(),
				log.thread_id,
				"{}",
				log.message,
			],
		)?;
	}

	// Insert log groups
	for (idx, group) in groups.iter().enumerate() {
		tx.execute(
			"INSERT INTO log_groups (id, template_id, count, start_time, end_time, duration_ms, variable_ranges) 
			 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
			params![
				idx as i64,
				group.template_id as i64,
				group.count as i64,
				group.start_time.to_rfc3339(),
				group.end_time.to_rfc3339(),
				group.duration_ms,
				serde_json::to_string(&group.variable_stats)?,
			],
		)?;
	}

	// Insert sequences
	for seq in sequences {
		tx.execute(
			"INSERT INTO sequences (id, template_sequence, repetitions, description, group_count) 
			 VALUES (?1, ?2, ?3, ?4, ?5)",
			params![
				seq.id as i64,
				serde_json::to_string(&seq.template_sequence)?,
				seq.repetitions as i64,
				seq.description,
				seq.group_indices.len() as i64,
			],
		)?;
	}

	tx.commit()?;

	Ok(())
}