//! Proxy job state structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Job execution phase
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProxyPhase {
	Discovery,
	Processing,
	Complete,
}

/// Job state for batch proxy generation
#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyState {
	pub phase: ProxyPhase,
	/// Entries to process: (entry_id, path, mime_type)
	pub entries: Vec<(i32, PathBuf, Option<String>)>,
	pub processed: usize,
	pub success_count: usize,
	pub error_count: usize,
	pub total_encoding_time_secs: u64,
}

impl ProxyState {
	pub fn new() -> Self {
		Self {
			phase: ProxyPhase::Discovery,
			entries: Vec::new(),
			processed: 0,
			success_count: 0,
			error_count: 0,
			total_encoding_time_secs: 0,
		}
	}
}

impl Default for ProxyState {
	fn default() -> Self {
		Self::new()
	}
}
