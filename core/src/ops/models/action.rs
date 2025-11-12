//! Model management actions

use super::{download::ModelDownloadJob, whisper::WhisperModel};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

// ============================================================================
// Download Whisper Model Action
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadWhisperModelInput {
	/// Model size: "tiny", "base", "small", "medium", "large"
	pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadWhisperModelOutput {
	/// Job ID for tracking download progress
	pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadWhisperModelAction {
	input: DownloadWhisperModelInput,
}

impl DownloadWhisperModelAction {
	pub fn new(input: DownloadWhisperModelInput) -> Self {
		Self { input }
	}
}

impl CoreAction for DownloadWhisperModelAction {
	type Input = DownloadWhisperModelInput;
	type Output = DownloadWhisperModelOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Parse model
		let model = WhisperModel::from_str(&self.input.model).ok_or_else(|| {
			ActionError::InvalidInput(format!("Invalid model name: {}", self.input.model))
		})?;

		// Get data directory
		let data_dir = crate::config::default_data_dir()
			.map_err(|e| ActionError::Internal(format!("Failed to get data dir: {}", e)))?;

		// Create download job
		let job = ModelDownloadJob::for_whisper_model(model, data_dir);

		// Get the first library to dispatch the job
		// TODO: Model downloads should be core-level jobs, not library-level
		let library = context
			.get_primary_library()
			.await
			.ok_or_else(|| ActionError::Internal("No library available".to_string()))?;

		// Dispatch job
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to dispatch job: {}", e)))?;

		Ok(DownloadWhisperModelOutput {
			job_id: job_handle.id().to_string(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"models.whisper.download"
	}
}

crate::register_core_action!(DownloadWhisperModelAction, "models.whisper.download");

// ============================================================================
// Delete Whisper Model Action
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteWhisperModelInput {
	pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteWhisperModelOutput {
	pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteWhisperModelAction {
	input: DeleteWhisperModelInput,
}

impl CoreAction for DeleteWhisperModelAction {
	type Input = DeleteWhisperModelInput;
	type Output = DeleteWhisperModelOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let model = WhisperModel::from_str(&self.input.model).ok_or_else(|| {
			ActionError::InvalidInput(format!("Invalid model name: {}", self.input.model))
		})?;

		let data_dir = crate::config::default_data_dir()
			.map_err(|e| ActionError::Internal(format!("Failed to get data dir: {}", e)))?;

		let manager = super::whisper::WhisperModelManager::new(&data_dir);

		manager
			.delete_model(&model)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to delete model: {}", e)))?;

		Ok(DeleteWhisperModelOutput { deleted: true })
	}

	fn action_kind(&self) -> &'static str {
		"models.whisper.delete"
	}
}

crate::register_core_action!(DeleteWhisperModelAction, "models.whisper.delete");
