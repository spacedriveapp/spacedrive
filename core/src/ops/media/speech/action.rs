//! Speech-to-text action handlers

use super::{
	job::{SpeechToTextJob, SpeechToTextJobConfig},
	processor::SpeechToTextProcessor,
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
pub struct TranscribeAudioInput {
	pub entry_uuid: Uuid,
	pub model: Option<String>, // whisper model (tiny, base, small, medium, large)
	pub language: Option<String>, // Language code or None for auto-detect
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscribeAudioOutput {
	/// Job ID for tracking transcription progress
	pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeAudioAction {
	input: TranscribeAudioInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpeechToTextJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl TranscribeAudioAction {
	pub fn new(input: TranscribeAudioInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for TranscribeAudioAction {
	type Input = TranscribeAudioInput;
	type Output = TranscribeAudioOutput;

	fn from_input(input: TranscribeAudioInput) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		tracing::info!(
			"Dispatching speech-to-text job for entry: {}",
			self.input.entry_uuid
		);

		// Create job config for single file
		let model_name = self.input.model.unwrap_or_else(|| "base".to_string());

		let job_config = super::job::SpeechToTextJobConfig {
			location_id: None,
			entry_uuid: Some(self.input.entry_uuid), // Single file mode
			model: model_name,
			language: self.input.language,
			reprocess: false,
		};

		// Create job
		let job = super::job::SpeechToTextJob::new(job_config);

		// Dispatch job - it will handle model download in discovery phase
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to dispatch job: {}", e)))?;

		tracing::info!("Speech-to-text job dispatched: {}", job_handle.id());

		Ok(TranscribeAudioOutput {
			job_id: job_handle.id().to_string(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"media.speech.transcribe"
	}
}

crate::register_library_action!(TranscribeAudioAction, "media.speech.transcribe");
