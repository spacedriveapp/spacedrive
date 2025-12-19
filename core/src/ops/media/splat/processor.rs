//! Gaussian splat processor - generates 3D splats from images

use crate::library::Library;
use crate::ops::indexing::processor::{ProcessorEntry, ProcessorResult};
use crate::ops::indexing::state::EntryKind;
use crate::ops::sidecar::types::{SidecarFormat, SidecarKind, SidecarVariant};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};

pub struct GaussianSplatProcessor {
	library: Arc<Library>,
	model_path: Option<String>,
}

impl GaussianSplatProcessor {
	pub fn new(library: Arc<Library>) -> Self {
		Self {
			library,
			model_path: None,
		}
	}

	pub fn with_model_path(mut self, path: String) -> Self {
		self.model_path = Some(path);
		self
	}

	pub fn with_settings(mut self, settings: &Value) -> Result<Self> {
		if let Some(path) = settings.get("model_path").and_then(|v| v.as_str()) {
			self.model_path = Some(path.to_string());
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
			.map_or(false, |m| super::is_splat_supported(m))
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
			return Ok(ProcessorResult::failure(
				"Entry has no content_id".to_string(),
			));
		};

		debug!("→ Generating Gaussian splat for: {}", entry.path.display());

		// Get sidecar manager
		let sidecar_manager = self
			.library
			.core_context()
			.get_sidecar_manager()
			.await
			.ok_or_else(|| anyhow::anyhow!("SidecarManager not available"))?;

		// Check if splat already exists
		if sidecar_manager
			.exists(
				&self.library.id(),
				&content_uuid,
				&SidecarKind::GaussianSplat,
				&SidecarVariant::new("ply"),
				&SidecarFormat::Ply,
			)
			.await
			.unwrap_or(false)
		{
			debug!("Gaussian splat already exists for {}", content_uuid);
			return Ok(ProcessorResult::success(0, 0));
		}

		// Compute sidecar path
		let sidecar_path = sidecar_manager
			.compute_path(
				&self.library.id(),
				&content_uuid,
				&SidecarKind::GaussianSplat,
				&SidecarVariant::new("ply"),
				&SidecarFormat::Ply,
			)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to compute path: {}", e))?;

		// Create temporary output directory
		let temp_dir = std::env::temp_dir().join(format!("sd_splat_{}", content_uuid));
		tokio::fs::create_dir_all(&temp_dir).await?;

		// Generate splat using SHARP
		let model_path_ref = self.model_path.as_ref().map(|s| std::path::Path::new(s));
		let ply_path = super::generate_splat_from_image(&entry.path, &temp_dir, model_path_ref)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to generate Gaussian splat: {}", e))?;

		// Read generated PLY file
		let ply_data = tokio::fs::read(&ply_path).await?;
		let ply_size = ply_data.len();

		debug!("Generated splat: {} bytes", ply_size);

		// Ensure sidecar directory exists
		if let Some(parent) = sidecar_path.absolute_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Copy PLY to sidecar location
		tokio::fs::copy(&ply_path, &sidecar_path.absolute_path).await?;

		// Clean up temp directory
		let _ = tokio::fs::remove_dir_all(&temp_dir).await;

		debug!(
			"✓ Generated Gaussian splat: {} ({} bytes)",
			sidecar_path.relative_path.display(),
			ply_size
		);

		// Register sidecar in database
		sidecar_manager
			.record_sidecar(
				&self.library,
				&content_uuid,
				&SidecarKind::GaussianSplat,
				&SidecarVariant::new("ply"),
				&SidecarFormat::Ply,
				ply_size as u64,
				None,
			)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to record sidecar: {}", e))?;

		Ok(ProcessorResult::success(1, ply_size as u64))
	}

	pub fn name(&self) -> &'static str {
		"gaussian_splat"
	}
}
