//! Database schema creation.

use anyhow::Result;
use rusqlite::Connection;

/// Create database schema.
pub fn create_schema(conn: &mut Connection) -> Result<()> {
	conn.execute(
		"CREATE TABLE IF NOT EXISTS templates (
			id INTEGER PRIMARY KEY,
			module TEXT NOT NULL,
			level TEXT NOT NULL,
			template_text TEXT NOT NULL,
			variable_schema TEXT NOT NULL,
			first_seen TEXT,
			last_seen TEXT,
			total_count INTEGER
		)",
		[],
	)?;

	conn.execute(
		"CREATE TABLE IF NOT EXISTS log_instances (
			id INTEGER PRIMARY KEY,
			template_id INTEGER,
			timestamp TEXT NOT NULL,
			thread_id TEXT,
			variables TEXT NOT NULL,
			raw_message TEXT,
			FOREIGN KEY (template_id) REFERENCES templates(id)
		)",
		[],
	)?;

	conn.execute(
		"CREATE TABLE IF NOT EXISTS log_groups (
			id INTEGER PRIMARY KEY,
			template_id INTEGER,
			count INTEGER NOT NULL,
			start_time TEXT NOT NULL,
			end_time TEXT NOT NULL,
			duration_ms INTEGER,
			variable_ranges TEXT NOT NULL,
			FOREIGN KEY (template_id) REFERENCES templates(id)
		)",
		[],
	)?;

	// Create indices
	conn.execute(
		"CREATE INDEX IF NOT EXISTS idx_instances_time ON log_instances(timestamp)",
		[],
	)?;

	conn.execute(
		"CREATE INDEX IF NOT EXISTS idx_instances_template ON log_instances(template_id)",
		[],
	)?;

	conn.execute(
		"CREATE INDEX IF NOT EXISTS idx_groups_template ON log_groups(template_id)",
		[],
	)?;

	// Create sequences table
	conn.execute(
		"CREATE TABLE IF NOT EXISTS sequences (
			id INTEGER PRIMARY KEY,
			template_sequence TEXT NOT NULL,
			repetitions INTEGER NOT NULL,
			description TEXT NOT NULL,
			group_count INTEGER NOT NULL
		)",
		[],
	)?;

	Ok(())
}
