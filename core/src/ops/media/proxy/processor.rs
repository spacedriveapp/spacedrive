//! Proxy processor - atomic operation for generating video proxies

use crate::library::Library;
use crate::ops::indexing::processor::{ProcessorEntry, ProcessorResult};
use crate::ops::indexing::state::EntryKind;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};

use super::{ProxyVariantConfig, ProxyVariants};

pub struct ProxyProcessor {
	library: Arc<Library>,
	pub variant: ProxyVariantConfig,
	enabled: bool,
	max_file_size_gb: u64,
	use_hardware_accel: bool,
	preset: String,
}

impl ProxyProcessor {
	pub fn new(library: Arc<Library>) -> Self {
		Self {
			library,
			variant: ProxyVariants::scrubbing(), // Only scrubbing proxy
			enabled: false,                      // Disabled by default
			max_file_size_gb: 5,
			use_hardware_accel: true,
			preset: "ultrafast".to_string(),
		}
	}

	pub fn with_enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	pub fn with_max_file_size_gb(mut self, max_gb: u64) -> Self {
		self.max_file_size_gb = max_gb;
		self
	}

	pub fn with_hardware_accel(mut self, use_hw: bool) -> Self {
		self.use_hardware_accel = use_hw;
		self
	}

	pub fn with_settings(mut self, settings: &Value) -> Result<Self> {
		if let Some(enabled) = settings.get("enabled").and_then(|v| v.as_bool()) {
			self.enabled = enabled;
		}

		if let Some(max_size) = settings.get("max_file_size_gb").and_then(|v| v.as_u64()) {
			self.max_file_size_gb = max_size;
		}

		if let Some(use_hw) = settings.get("use_hardware_accel").and_then(|v| v.as_bool()) {
			self.use_hardware_accel = use_hw;
		}

		Ok(self)
	}

	pub fn should_process(&self, entry: &ProcessorEntry) -> bool {
		if !self.enabled {
			return false;
		}

		if !matches!(entry.kind, EntryKind::File) {
			return false;
		}

		if entry.content_id.is_none() {
			return false;
		}

		// Check file size limit (skip huge files)
		let size_gb = entry.size / (1024 * 1024 * 1024);
		if size_gb > self.max_file_size_gb {
			debug!(
				"Skipping proxy generation for {}: file too large ({} GB)",
				entry.path.display(),
				size_gb
			);
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
		if !self.enabled {
			return Ok(ProcessorResult::success(0, 0));
		}

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

		debug!("→ Generating scrubbing proxy for: {}", entry.path.display());

		// Call shared generation function
		let count = super::generate_proxy_for_file(
			&self.library,
			&content_uuid,
			&entry.path,
			&[self.variant.clone()],
			self.use_hardware_accel,
			&self.preset,
			false, // Don't regenerate in processor
		)
		.await
		.map_err(|e| anyhow::anyhow!("Proxy generation failed: {}", e))?;

		if count > 0 {
			debug!("✓ Generated scrubbing proxy for: {}", entry.path.display());
		}

		Ok(ProcessorResult::success(count, 0))
	}

	pub fn name(&self) -> &'static str {
		"proxy"
	}
}
