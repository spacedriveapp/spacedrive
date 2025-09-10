//! Thumbnail generation action handler

use super::job::{ThumbnailJob, ThumbnailJobConfig};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailInput {
	pub paths: Vec<std::path::PathBuf>,
	pub size: u32,
	pub quality: u8,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailAction {
	input: ThumbnailInput,
}

impl ThumbnailAction {
	/// Create a new thumbnail generation action
	pub fn new(input: ThumbnailInput) -> Self {
		Self { input }
	}
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for ThumbnailAction {
	type Input = ThumbnailInput;
	type Output = JobHandle;

	fn from_input(input: ThumbnailInput) -> Result<Self, String> {
		Ok(ThumbnailAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Create thumbnail job config
		let config = ThumbnailJobConfig {
			sizes: vec![self.input.size],
			quality: self.input.quality,
			regenerate: false,
			..Default::default()
		};

		// Create job instance directly
		let job = ThumbnailJob::new(config);

		// Dispatch job and return handle directly
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"media.thumbnail"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<(), ActionError> {
		// Validate paths
		if self.input.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: "At least one path must be specified".to_string(),
			});
		}

		Ok(())
	}
}
