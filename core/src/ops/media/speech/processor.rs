//! Speech-to-text processor - generates subtitle files from audio/video

use crate::library::Library;
use crate::ops::indexing::processor::{ProcessorEntry, ProcessorResult};
use crate::ops::indexing::state::EntryKind;
use crate::ops::sidecar::types::{SidecarFormat, SidecarKind, SidecarVariant};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};

pub struct SpeechToTextProcessor {
	library: Arc<Library>,
	model: String,
	language: Option<String>,
}

impl SpeechToTextProcessor {
	pub fn new(library: Arc<Library>) -> Self {
		Self {
			library,
			model: "base".to_string(), // whisper base model
			language: None,            // Auto-detect
		}
	}

	pub fn with_model(mut self, model: String) -> Self {
		self.model = model;
		self
	}

	pub fn with_language(mut self, language: Option<String>) -> Self {
		self.language = language;
		self
	}

	pub fn with_settings(mut self, settings: &Value) -> Result<Self> {
		if let Some(model) = settings.get("model").and_then(|v| v.as_str()) {
			self.model = model.to_string();
		}

		if let Some(lang) = settings.get("language").and_then(|v| v.as_str()) {
			self.language = Some(lang.to_string());
		}

		Ok(self)
	}

	pub fn should_process(&self, entry: &ProcessorEntry) -> bool {
		if !matches!(entry.kind, EntryKind::File) {
			return false;
		}

		if entry.content_id.is_none() {
			return false;
		}

		entry
			.mime_type
			.as_ref()
			.map_or(false, |m| super::is_speech_supported(m))
	}

	pub async fn process(
		&self,
		db: &sea_orm::DatabaseConnection,
		entry: &ProcessorEntry,
	) -> Result<ProcessorResult> {
		let content_uuid = if let Some(content_id) = entry.content_id {
			use crate::infra::db::entities::content_identity;
			use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

			let ci = content_identity::Entity::find()
				.filter(content_identity::Column::Id.eq(content_id))
				.one(db)
				.await?
				.ok_or_else(|| anyhow::anyhow!("ContentIdentity not found"))?;

			ci.uuid
				.ok_or_else(|| anyhow::anyhow!("ContentIdentity missing UUID"))?
		} else {
			return Ok(ProcessorResult::failure("Entry has no content_id".to_string()));
		};

		debug!("→ Transcribing audio for: {}", entry.path.display());

		// Get sidecar manager
		let sidecar_manager = self
			.library
			.core_context()
			.get_sidecar_manager()
			.await
			.ok_or_else(|| anyhow::anyhow!("SidecarManager not available"))?;

		// Check if transcript already exists
		if sidecar_manager
			.exists(
				&self.library.id(),
				&content_uuid,
				&SidecarKind::Transcript,
				&SidecarVariant::new("srt"),
				&SidecarFormat::Text,
			)
			.await
			.unwrap_or(false)
		{
			debug!("Transcript already exists for {}", content_uuid);
			return Ok(ProcessorResult::success(0, 0));
		}

		// Compute sidecar path
		let sidecar_path = sidecar_manager
			.compute_path(
				&self.library.id(),
				&content_uuid,
				&SidecarKind::Transcript,
				&SidecarVariant::new("srt"),
				&SidecarFormat::Text,
			)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to compute path: {}", e))?;

		// Transcribe audio
		let srt_content = super::transcribe_audio_file(
			&entry.path,
			&self.model,
			self.language.as_deref(),
		)
		.await?;

		debug!("Transcription complete: {} bytes", srt_content.len());

		// Ensure sidecar directory exists
		if let Some(parent) = sidecar_path.absolute_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Write SRT file
		tokio::fs::write(&sidecar_path.absolute_path, &srt_content).await?;

		debug!(
			"✓ Generated subtitle file: {} ({} bytes)",
			sidecar_path.relative_path.display(),
			srt_content.len()
		);

		// Register sidecar in database
		sidecar_manager
			.record_sidecar(
				&self.library,
				&content_uuid,
				&SidecarKind::Transcript,
				&SidecarVariant::new("srt"),
				&SidecarFormat::Text,
				srt_content.len() as u64,
				None,
			)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to record sidecar: {}", e))?;

		Ok(ProcessorResult::success(1, srt_content.len() as u64))
	}

	pub fn name(&self) -> &'static str {
		"speech_to_text"
	}
}
