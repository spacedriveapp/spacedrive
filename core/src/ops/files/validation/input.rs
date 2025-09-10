//! File validation input for external API

use crate::register_library_action_input;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Input for file validation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileValidationInput {
	/// Paths to validate
	pub paths: Vec<PathBuf>,
	/// Whether to verify file checksums
	pub verify_checksums: bool,
	/// Whether to perform deep scanning
	pub deep_scan: bool,
}

impl crate::client::Wire for FileValidationInput {
	const METHOD: &'static str = "action:files.validation.input.v1";
}

impl crate::ops::registry::BuildLibraryActionInput for FileValidationInput {
	type Action = crate::ops::files::validation::action::ValidationAction;

	fn build(self) -> Result<Self::Action, String> {
		Ok(
			crate::ops::files::validation::action::ValidationAction::new(
				self.paths,
				self.verify_checksums,
				self.deep_scan,
			),
		)
	}
}

register_library_action_input!(FileValidationInput);
