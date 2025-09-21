//! Generic progress system for job monitoring
//!
//! This module provides a unified progress structure that jobs can convert
//! their domain-specific progress into, making progress data compatible
//! with the job monitoring system while preserving rich information.

use crate::domain::addressing::SdPath;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Generic progress information that all job types can convert into
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenericProgress {
	/// Current progress as a percentage (0.0 to 1.0)
	pub percentage: f32,

	/// Current phase or stage name (e.g., "Discovery", "Processing", "Finalizing")
	pub phase: String,

	/// Current path being processed (if applicable)
	pub current_path: Option<SdPath>,

	/// Human-readable message describing current activity
	pub message: String,

	/// Completion metrics
	pub completion: ProgressCompletion,

	/// Performance metrics
	pub performance: PerformanceMetrics,

	/// Extended metadata specific to job type
	pub metadata: serde_json::Value,
}

/// Progress completion information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProgressCompletion {
	/// Items completed (files, entries, operations, etc.)
	pub completed: u64,

	/// Total items to complete
	pub total: u64,

	/// Bytes processed (if applicable)
	pub bytes_completed: Option<u64>,

	/// Total bytes to process (if applicable)
	pub total_bytes: Option<u64>,
}

/// Performance and timing metrics
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerformanceMetrics {
	/// Processing rate (items per second)
	pub rate: f32,

	/// Estimated time remaining
	pub estimated_remaining: Option<Duration>,

	/// Time elapsed since start
	pub elapsed: Option<Duration>,

	/// Number of errors encountered
	pub error_count: u64,

	/// Number of warnings
	pub warning_count: u64,
}

impl GenericProgress {
	/// Create a new generic progress instance
	pub fn new(percentage: f32, phase: impl Into<String>, message: impl Into<String>) -> Self {
		Self {
			percentage: percentage.clamp(0.0, 1.0),
			phase: phase.into(),
			current_path: None,
			message: message.into(),
			completion: ProgressCompletion {
				completed: 0,
				total: 0,
				bytes_completed: None,
				total_bytes: None,
			},
			performance: PerformanceMetrics {
				rate: 0.0,
				estimated_remaining: None,
				elapsed: None,
				error_count: 0,
				warning_count: 0,
			},
			metadata: serde_json::Value::Null,
		}
	}

	/// Set the current path being processed
	pub fn with_current_path(mut self, path: SdPath) -> Self {
		self.current_path = Some(path);
		self
	}

	/// Set completion metrics
	pub fn with_completion(mut self, completed: u64, total: u64) -> Self {
		self.completion.completed = completed;
		self.completion.total = total;
		// Auto-calculate percentage if not already set appropriately
		if total > 0 {
			self.percentage = (completed as f32 / total as f32).clamp(0.0, 1.0);
		}
		self
	}

	/// Set byte metrics
	pub fn with_bytes(mut self, bytes_completed: u64, total_bytes: u64) -> Self {
		self.completion.bytes_completed = Some(bytes_completed);
		self.completion.total_bytes = Some(total_bytes);
		self
	}

	/// Set performance metrics
	pub fn with_performance(
		mut self,
		rate: f32,
		estimated_remaining: Option<Duration>,
		elapsed: Option<Duration>,
	) -> Self {
		self.performance.rate = rate;
		self.performance.estimated_remaining = estimated_remaining;
		self.performance.elapsed = elapsed;
		self
	}

	/// Set error and warning counts
	pub fn with_errors(mut self, error_count: u64, warning_count: u64) -> Self {
		self.performance.error_count = error_count;
		self.performance.warning_count = warning_count;
		self
	}

	/// Set job-specific metadata
	pub fn with_metadata<T: Serialize>(mut self, metadata: T) -> Self {
		self.metadata = serde_json::to_value(metadata).unwrap_or(serde_json::Value::Null);
		self
	}

	/// Get a simple percentage (0.0 to 1.0) for basic progress bars
	pub fn as_percentage(&self) -> f32 {
		self.percentage
	}

	/// Get a formatted progress string for display
	pub fn format_progress(&self) -> String {
		format!("{} - {:.1}%", self.message, self.percentage * 100.0)
	}

	/// Get completion ratio as a formatted string
	pub fn format_completion(&self) -> String {
		if self.completion.total > 0 {
			format!("{}/{}", self.completion.completed, self.completion.total)
		} else {
			"Processing...".to_string()
		}
	}

	/// Get bytes progress as a formatted string
	pub fn format_bytes(&self) -> Option<String> {
		match (self.completion.bytes_completed, self.completion.total_bytes) {
			(Some(completed), Some(total)) => Some(format!(
				"{}/{}",
				format_bytes(completed),
				format_bytes(total)
			)),
			_ => None,
		}
	}

	/// Get processing rate as a formatted string
	pub fn format_rate(&self) -> String {
		if self.performance.rate > 0.0 {
			format!("{:.1} items/sec", self.performance.rate)
		} else {
			"Calculating...".to_string()
		}
	}

	/// Get estimated remaining time as a formatted string
	pub fn format_eta(&self) -> String {
		match self.performance.estimated_remaining {
			Some(duration) => format_duration(duration),
			None => "Unknown".to_string(),
		}
	}
}

/// Trait for converting job-specific progress into generic progress
pub trait ToGenericProgress {
	/// Convert this progress type into a GenericProgress
	fn to_generic_progress(&self) -> GenericProgress;
}

// Helper function to format bytes
fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
	let mut size = bytes as f64;
	let mut unit_idx = 0;

	while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
		size /= 1024.0;
		unit_idx += 1;
	}

	if unit_idx == 0 {
		format!("{} {}", size as u64, UNITS[unit_idx])
	} else {
		format!("{:.2} {}", size, UNITS[unit_idx])
	}
}

// Helper function to format duration
fn format_duration(duration: Duration) -> String {
	let secs = duration.as_secs();
	let hours = secs / 3600;
	let mins = (secs % 3600) / 60;
	let secs = secs % 60;

	if hours > 0 {
		format!("{}h {}m {}s", hours, mins, secs)
	} else if mins > 0 {
		format!("{}m {}s", mins, secs)
	} else {
		format!("{}s", secs)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_generic_progress_creation() {
		let progress = GenericProgress::new(0.5, "Processing", "Processing files")
			.with_completion(50, 100)
			.with_performance(
				10.5,
				Some(Duration::from_secs(30)),
				Some(Duration::from_secs(60)),
			);

		assert_eq!(progress.percentage, 0.5);
		assert_eq!(progress.phase, "Processing");
		assert_eq!(progress.completion.completed, 50);
		assert_eq!(progress.completion.total, 100);
	}

	#[test]
	fn test_auto_percentage_calculation() {
		let progress = GenericProgress::new(0.0, "Test", "Testing").with_completion(25, 100);

		assert_eq!(progress.percentage, 0.25);
	}

	#[test]
	fn test_formatting() {
		let progress = GenericProgress::new(0.75, "Test", "Testing files")
			.with_completion(75, 100)
			.with_bytes(1024 * 1024 * 500, 1024 * 1024 * 1000); // 500MB / 1000MB

		assert_eq!(progress.format_progress(), "Testing files - 75.0%");
		assert_eq!(progress.format_completion(), "75/100");
		assert_eq!(
			progress.format_bytes(),
			Some("500.00 MB/1000.00 MB".to_string())
		);
	}
}
