//! File duplicate detection input for external API

use super::action::DuplicateDetectionAction;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::op;

/// Input for file duplicate detection operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateDetectionInput {
	/// Paths to search for duplicates
	pub paths: Vec<PathBuf>,
	/// Detection algorithm to use
	pub algorithm: String,
	/// Similarity threshold (0.0 to 1.0)
	pub threshold: f64,
}

op!(library_action DuplicateDetectionInput => DuplicateDetectionAction, "files.duplicate_detection");
