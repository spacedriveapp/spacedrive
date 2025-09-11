//! File delete action handler

use super::input::FileDeleteInput;
use super::job::{DeleteJob, DeleteMode, DeleteOptions};
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteAction {
	pub targets: SdPathBatch,
	pub options: DeleteOptions,
}

impl FileDeleteAction {
	/// Create a new file delete action
	pub fn new(targets: SdPathBatch, options: DeleteOptions) -> Self {
		Self { targets, options }
	}

	/// Create a delete action with default options
	pub fn with_defaults(targets: SdPathBatch) -> Self {
		Self::new(targets, DeleteOptions::default())
	}
}

// Implement the unified LibraryAction
impl LibraryAction for FileDeleteAction {
	type Input = FileDeleteInput;
	type Output = JobHandle;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(FileDeleteAction {
			targets: input.targets,
			options: DeleteOptions {
				permanent: input.permanent,
				recursive: input.recursive,
			},
		})
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let mode = if self.options.permanent {
			DeleteMode::Permanent
		} else {
			DeleteMode::Trash
		};

		let job = DeleteJob::new(self.targets, mode);

		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"files.delete"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<(), ActionError> {
		// Validate targets
		if self.targets.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "targets".to_string(),
				message: "At least one target file must be specified".to_string(),
			});
		}

		Ok(())
	}
}

// Register this action with the new registry
crate::register_library_action!(FileDeleteAction, "files.delete");
