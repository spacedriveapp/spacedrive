//! Thumbnail generation action handler

use super::job::{ThumbnailJob, ThumbnailJobConfig};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailAction {
	pub library_id: uuid::Uuid,
	pub paths: Vec<std::path::PathBuf>,
	pub size: u32,
	pub quality: u8,
}

impl ThumbnailAction {
	/// Create a new thumbnail generation action
	pub fn new(
		library_id: uuid::Uuid,
		paths: Vec<std::path::PathBuf>,
		size: u32,
		quality: u8,
	) -> Self {
		Self {
			library_id,
			paths,
			size,
			quality,
		}
	}
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for ThumbnailAction {
	type Output = JobHandle;

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Library is pre-validated by ActionManager - no boilerplate!

		// Create thumbnail job config
		let config = ThumbnailJobConfig {
			sizes: vec![self.size],
			quality: self.quality,
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
		// Library existence already validated by ActionManager - no boilerplate!

		// Validate paths
		if self.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: "At least one path must be specified".to_string(),
			});
		}

		Ok(())
	}
}
// Old ActionHandler implementation removed - using unified LibraryAction
