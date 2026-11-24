//! Model management actions

use super::{download::ModelDownloadJob, whisper::WhisperModel};
use crate::{
	context::CoreContext,
	infra::{
		action::{context::ActionContext, error::ActionError, CoreAction, LibraryAction},
		job::types::JobPriority,
	},
	library::Library,
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

/// Runs in the caller's library, so the client that started a download sees
/// its job in that library's job list and can cancel it.
impl LibraryAction for DownloadWhisperModelAction {
	type Input = DownloadWhisperModelInput;
	type Output = DownloadWhisperModelOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Parse model
		let model = WhisperModel::from_str(&self.input.model).ok_or_else(|| {
			ActionError::InvalidInput(format!("Invalid model name: {}", self.input.model))
		})?;

		let job = ModelDownloadJob::for_whisper_model(model, context.data_dir.clone());

		// The model id rides on the job so a client can match a running
		// download to its model after a restart.
		let action_context = ActionContext::new(
			"models.whisper.download",
			serde_json::json!({ "model_id": model.id() }),
			serde_json::json!({ "size_bytes": model.size_bytes() }),
		);
		let job_handle = library
			.jobs()
			.dispatch_with_priority(job, JobPriority::NORMAL, Some(action_context))
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

crate::register_library_action!(DownloadWhisperModelAction, "models.whisper.download");

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

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let model = WhisperModel::from_str(&self.input.model).ok_or_else(|| {
			ActionError::InvalidInput(format!("Invalid model name: {}", self.input.model))
		})?;

		let manager = super::whisper::WhisperModelManager::new(&context.data_dir);

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
