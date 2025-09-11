//! Input types for file deletion operations

use super::action::FileDeleteAction;
use crate::domain::SdPathBatch;
use serde::{Deserialize, Serialize};

/// Input for deleting files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteInput {
	/// Files or directories to delete
	pub targets: SdPathBatch,

	/// Whether to permanently delete (true) or move to trash (false)
	pub permanent: bool,

	/// Whether to delete directories recursively
	pub recursive: bool,
}

impl FileDeleteInput {
	/// Create a new file deletion input
	pub fn new(targets: SdPathBatch) -> Self {
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

		if self.targets.paths.is_empty() {
			errors.push("At least one target file must be specified".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
