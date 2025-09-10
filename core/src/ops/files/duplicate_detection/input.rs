//! File duplicate detection input for external API

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

impl crate::client::Wire for DuplicateDetectionInput {
	const METHOD: &'static str = "action:files.duplicate_detection.input.v1";
}

impl crate::ops::registry::BuildLibraryActionInput for DuplicateDetectionInput {
	type Action = crate::ops::files::duplicate_detection::action::DuplicateDetectionAction;

	fn build(self) -> Result<Self::Action, String> {
		Ok(
			crate::ops::files::duplicate_detection::action::DuplicateDetectionAction::new(
				self.paths,
				self.algorithm,
				self.threshold,
			),
		)
	}
}

use crate::register_library_action_input;
register_library_action_input!(DuplicateDetectionInput);
