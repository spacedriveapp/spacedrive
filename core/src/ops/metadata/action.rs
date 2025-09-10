//! Metadata operations action handler

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
pub struct MetadataAction {
	pub library_id: uuid::Uuid,
	pub paths: Vec<std::path::PathBuf>,
	pub extract_exif: bool,
	pub extract_xmp: bool,
}

impl MetadataAction {
	/// Create a new metadata extraction action
	pub fn new(
		library_id: uuid::Uuid,
		paths: Vec<std::path::PathBuf>,
		extract_exif: bool,
		extract_xmp: bool,
	) -> Self {
		Self {
			library_id,
			paths,
			extract_exif,
			extract_xmp,
		}
	}
}

// Old ActionHandler implementation removed - using unified LibraryAction

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for MetadataAction {
	type Output = JobHandle;

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Create metadata extraction job
		let job_params = serde_json::json!({
			"paths": self.paths,
			"extract_exif": self.extract_exif,
			"extract_xmp": self.extract_xmp,
		});

		// Dispatch job and return handle
		let job_handle = library
			.jobs()
			.dispatch_by_name("extract_metadata", job_params)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"metadata.extract"
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
