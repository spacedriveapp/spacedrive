//! Thumbstrip job state structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Job execution phase
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThumbstripPhase {
	Discovery,
	Processing,
	Complete,
}

/// Job state for batch thumbstrip generation
#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbstripState {
	pub phase: ThumbstripPhase,
	/// Entries to process: (entry_id, path, mime_type)
	pub entries: Vec<(i32, PathBuf, Option<String>)>,
	pub processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl ThumbstripState {
	pub fn new() -> Self {
		Self {
			phase: ThumbstripPhase::Discovery,
			entries: Vec::new(),
			processed: 0,
			success_count: 0,
			error_count: 0,
		}
	}
}

impl Default for ThumbstripState {
	fn default() -> Self {
		Self::new()
	}
}
