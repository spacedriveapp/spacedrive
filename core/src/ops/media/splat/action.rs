//! Gaussian splat action handlers

use super::{
	job::{GaussianSplatJob, GaussianSplatJobConfig},
	processor::GaussianSplatProcessor,
};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	ops::indexing::{path_resolver::PathResolver, processor::ProcessorEntry},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerateSplatInput {
	pub entry_uuid: Uuid,
	pub model_path: Option<String>, // Path to SHARP model checkpoint or None for auto-download
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerateSplatOutput {
	/// Job ID for tracking splat generation progress
	pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateSplatAction {
	input: GenerateSplatInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GaussianSplatJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl GenerateSplatAction {
	pub fn new(input: GenerateSplatInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for GenerateSplatAction {
	type Input = GenerateSplatInput;
	type Output = GenerateSplatOutput;

	fn from_input(input: GenerateSplatInput) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		tracing::info!(
			"Dispatching Gaussian splat job for entry: {}",
			self.input.entry_uuid
		);

		// Create job config for single file
		let job_config = super::job::GaussianSplatJobConfig {
			location_id: None,
			entry_uuid: Some(self.input.entry_uuid), // Single file mode
			model_path: self.input.model_path,
			reprocess: false,
		};

		// Create job
		let job = super::job::GaussianSplatJob::new(job_config);

		tracing::info!("Gaussian splat job created: {:?}", self.input.entry_uuid);

		// Dispatch job
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to dispatch job: {}", e)))?;

		tracing::info!("Gaussian splat job dispatched: {}", job_handle.id());

		Ok(GenerateSplatOutput {
			job_id: job_handle.id().to_string(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"media.splat.generate"
	}
}

crate::register_library_action!(GenerateSplatAction, "media.splat.generate");
