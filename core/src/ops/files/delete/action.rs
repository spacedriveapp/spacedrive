//! File delete action handler

use super::job::{DeleteJob, DeleteMode, DeleteOptions};
use super::output::FileDeleteOutput;
use crate::{
	context::CoreContext,
	infra::{
		action::{
			error::ActionError,
			LibraryAction,
		},
		job::handle::JobHandle,
	},
	domain::addressing::{SdPath, SdPathBatch},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteAction {
	pub targets: Vec<PathBuf>,
	pub options: DeleteOptions,
}

impl FileDeleteAction {
	/// Create a new file delete action
	pub fn new(targets: Vec<PathBuf>, options: DeleteOptions) -> Self {
		Self { targets, options }
	}

	/// Create a delete action with default options
	pub fn with_defaults(targets: Vec<PathBuf>) -> Self {
		Self::new(targets, DeleteOptions::default())
	}
}

// Old ActionHandler implementation removed - using unified LibraryAction

// Implement the unified ActionTrait (replaces ActionHandler)
impl LibraryAction for FileDeleteAction {
	type Output = JobHandle;

	async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Library is pre-validated by ActionManager - no boilerplate!

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

	async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
		// Library existence already validated by ActionManager - no boilerplate!

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
