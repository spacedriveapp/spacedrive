//! File validation action handler

use super::job::{ValidationJob, ValidationMode};
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
	ops::files::FileValidationInput,
};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationAction {
	pub targets: SdPathBatch,
	pub verify_checksums: bool,
	pub deep_scan: bool,
}

impl ValidationAction {
	/// Create a new file validation action
	pub fn new(targets: SdPathBatch, verify_checksums: bool, deep_scan: bool) -> Self {
		Self {
			targets,
			verify_checksums,
			deep_scan,
		}
	}
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for ValidationAction {
	type Input = FileValidationInput;
	type Output = JobHandle;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		let paths = input
			.paths
			.into_iter()
			.map(|p| SdPath::local(p))
			.collect::<Vec<_>>();
		Ok(ValidationAction {
			targets: SdPathBatch { paths },
			verify_checksums: input.verify_checksums,
			deep_scan: input.deep_scan,
		})
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Create validation job
		let mode = if self.deep_scan {
			ValidationMode::Complete
		} else if self.verify_checksums {
			ValidationMode::Integrity
		} else {
			ValidationMode::Basic
		};

		let job = ValidationJob::new(self.targets, mode);

		// Dispatch job and return handle directly
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"file.validate"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), ActionError> {
		// Validate paths
		if self.targets.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: "At least one path must be specified".to_string(),
			});
		}

		Ok(())
	}
}

// Register this action with the new registry
crate::register_library_action!(ValidationAction, "files.validation");
