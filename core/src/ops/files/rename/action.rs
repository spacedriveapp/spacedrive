//! File rename action handler

use super::input::FileRenameInput;
use super::validation::validate_filename;
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction, ValidationResult},
		job::handle::JobReceipt,
	},
	ops::files::copy::job::FileCopyJob,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Action for renaming a file or directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRenameAction {
	/// The file or directory to rename
	pub target: SdPath,
	/// The new name (filename only, no path)
	pub new_name: String,
}

impl FileRenameAction {
	/// Create a new rename action
	pub fn new(target: SdPath, new_name: impl Into<String>) -> Self {
		Self {
			target,
			new_name: new_name.into(),
		}
	}
}

impl LibraryAction for FileRenameAction {
	type Input = FileRenameInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(FileRenameAction {
			target: input.target,
			new_name: input.new_name,
		})
	}

	async fn validate(
		&self,
		_library: &Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		// Validate the new filename
		validate_filename(&self.new_name).map_err(|e| ActionError::Validation {
			field: "new_name".to_string(),
			message: e.to_string(),
		})?;

		// Validate target is not a Content or Sidecar path (these cannot be renamed directly)
		match &self.target {
			SdPath::Content { .. } => {
				return Err(ActionError::Validation {
					field: "target".to_string(),
					message: "Cannot rename content-addressed files directly".to_string(),
				});
			}
			SdPath::Sidecar { .. } => {
				return Err(ActionError::Validation {
					field: "target".to_string(),
					message: "Cannot rename sidecar files directly".to_string(),
				});
			}
			_ => {}
		}

		Ok(ValidationResult::Success)
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Use FileCopyJob::new_rename which handles the rename as a move operation
		let job = FileCopyJob::new_rename(self.target, self.new_name);

		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.rename"
	}
}

// Register with the action-centric registry
crate::register_library_action!(FileRenameAction, "files.rename");

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_action_creation() {
		let target = SdPath::local(std::path::PathBuf::from("/test/file.txt"));
		let action = FileRenameAction::new(target, "newname.txt");
		assert_eq!(action.new_name, "newname.txt");
	}
}
