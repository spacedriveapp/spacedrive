//! Performance metrics and monitoring for the indexer

use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::{Duration, Instant};

/// Comprehensive metrics for indexing operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerMetrics {
	// Timing
	pub total_duration: Duration,
	pub discovery_duration: Duration,
	pub processing_duration: Duration,
	pub content_duration: Duration,

	// Throughput
	pub files_per_second: f32,
	pub bytes_per_second: f64,
	pub dirs_per_second: f32,

	// Database operations
	pub db_writes: u64,
	pub db_reads: u64,
	pub batch_count: u64,
	pub avg_batch_size: f32,

	// Error tracking
	pub total_errors: u64,
	pub critical_errors: u64,
	pub non_critical_errors: u64,
	pub skipped_paths: u64,

	// Memory usage (if available)
	pub peak_memory_bytes: Option<u64>,
	pub avg_memory_bytes: Option<u64>,
}

impl Default for IndexerMetrics {
	fn default() -> Self {
		Self {
			total_duration: Duration::default(),
			discovery_duration: Duration::default(),
			processing_duration: Duration::default(),
			content_duration: Duration::default(),
			files_per_second: 0.0,
			bytes_per_second: 0.0,
			dirs_per_second: 0.0,
			db_writes: 0,
			db_reads: 0,
			batch_count: 0,
			avg_batch_size: 0.0,
			total_errors: 0,
			critical_errors: 0,
			non_critical_errors: 0,
			skipped_paths: 0,
			peak_memory_bytes: None,
			avg_memory_bytes: None,
		}
	}
}

/// Tracks timing for different phases
#[derive(Debug)]
pub struct PhaseTimer {
	phase_start: Instant,
	discovery_start: Option<Instant>,
	processing_start: Option<Instant>,
	content_start: Option<Instant>,
}

impl PhaseTimer {
	pub fn new() -> Self {
		Self {
			phase_start: Instant::now(),
			discovery_start: Some(Instant::now()),
			processing_start: None,
			content_start: None,
		}
	}

	pub fn start_processing(&mut self) {
		self.processing_start = Some(Instant::now());
	}

	pub fn start_content(&mut self) {
		self.content_start = Some(Instant::now());
	}

	pub fn get_durations(&self) -> (Duration, Duration, Duration, Duration) {
		let total = self.phase_start.elapsed();

		let discovery = self
			.discovery_start
			.and_then(|start| self.processing_start.map(|_| start.elapsed()))
			.unwrap_or_default();

		let processing = self
			.processing_start
			.and_then(|start| self.content_start.map(|_| start.elapsed()))
			.unwrap_or_default();

		let content = self
			.content_start
			.map(|start| start.elapsed())
			.unwrap_or_default();

		(total, discovery, processing, content)
	}
}

impl IndexerMetrics {
	/// Calculate final metrics from state and timer
	pub fn calculate(
		stats: &super::state::IndexerStats,
		timer: &PhaseTimer,
		db_operations: (u64, u64), // (reads, writes)
		batch_info: (u64, usize),  // (count, total_size)
	) -> Self {
		let (total, discovery, processing, content) = timer.get_durations();

		let total_secs = total.as_secs_f32();
		let (db_reads, db_writes) = db_operations;
		let (batch_count, total_batch_size) = batch_info;

		Self {
			total_duration: total,
			discovery_duration: discovery,
			processing_duration: processing,
			content_duration: content,

			files_per_second: if total_secs > 0.0 {
				stats.files as f32 / total_secs
			} else {
				0.0
			},
			bytes_per_second: if total_secs > 0.0 {
				stats.bytes as f64 / total_secs as f64
			} else {
				0.0
			},
			dirs_per_second: if total_secs > 0.0 {
				stats.dirs as f32 / total_secs
			} else {
				0.0
			},

			db_writes,
			db_reads,
			batch_count,
			avg_batch_size: if batch_count > 0 {
				total_batch_size as f32 / batch_count as f32
			} else {
				0.0
			},

			total_errors: stats.errors,
			critical_errors: 0, // TODO: Track separately
			non_critical_errors: stats.errors,
			skipped_paths: stats.skipped,

			peak_memory_bytes: None, // TODO: Implement memory tracking
			avg_memory_bytes: None,
		}
	}

	/// Format metrics for logging
	pub fn format_summary(&self) -> String {
		format!(
			"Indexing completed in {:.2}s:\n\
             - Files: {} ({:.1}/s)\n\
             - Directories: {} ({:.1}/s)\n\
             - Total size: {:.2} GB ({:.2} MB/s)\n\
             - Database writes: {} in {} batches (avg {:.1} items/batch)\n\
             - Errors: {} (skipped {} paths)\n\
             - Phase timing: discovery {:.1}s, processing {:.1}s, content {:.1}s",
			self.total_duration.as_secs_f32(),
			self.files_per_second * self.total_duration.as_secs_f32(),
			self.files_per_second,
			self.dirs_per_second * self.total_duration.as_secs_f32(),
			self.dirs_per_second,
			self.bytes_per_second * self.total_duration.as_secs_f64() / 1_073_741_824.0,
			self.bytes_per_second / 1_048_576.0,
			self.db_writes,
			self.batch_count,
			self.avg_batch_size,
			self.total_errors,
			self.skipped_paths,
			self.discovery_duration.as_secs_f32(),
			self.processing_duration.as_secs_f32(),
			self.content_duration.as_secs_f32(),
		)
	}
}
