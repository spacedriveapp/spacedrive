//! File delete action handler

use super::job::{DeleteJob, DeleteMode, DeleteOptions};
use super::output::FileDeleteOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		error::{ActionError, ActionResult},
		Action,
	},
	register_action_handler,
	domain::addressing::{SdPath, SdPathBatch},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteAction {
	pub library_id: Uuid,
	pub targets: Vec<PathBuf>,
	pub options: DeleteOptions,
}

impl FileDeleteAction {
	/// Create a new file delete action
	pub fn new(library_id: Uuid, targets: Vec<PathBuf>, options: DeleteOptions) -> Self {
		Self {
			library_id,
			targets,
			options,
		}
	}

	/// Create a delete action with default options
	pub fn with_defaults(library_id: Uuid, targets: Vec<PathBuf>) -> Self {
		Self::new(library_id, targets, DeleteOptions::default())
	}
}

pub struct FileDeleteHandler;

impl FileDeleteHandler {
	pub fn new() -> Self {
		Self
	}
}

#[async_trait]
impl ActionHandler for FileDeleteHandler {
	async fn validate(&self, _context: Arc<CoreContext>, action: &Action) -> ActionResult<()> {
		if let Action::FileDelete {
			library_id: _,
			action,
		} = action
		{
			if action.targets.is_empty() {
				return Err(ActionError::Validation {
					field: "targets".to_string(),
					message: "At least one target file must be specified".to_string(),
				});
			}
			Ok(())
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	async fn execute(
		&self,
		context: Arc<CoreContext>,
		action: Action,
	) -> ActionResult<String> {
		if let Action::FileDelete { library_id, action } = action {
			let library_manager = &context.library_manager;

			// Get the specific library
			let library = library_manager
				.get_library(library_id)
				.await
				.ok_or(ActionError::LibraryNotFound(library_id))?;

			// Create job instance directly (no JSON roundtrip)
			let targets_count = action.targets.len();
			let targets = action
				.targets
				.into_iter()
				.map(|path| SdPath::local(path))
				.collect();

			let mode = if action.options.permanent {
				DeleteMode::Permanent
			} else {
				DeleteMode::Trash
			};

			let job = DeleteJob::new(SdPathBatch::new(targets), mode);

			// Dispatch the job directly
			let job_handle = library
				.jobs()
				.dispatch(job)
				.await
				.map_err(ActionError::Job)?;

			// Return action output instead of receipt
			let output = FileDeleteOutput::new(job_handle.id().into(), targets_count);
			Ok("Action completed successfully".to_string())
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	fn can_handle(&self, action: &Action) -> bool {
		matches!(action, Action::FileDelete { .. })
	}

	fn supported_actions() -> &'static [&'static str] {
		&["file.delete"]
	}
}

// Register this handler
register_action_handler!(FileDeleteHandler, "file.delete");

// Implement the unified ActionTrait (replaces ActionHandler)
impl ActionTrait for FileDeleteAction {
	type Output = JobHandle;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Get the specific library
		let library = context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or(ActionError::LibraryNotFound(self.library_id))?;

		// Create job instance directly
		let targets = self
			.targets
			.into_iter()
			.map(|path| SdPath::local(path))
			.collect();

		let mode = if self.options.permanent {
			DeleteMode::Permanent
		} else {
			DeleteMode::Trash
		};

		let job = DeleteJob::new(SdPathBatch::new(targets), mode);

		// Dispatch job and return handle directly
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"file.delete"
	}

	fn library_id(&self) -> Option<Uuid> {
		Some(self.library_id)
	}

	async fn validate(&self, context: Arc<CoreContext>) -> Result<(), ActionError> {
		// Validate library exists
		let _library = context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or_else(|| ActionError::LibraryNotFound(self.library_id))?;

		// Validate targets
		if self.targets.is_empty() {
			return Err(ActionError::Validation {
				field: "targets".to_string(),
				message: "At least one target file must be specified".to_string(),
			});
		}

		Ok(())
	}
}
