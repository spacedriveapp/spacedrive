//! Thumbstrip processor - atomic operation for generating thumbstrips

use crate::library::Library;
use crate::ops::indexing::processor::{ProcessorEntry, ProcessorResult};
use crate::ops::indexing::state::EntryKind;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};

use super::{ThumbstripVariantConfig, ThumbstripVariants};

pub struct ThumbstripProcessor {
	library: Arc<Library>,
	pub variants: Vec<ThumbstripVariantConfig>,
	regenerate: bool,
}

impl ThumbstripProcessor {
	pub fn new(library: Arc<Library>) -> Self {
		Self {
			library,
			variants: ThumbstripVariants::defaults(),
			regenerate: false,
		}
	}

	pub fn with_variants(mut self, variants: Vec<ThumbstripVariantConfig>) -> Self {
		self.variants = variants;
		self
	}

	pub fn with_regenerate(mut self, regenerate: bool) -> Self {
		self.regenerate = regenerate;
		self
	}

	pub fn with_settings(mut self, settings: &Value) -> Result<Self> {
		// Parse variant names from settings
		if let Some(variant_names) = settings.get("variants").and_then(|v| v.as_array()) {
			let mut variants = Vec::new();
			for name in variant_names {
				if let Some(name_str) = name.as_str() {
					let variant = match name_str {
						"thumbstrip_preview" => ThumbstripVariants::preview(),
						"thumbstrip_detailed" => ThumbstripVariants::detailed(),
						"thumbstrip_mobile" => ThumbstripVariants::mobile(),
						_ => continue,
					};
					variants.push(variant);
				}
			}
			if !variants.is_empty() {
				self.variants = variants;
			}
		}

		// Parse regenerate flag
		if let Some(regen) = settings.get("regenerate").and_then(|v| v.as_bool()) {
			self.regenerate = regen;
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

		// Only video files
		entry
			.mime_type
			.as_ref()
			.map_or(false, |m| m.starts_with("video/"))
	}

	pub async fn process(
		&self,
		db: &sea_orm::DatabaseConnection,
		entry: &ProcessorEntry,
	) -> Result<ProcessorResult> {
		// Get content UUID
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
			return Ok(ProcessorResult::failure(
				"Entry has no content_id".to_string(),
			));
		};

		let mime_type = entry
			.mime_type
			.as_ref()
			.ok_or_else(|| anyhow::anyhow!("Entry has no MIME type"))?;

		if !mime_type.starts_with("video/") {
			debug!("Thumbstrips only supported for video files");
			return Ok(ProcessorResult::success(0, 0));
		}

		debug!("→ Generating thumbstrip for: {}", entry.path.display());

		// Call shared generation function
		let count = super::generate_thumbstrip_for_file(
			&self.library,
			&content_uuid,
			&entry.path,
			&self.variants,
			self.regenerate,
		)
		.await
		.map_err(|e| anyhow::anyhow!("Thumbstrip generation failed: {}", e))?;

		if count > 0 {
			debug!(
				"✓ Generated {} thumbstrip variants for: {}",
				count,
				entry.path.display()
			);
		}

		Ok(ProcessorResult::success(count, 0))
	}

	pub fn name(&self) -> &'static str {
		"thumbstrip"
	}
}
