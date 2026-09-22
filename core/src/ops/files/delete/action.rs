//! File delete action handler

use super::input::{DeleteTargets, FileDeleteInput};
use super::job::{DeleteJob, DeleteMode, DeleteOptions};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteAction {
	pub targets: DeleteTargets,
	pub options: DeleteOptions,
}

impl LibraryAction for FileDeleteAction {
	type Input = FileDeleteInput;
	type Output = crate::infra::job::handle::JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		input.validate().map_err(|errors| errors.join("; "))?;
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
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let mode = if self.options.permanent {
			DeleteMode::Permanent
		} else {
			DeleteMode::Trash
		};

		let job_handle = library
			.jobs()
			.dispatch(DeleteJob::new(self.targets, mode))
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.delete"
	}
}

crate::register_library_action!(FileDeleteAction, "files.delete");
