//! # Transcribe action
//!
//! `media.transcribe`: queue transcription of the audio and video files at
//! or under a path.

use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::{
	job::{model_name, TranscribeJob},
	transcript::MEDIA_EXTENSIONS,
};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::types::JobPriority,
	},
	library::Library,
	ops::{
		models::whisper::{WhisperModel, WhisperModelManager},
		sources::registry,
	},
};

/// What to transcribe, and how.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscribeInput {
	/// A file or directory under a source tracked on this device.
	pub path: PathBuf,
	/// A Whisper model: tiny, base, small, medium or large. Defaults to the
	/// most accurate downloaded model up to small, then medium or large.
	#[serde(default)]
	pub model: Option<String>,
	/// ISO 639-1 code such as "en" to skip detection.
	#[serde(default)]
	pub language: Option<String>,
	/// Make transcripts again even when one exists for the same bytes.
	#[serde(default)]
	pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscribeActionOutput {
	pub job_id: Uuid,
	/// The model the job runs.
	pub model: String,
	/// Audio and video files under the path without a current transcript.
	pub pending_files: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeAction {
	input: TranscribeInput,
}

impl LibraryAction for TranscribeAction {
	type Input = TranscribeInput;
	type Output = TranscribeActionOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if let Some(language) = &input.language {
			if language != "auto" && whisper_rs::get_lang_id(language).is_none() {
				return Err(format!("unknown language code '{language}'"));
			}
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let index = context.volume_index();
		let path = &self.input.path;
		let not_tracked = || {
			ActionError::InvalidInput(format!(
				"{} is not under a source tracked on this device",
				path.display()
			))
		};
		let source_id = index.source_id_for(path).ok_or_else(not_tracked)?;
		registry::get(library.db().conn(), source_id)
			.await
			.map_err(|_| not_tracked())?;
		let root = index.source_root(source_id).ok_or_else(not_tracked)?;
		// Transcripts are found through the content index, which a source
		// that reads no file contents does not have.
		if !index
			.source_config(source_id)
			.is_some_and(|config| config.identify_content)
		{
			return Err(ActionError::InvalidInput(
				"this source does not read file contents, so its recordings cannot be transcribed"
					.into(),
			));
		}
		if let Some(reason) = index.dispatch_refusal(&root) {
			return Err(ActionError::InvalidInput(reason));
		}
		let under = relative_under(&root, path).await.ok_or_else(not_tracked)?;

		let data_dir = crate::config::default_data_dir()
			.map_err(|e| ActionError::Internal(format!("data directory: {e}")))?;
		let model = choose_model(
			&WhisperModelManager::new(&data_dir),
			self.input.model.as_deref(),
		)
		.await?;

		let store = index.store_for(&root).await.ok_or_else(|| {
			ActionError::Internal("the source has no store on this device".into())
		})?;
		let text = store
			.text_index()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		let pending = text
			.pending_transcripts(MEDIA_EXTENSIONS, &under, 0, 1 << 30)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.len();

		let language = self.input.language.filter(|language| language != "auto");
		let handle = library
			.jobs()
			.dispatch_with_priority(
				TranscribeJob::new(root, under, model, language, self.input.force),
				JobPriority::LOW,
				None,
			)
			.await?;
		Ok(TranscribeActionOutput {
			job_id: handle.id().0,
			model: model_name(model).to_string(),
			pending_files: u32::try_from(pending).unwrap_or(u32::MAX),
		})
	}

	fn action_kind(&self) -> &'static str {
		"media.transcribe"
	}
}

/// `path` relative to the source root in the store's spelling, or None when
/// it resolves outside the source.
async fn relative_under(root: &std::path::Path, path: &std::path::Path) -> Option<String> {
	let canonical_root = tokio::fs::canonicalize(root)
		.await
		.unwrap_or_else(|_| root.to_path_buf());
	let canonical = tokio::fs::canonicalize(path).await.ok()?;
	let relative = canonical.strip_prefix(&canonical_root).ok()?;
	Some(
		relative
			.components()
			.map(|component| component.as_os_str().to_string_lossy())
			.collect::<Vec<_>>()
			.join("/"),
	)
}

/// The model the person named, or the best downloaded one. Small is the
/// default ceiling because medium and large are several times slower on a
/// CPU for a modest gain in accuracy.
async fn choose_model(
	models: &WhisperModelManager,
	requested: Option<&str>,
) -> Result<WhisperModel, ActionError> {
	if let Some(name) = requested {
		let model = WhisperModel::from_str(name).ok_or_else(|| {
			ActionError::InvalidInput(format!(
				"unknown model '{name}'; use tiny, base, small, medium or large"
			))
		})?;
		if !models.is_downloaded(&model).await {
			return Err(ActionError::InvalidInput(format!(
				"the {name} model is not downloaded; run models.whisper.download with model '{name}' first"
			)));
		}
		return Ok(model);
	}
	for model in [
		WhisperModel::Small,
		WhisperModel::Base,
		WhisperModel::Tiny,
		WhisperModel::Medium,
		WhisperModel::Large,
	] {
		if models.is_downloaded(&model).await {
			return Ok(model);
		}
	}
	Err(ActionError::InvalidInput(
		"no Whisper model is downloaded; run models.whisper.download first".into(),
	))
}

crate::register_library_action!(TranscribeAction, "media.transcribe");
