//! Timeline generation and analysis.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{LogGroup, ParsedLog};

/// Timeline view of log activity.
#[derive(Debug, Serialize, Deserialize)]
pub struct Timeline {
	pub start: DateTime<Utc>,
	pub end: DateTime<Utc>,
	pub buckets: Vec<TimelineBucket>,
}

/// Time bucket with event counts.
#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineBucket {
	pub timestamp: DateTime<Utc>,
	pub count: usize,
	pub template_counts: HashMap<u64, usize>,
}

/// Generate timeline from logs and groups.
pub fn generate_timeline(logs: &[ParsedLog], _groups: &[LogGroup]) -> Result<Timeline> {
	if logs.is_empty() {
		anyhow::bail!("Cannot generate timeline from empty logs");
	}

	let start = logs.first().unwrap().timestamp;
	let end = logs.last().unwrap().timestamp;

	// Create buckets (1 second intervals)
	let mut buckets = Vec::new();
	let total_duration = (end - start).num_seconds();
	let bucket_size = if total_duration > 300 {
		// > 5 minutes, use 5-second buckets
		5
	} else if total_duration > 60 {
		// > 1 minute, use 1-second buckets
		1
	} else {
		// < 1 minute, use 100ms buckets
		1
	};

	let mut current = start;
	while current <= end {
		let bucket_end = current + chrono::Duration::seconds(bucket_size);

		let logs_in_bucket: Vec<_> = logs
			.iter()
			.filter(|l| l.timestamp >= current && l.timestamp < bucket_end)
			.collect();

		let mut template_counts = HashMap::new();
		for log in &logs_in_bucket {
			if let Some(template_id) = log.template_id {
				*template_counts.entry(template_id).or_insert(0) += 1;
			}
		}

		buckets.push(TimelineBucket {
			timestamp: current,
			count: logs_in_bucket.len(),
			template_counts,
		});

		current = bucket_end;
	}

	Ok(Timeline {
		start,
		end,
		buckets,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::LogLevel;

	fn create_test_log(offset_secs: i64) -> ParsedLog {
		ParsedLog {
			timestamp: Utc::now() + chrono::Duration::seconds(offset_secs),
			level: LogLevel::Debug,
			thread_id: None,
			module: "test".to_string(),
			message: "test".to_string(),
			raw: String::new(),
			template_id: Some(1),
		}
	}

	#[test]
	fn test_generate_timeline() {
		let logs = vec![
			create_test_log(0),
			create_test_log(1),
			create_test_log(2),
			create_test_log(5),
		];

		let timeline = generate_timeline(&logs, &[]).unwrap();

		assert!(!timeline.buckets.is_empty());
		let total_count: usize = timeline.buckets.iter().map(|b| b.count).sum();
		assert_eq!(total_count, 4);
	}
}