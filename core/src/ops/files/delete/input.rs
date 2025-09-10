//! Input types for file deletion operations

use crate::register_library_action_input;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Input for deleting files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteInput {
	/// Files or directories to delete
	pub targets: Vec<PathBuf>,

	/// Whether to permanently delete (true) or move to trash (false)
	pub permanent: bool,

	/// Whether to delete directories recursively
	pub recursive: bool,
}

impl crate::client::Wire for FileDeleteInput {
	const METHOD: &'static str = "action:files.delete.input.v1";
}

impl crate::ops::registry::BuildLibraryActionInput for FileDeleteInput {
	type Action = crate::ops::files::delete::action::FileDeleteAction;

	fn build(self) -> Result<Self::Action, String> {
		use crate::ops::files::delete::job::DeleteOptions;

		Ok(crate::ops::files::delete::action::FileDeleteAction {
			targets: self.targets,
			options: DeleteOptions {
				permanent: self.permanent,
				recursive: self.recursive,
			},
		})
	}
}

register_library_action_input!(FileDeleteInput);

impl FileDeleteInput {
	/// Create a new file deletion input
	pub fn new(targets: Vec<PathBuf>) -> Self {
		Self {
			targets,
			permanent: false,
			recursive: true,
		}
	}

	/// Set permanent deletion
	pub fn with_permanent(mut self, permanent: bool) -> Self {
		self.permanent = permanent;
		self
	}

	/// Set recursive deletion
	pub fn with_recursive(mut self, recursive: bool) -> Self {
		self.recursive = recursive;
		self
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if self.targets.is_empty() {
			errors.push("At least one target file must be specified".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
