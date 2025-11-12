//! OCR action handlers

use super::{
	job::{OcrJob, OcrJobConfig},
	processor::OcrProcessor,
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

// ============================================================================
// Extract Text Action (for single file UI triggering)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtractTextInput {
	/// UUID of the entry to extract text from
	pub entry_uuid: Uuid,
	/// Languages to use for OCR (e.g., ["eng", "spa"])
	pub languages: Option<Vec<String>>,
	/// Force re-extraction even if text exists
	pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtractTextOutput {
	/// Job ID for tracking OCR progress
	pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractTextAction {
	input: ExtractTextInput,
}

impl ExtractTextAction {
	pub fn new(input: ExtractTextInput) -> Self {
		Self { input }
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OcrJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl LibraryAction for ExtractTextAction {
	type Input = ExtractTextInput;
	type Output = ExtractTextOutput;

	fn from_input(input: ExtractTextInput) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		tracing::info!("Dispatching OCR job for entry: {}", self.input.entry_uuid);

		// Create job config for single file
		let languages = self.input.languages.unwrap_or_else(|| vec!["eng".to_string()]);

		let job_config = super::job::OcrJobConfig {
			location_id: None,
			entry_uuid: Some(self.input.entry_uuid),
			languages,
			min_confidence: 0.6,
			reprocess: self.input.force,
		};

		// Create and dispatch job
		let job = super::job::OcrJob::new(job_config);

		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to dispatch job: {}", e)))?;

		tracing::info!("OCR job dispatched: {}", job_handle.id());

		Ok(ExtractTextOutput {
			job_id: job_handle.id().to_string(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"media.ocr.extract"
	}
}

crate::register_library_action!(ExtractTextAction, "media.ocr.extract");
