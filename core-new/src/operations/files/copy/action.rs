//! File copy action handler

use super::job::{CopyOptions, FileCopyJob};
use crate::{
	context::CoreContext,
	infrastructure::actions::{
		error::{ActionError, ActionResult},
		handler::ActionHandler,
		receipt::ActionReceipt,
		Action,
	},
	register_action_handler,
	shared::types::{SdPath, SdPathBatch},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyAction {
	pub sources: Vec<PathBuf>,
	pub destination: PathBuf,
	pub options: CopyOptions,
}

pub struct FileCopyHandler;

impl FileCopyHandler {
	pub fn new() -> Self {
		Self
	}
}

#[async_trait]
impl ActionHandler for FileCopyHandler {
	async fn validate(&self, _context: Arc<CoreContext>, action: &Action) -> ActionResult<()> {
		if let Action::FileCopy {
			library_id: _,
			action,
		} = action
		{
			if action.sources.is_empty() {
				return Err(ActionError::Validation {
					field: "sources".to_string(),
					message: "At least one source file must be specified".to_string(),
				});
			}

			// Additional validation could include:
			// - Check if source files exist
			// - Check permissions
			// - Check if destination is valid
			// - Check if it would be a cross-device operation

			Ok(())
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	async fn execute(
		&self,
		context: Arc<CoreContext>,
		action: Action,
	) -> ActionResult<ActionReceipt> {
		if let Action::FileCopy { library_id, action } = action {
			let library_manager = &context.library_manager;

			// Get the specific library
			let library = library_manager
				.get_library(library_id)
				.await
				.ok_or(ActionError::LibraryNotFound(library_id))?;

			// Create job instance
			let sources = action
				.sources
				.into_iter()
				.map(|path| SdPath::local(path))
				.collect();

			let job =
				FileCopyJob::new(SdPathBatch::new(sources), SdPath::local(action.destination))
					.with_options(action.options);

			// Dispatch the job
			let job_handle = library
				.jobs()
				.dispatch(job)
				.await
				.map_err(ActionError::Job)?;

			Ok(ActionReceipt::job_based(Uuid::new_v4(), job_handle))
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	fn can_handle(&self, action: &Action) -> bool {
		matches!(action, Action::FileCopy { .. })
	}

	fn supported_actions() -> &'static [&'static str] {
		&["file.copy"]
	}
}

// Register this handler
register_action_handler!(FileCopyHandler, "file.copy");
