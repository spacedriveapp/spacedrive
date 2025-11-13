//! Thumbnail job state management

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Phases of thumbnail generation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailPhase {
	Discovery,
	Processing,
	Cleanup,
	Complete,
}

/// Entry information for thumbnail generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailEntry {
	pub entry_id: Uuid,
	pub content_uuid: Uuid,
	pub content_kind_id: i32,
	pub extension: Option<String>,
	pub file_size: u64,
	pub relative_path: String,
}

/// Statistics for thumbnail generation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThumbnailStats {
	pub discovered_count: u64,
	pub generated_count: u64,
	pub skipped_count: u64,
	pub error_count: u64,
	pub total_size_bytes: u64,
	pub thumbnails_size_bytes: u64,
}

/// State for thumbnail generation job
#[derive(Debug, Clone, Serialize)]
pub struct ThumbnailState {
	pub phase: ThumbnailPhase,
	pub stats: ThumbnailStats,
	pub pending_entries: Vec<ThumbnailEntry>,
	pub batches: Vec<Vec<ThumbnailEntry>>,
	pub current_batch_index: usize,
	pub errors: Vec<String>,

	#[serde(skip)]
	pub started_at: Instant,
}

impl ThumbnailState {
	pub fn new() -> Self {
		Self {
			phase: ThumbnailPhase::Discovery,
			stats: ThumbnailStats::default(),
			pending_entries: Vec::new(),
			batches: Vec::new(),
			current_batch_index: 0,
			errors: Vec::new(),
			started_at: Instant::now(),
		}
	}

	pub fn add_error(&mut self, error: String) {
		self.errors.push(error);
		self.stats.error_count += 1;
	}

	pub fn record_generated(&mut self, thumbnail_size: u64) {
		self.stats.generated_count += 1;
		self.stats.thumbnails_size_bytes += thumbnail_size;
	}

	pub fn record_skipped(&mut self) {
		self.stats.skipped_count += 1;
	}

	pub fn total_processed(&self) -> u64 {
		self.stats.generated_count + self.stats.skipped_count + self.stats.error_count
	}

	pub fn progress_percentage(&self) -> f32 {
		if self.stats.discovered_count == 0 {
			return 0.0;
		}
		(self.total_processed() as f32 / self.stats.discovered_count as f32) * 100.0
	}
}

impl Default for ThumbnailState {
	fn default() -> Self {
		Self::new()
	}
}

// Custom deserialization to handle Instant
impl<'de> Deserialize<'de> for ThumbnailState {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct ThumbnailStateHelper {
			phase: ThumbnailPhase,
			stats: ThumbnailStats,
			pending_entries: Vec<ThumbnailEntry>,
			batches: Vec<Vec<ThumbnailEntry>>,
			current_batch_index: usize,
			errors: Vec<String>,
		}

		let helper = ThumbnailStateHelper::deserialize(deserializer)?;
		Ok(Self {
			phase: helper.phase,
			stats: helper.stats,
			pending_entries: helper.pending_entries,
			batches: helper.batches,
			current_batch_index: helper.current_batch_index,
			errors: helper.errors,
			started_at: Instant::now(), // Reset to current time on deserialization
		})
	}
}
