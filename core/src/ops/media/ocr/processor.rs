//! OCR processor - atomic operation for text extraction

use crate::library::Library;
use crate::ops::indexing::processor::{ProcessorEntry, ProcessorResult};
use crate::ops::indexing::state::EntryKind;
use anyhow::Result;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};

pub struct OcrProcessor {
	library: Arc<Library>,
	languages: Vec<String>,
	min_confidence: f32,
}

impl OcrProcessor {
	pub fn new(library: Arc<Library>) -> Self {
		Self {
			library,
			languages: vec!["eng".to_string()],
			min_confidence: 0.6,
		}
	}

	pub fn with_languages(mut self, languages: Vec<String>) -> Self {
		self.languages = languages;
		self
	}

	pub fn with_min_confidence(mut self, confidence: f32) -> Self {
		self.min_confidence = confidence;
		self
	}

	pub fn with_settings(mut self, settings: &Value) -> Result<Self> {
		// Parse languages
		if let Some(langs) = settings.get("languages").and_then(|v| v.as_array()) {
			self.languages = langs
				.iter()
				.filter_map(|v| v.as_str().map(|s| s.to_string()))
				.collect();
		}

		// Parse min_confidence
		if let Some(conf) = settings.get("min_confidence").and_then(|v| v.as_f64()) {
			self.min_confidence = conf as f32;
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

		entry.mime_type.as_ref().map_or(false, |m| {
			super::is_ocr_supported(m, self.library.core_context().file_type_registry())
		})
	}

	pub async fn process(
		&self,
		db: &sea_orm::DatabaseConnection,
		entry: &ProcessorEntry,
	) -> Result<ProcessorResult> {
		let content_id = entry
			.content_id
			.ok_or_else(|| anyhow::anyhow!("Entry has no content_id"))?;

		debug!("→ Extracting text via OCR for: {}", entry.path.display());

		// Extract text
		let extracted_text = super::extract_text_from_file(&entry.path, &self.languages).await?;

		if extracted_text.is_empty() {
			debug!("No text extracted from: {}", entry.path.display());
			return Ok(ProcessorResult::success(0, 0));
		}

		debug!("✓ Extracted {} characters of text", extracted_text.len());

		// Update content_identity with extracted text
		use crate::infra::db::entities::content_identity;

		let ci = content_identity::Entity::find_by_id(content_id)
			.one(db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("ContentIdentity not found"))?;

		let mut ci_active: content_identity::ActiveModel = ci.into();
		ci_active.text_content = Set(Some(extracted_text.clone()));

		ci_active.update(db).await?;

		debug!("✓ Stored extracted text in content_identity");

		Ok(ProcessorResult::success(1, extracted_text.len() as u64))
	}

	pub fn name(&self) -> &'static str {
		"ocr"
	}
}
