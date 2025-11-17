//! Thumbnail processor - atomic operation for generating thumbnails

use super::{ThumbnailUtils, ThumbnailVariantConfig, ThumbnailVariants};
use crate::library::Library;
use crate::ops::indexing::processor::{ProcessorEntry, ProcessorResult};
use crate::ops::indexing::state::EntryKind;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

pub struct ThumbnailProcessor {
	library: Arc<Library>,
	pub variants: Vec<ThumbnailVariantConfig>,
	regenerate: bool,
}

impl ThumbnailProcessor {
	pub fn new(library: Arc<Library>) -> Self {
		Self {
			library,
			variants: ThumbnailVariants::defaults(),
			regenerate: false,
		}
	}

	pub fn with_variants(mut self, variants: Vec<ThumbnailVariantConfig>) -> Self {
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
						"grid@1x" => ThumbnailVariants::grid_1x(),
						"grid@2x" => ThumbnailVariants::grid_2x(),
						"detail@1x" => ThumbnailVariants::detail_1x(),
						"detail@2x" => ThumbnailVariants::detail_2x(),
						"icon@1x" => ThumbnailVariants::icon_1x(),
						"icon@2x" => ThumbnailVariants::icon_2x(),
						_ => continue,
					};
					variants.push(variant);
				}
			}
			if !variants.is_empty() {
				self.variants = variants;
			}
		}

		// Parse quality
		if let Some(quality) = settings.get("quality").and_then(|v| v.as_u64()) {
			for variant in &mut self.variants {
				variant.quality = quality as u8;
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

		entry
			.mime_type
			.as_ref()
			.map_or(false, |m| ThumbnailUtils::is_thumbnail_supported(m))
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

		if !ThumbnailUtils::is_thumbnail_supported(mime_type) {
			debug!("Thumbnails not supported for MIME type: {}", mime_type);
			return Ok(ProcessorResult::success(0, 0));
		}

		debug!("→ Generating thumbnails for: {}", entry.path.display());

		let count = super::generate_thumbnails_for_file(
			&self.library,
			&content_uuid,
			&entry.path,
			mime_type,
		)
		.await
		.map_err(|e| anyhow::anyhow!("Thumbnail generation failed: {}", e))?;

		if count > 0 {
			debug!(
				"✓ Generated {} thumbnails for: {}",
				count,
				entry.path.display()
			);
		}

		Ok(ProcessorResult::success(count, 0))
	}

	pub fn name(&self) -> &'static str {
		"thumbnail"
	}
}
