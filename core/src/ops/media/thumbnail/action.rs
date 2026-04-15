//! Thumbnail generation action handlers

use super::{
	job::{ThumbnailJob, ThumbnailJobConfig},
	processor::ThumbnailProcessor,
};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	ops::indexing::{path_resolver::PathResolver, processor::ProcessorEntry},
};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct ThumbnailInput {
	pub paths: Vec<PathBuf>,
	pub size: u32,
	pub quality: u8,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailAction {
	input: ThumbnailInput,
}

impl ThumbnailAction {
	pub fn new(input: ThumbnailInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for ThumbnailAction {
	type Input = ThumbnailInput;
	type Output = crate::infra::job::handle::JobReceipt;

	fn from_input(input: ThumbnailInput) -> Result<Self, String> {
		Ok(ThumbnailAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let config = ThumbnailJobConfig::from_sizes(vec![self.input.size]);
		let job = ThumbnailJob::new(config);
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"media.thumbnail"
	}
}

crate::register_library_action!(ThumbnailAction, "media.thumbnail");

// ============================================================================
// Regenerate Thumbnail Action (for single file UI triggering)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct RegenerateThumbnailInput {
	/// UUID of the entry to regenerate thumbnails for
	pub entry_uuid: Uuid,
	/// Optional variant names (defaults to grid@1x, grid@2x, detail@1x)
	pub variants: Option<Vec<String>>,
	/// Force regeneration even if thumbnails exist
	pub force: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct RegenerateThumbnailOutput {
	pub generated_count: usize,
	pub variants: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegenerateThumbnailAction {
	input: RegenerateThumbnailInput,
}

impl RegenerateThumbnailAction {
	pub fn new(input: RegenerateThumbnailInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for RegenerateThumbnailAction {
	type Input = RegenerateThumbnailInput;
	type Output = RegenerateThumbnailOutput;

	fn from_input(input: RegenerateThumbnailInput) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let db = library.db().conn();

		// Load entry by UUID
		let entry = entities::entry::Entity::find()
			.filter(entities::entry::Column::Uuid.eq(self.input.entry_uuid))
			.one(db)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to load entry: {}", e)))?
			.ok_or_else(|| ActionError::Internal("Entry not found".to_string()))?;

		// Get full path
		let path = PathResolver::get_full_path(db, entry.id)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to resolve path: {}", e)))?;

		// Get MIME type: try content_identity first, fall back to extension
		let mime_type = if let Some(content_id) = entry.content_id {
			match entities::content_identity::Entity::find_by_id(content_id)
				.one(db)
				.await
			{
				Ok(Some(ci)) => {
					if let Some(mime_id) = ci.mime_type_id {
						match entities::mime_type::Entity::find_by_id(mime_id)
							.one(db)
							.await
						{
							Ok(Some(m)) => Some(m.mime_type),
							Ok(None) => mime_from_extension(&path),
							Err(e) => {
								return Err(ActionError::Internal(format!(
									"Failed to load MIME type {mime_id}: {e}"
								)))
							}
						}
					} else {
						mime_from_extension(&path)
					}
				}
				Ok(None) => mime_from_extension(&path),
				Err(e) => {
					return Err(ActionError::Internal(format!(
						"Failed to load content identity {content_id}: {e}"
					)))
				}
			}
		} else {
			mime_from_extension(&path)
		};

		// Build processor entry
		let kind = match entry.kind {
			0 => crate::ops::indexing::state::EntryKind::File,
			1 => crate::ops::indexing::state::EntryKind::Directory,
			2 => crate::ops::indexing::state::EntryKind::Symlink,
			_ => crate::ops::indexing::state::EntryKind::File,
		};

		let proc_entry = ProcessorEntry {
			id: entry.id,
			uuid: entry.uuid,
			path: path.clone(),
			kind,
			size: entry.size as u64,
			content_id: entry.content_id,
			mime_type: mime_type.clone(),
		};

		// Create thumbnail processor
		let mut processor =
			ThumbnailProcessor::new(library.clone()).with_regenerate(self.input.force);

		if let Some(variant_names) = &self.input.variants {
			let settings = serde_json::json!({ "variants": variant_names });
			processor = processor
				.with_settings(&settings)
				.map_err(|e| ActionError::Internal(format!("Invalid settings: {}", e)))?;
		}

		if !processor.should_process(&proc_entry) {
			return Err(ActionError::Internal(
				"File type does not support thumbnails".to_string(),
			));
		}

		let result = processor
			.process(db, &proc_entry)
			.await
			.map_err(|e| ActionError::Internal(format!("Thumbnail generation failed: {}", e)))?;

		if !result.success {
			return Err(ActionError::Internal(
				result.error.unwrap_or_else(|| "Unknown error".to_string()),
			));
		}

		let variant_names: Vec<String> = processor
			.variants
			.iter()
			.map(|v| v.variant.as_str().to_string())
			.collect();

		Ok(RegenerateThumbnailOutput {
			generated_count: result.artifacts_created,
			variants: variant_names,
		})
	}

	fn action_kind(&self) -> &'static str {
		"media.thumbnail.regenerate"
	}
}

crate::register_library_action!(RegenerateThumbnailAction, "media.thumbnail.regenerate");

/// Infer MIME type from file extension
fn mime_from_extension(path: &std::path::Path) -> Option<String> {
	path.extension()
		.and_then(|ext| ext.to_str())
		.and_then(|ext| match ext.to_lowercase().as_str() {
			"jpg" | "jpeg" => Some("image/jpeg"),
			"png" => Some("image/png"),
			"gif" => Some("image/gif"),
			"webp" => Some("image/webp"),
			"bmp" => Some("image/bmp"),
			"svg" => Some("image/svg+xml"),
			"tiff" | "tif" => Some("image/tiff"),
			"avif" => Some("image/avif"),
			"heic" | "heif" => Some("image/heif"),
			"mp4" => Some("video/mp4"),
			"mkv" => Some("video/x-matroska"),
			"avi" => Some("video/x-msvideo"),
			"mov" => Some("video/quicktime"),
			"webm" => Some("video/webm"),
			"pdf" => Some("application/pdf"),
			_ => None,
		})
		.map(|s| s.to_string())
}
