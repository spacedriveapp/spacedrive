//! Log line parsing and extraction.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::DateTime;
use regex::Regex;

use crate::types::{LogLevel, ParsedLog};

/// Parse log file into structured entries.
pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<ParsedLog>> {
	let file = File::open(path.as_ref())
		.with_context(|| format!("Failed to open log file: {:?}", path.as_ref()))?;
	let reader = BufReader::new(file);
	let mut logs = Vec::new();

	for line in reader.lines() {
		let line = line?;
		if let Some(log) = parse_line(&line) {
			logs.push(log);
		}
	}

	Ok(logs)
}

/// Parse log string into structured entries.
pub fn parse_string(content: &str) -> Result<Vec<ParsedLog>> {
	let mut logs = Vec::new();

	for line in content.lines() {
		if let Some(log) = parse_line(line) {
			logs.push(log);
		}
	}

	Ok(logs)
}

/// Parse a single log line.
///
/// Expected format:
/// `2025-11-16T07:19:57.232531Z DEBUG ThreadId(02) sd_core::service::sync::peer: Message`
pub fn parse_line(line: &str) -> Option<ParsedLog> {
	// Regex to match tracing logs
	// Format: TIMESTAMP LEVEL ThreadId(XX) module::path: message
	let re = Regex::new(
		r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s+(\w+)\s+(?:ThreadId\((\d+)\)\s+)?([a-zA-Z0-9_:]+):\s+(.*)$"
	).ok()?;

	let captures = re.captures(line)?;

	let timestamp_str = captures.get(1)?.as_str();
	let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
		.ok()?
		.with_timezone(&chrono::Utc);

	let level_str = captures.get(2)?.as_str();
	let level = level_str.parse::<LogLevel>().ok()?;

	let thread_id = captures.get(3).map(|m| m.as_str().to_string());

	let module = captures.get(4)?.as_str().to_string();

	let message = captures.get(5)?.as_str().to_string();

	Some(ParsedLog {
		timestamp,
		level,
		thread_id,
		module,
		message,
		raw: line.to_string(),
		template_id: None,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_line_with_thread() {
		let line = "2025-11-16T07:19:57.232531Z DEBUG ThreadId(02) sd_core::service::sync::peer: Recorded ACK from peer peer=1817e146";

		let log = parse_line(line).expect("Failed to parse");

		assert_eq!(log.level, LogLevel::Debug);
		assert_eq!(log.thread_id, Some("02".to_string()));
		assert_eq!(log.module, "sd_core::service::sync::peer");
		assert!(log.message.contains("Recorded ACK"));
	}

	#[test]
	fn test_parse_line_without_thread() {
		let line = "2025-11-16T07:19:57.232531Z INFO sd_core::service::sync: Starting sync service";

		let log = parse_line(line).expect("Failed to parse");

		assert_eq!(log.level, LogLevel::Info);
		assert_eq!(log.thread_id, None);
		assert_eq!(log.module, "sd_core::service::sync");
	}

	#[test]
	fn test_parse_string() {
		let content = r#"2025-11-16T07:19:57.232531Z DEBUG ThreadId(02) sd_core::service::sync::peer: Message 1
2025-11-16T07:19:57.232532Z INFO sd_core::service::sync: Message 2
2025-11-16T07:19:57.232533Z ERROR sd_core::service::sync::peer: Message 3"#;

		let logs = parse_string(content).expect("Failed to parse");

		assert_eq!(logs.len(), 3);
		assert_eq!(logs[0].level, LogLevel::Debug);
		assert_eq!(logs[1].level, LogLevel::Info);
		assert_eq!(logs[2].level, LogLevel::Error);
	}
}