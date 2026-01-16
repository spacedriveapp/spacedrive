//! Thumbstrip generation action handlers

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "ffmpeg")]
use {
	super::processor::ThumbstripProcessor,
	crate::ops::indexing::{path_resolver::PathResolver, processor::ProcessorEntry},
};

/// Generate thumbstrip for a single video file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct GenerateThumbstripInput {
	/// UUID of the entry to generate thumbstrip for
	pub entry_uuid: Uuid,
	/// Optional variant names (defaults to thumbstrip_preview)
	pub variants: Option<Vec<String>>,
	/// Force regeneration even if thumbstrip exists
	pub force: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct GenerateThumbstripOutput {
	/// Number of thumbstrips generated
	pub generated_count: usize,
	/// Variant names that were generated
	pub variants: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerateThumbstripAction {
	input: GenerateThumbstripInput,
}

impl GenerateThumbstripAction {
	pub fn new(input: GenerateThumbstripInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for GenerateThumbstripAction {
	type Input = GenerateThumbstripInput;
	type Output = GenerateThumbstripOutput;

	fn from_input(input: GenerateThumbstripInput) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		#[cfg(feature = "ffmpeg")]
		{
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

			// Get MIME type
			let mime_type = if let Some(content_id) = entry.content_id {
				if let Ok(Some(ci)) = entities::content_identity::Entity::find_by_id(content_id)
					.one(db)
					.await
				{
					if let Some(mime_id) = ci.mime_type_id {
						if let Ok(Some(mime)) = entities::mime_type::Entity::find_by_id(mime_id)
							.one(db)
							.await
						{
							Some(mime.mime_type)
						} else {
							None
						}
					} else {
						None
					}
				} else {
					None
				}
			} else {
				return Err(ActionError::Internal(
					"Entry has no content identity".to_string(),
				));
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

			// Create thumbstrip processor with custom settings
			let mut processor =
				ThumbstripProcessor::new(library.clone()).with_regenerate(self.input.force);

			// Apply custom variants if provided
			if let Some(variant_names) = &self.input.variants {
				let settings = serde_json::json!({
					"variants": variant_names,
				});
				processor = processor
					.with_settings(&settings)
					.map_err(|e| ActionError::Internal(format!("Invalid settings: {}", e)))?;
			}

			// Check if processor should run
			if !processor.should_process(&proc_entry) {
				return Err(ActionError::Internal(
					"File type does not support thumbstrips (not a video)".to_string(),
				));
			}

			// Process the file
			let result = processor.process(db, &proc_entry).await.map_err(|e| {
				ActionError::Internal(format!("Thumbstrip generation failed: {}", e))
			})?;

			if !result.success {
				return Err(ActionError::Internal(
					result.error.unwrap_or_else(|| "Unknown error".to_string()),
				));
			}

			// Get variant names
			let variant_names: Vec<String> = processor
				.variants
				.iter()
				.map(|v| v.variant.as_str().to_string())
				.collect();

			Ok(GenerateThumbstripOutput {
				generated_count: result.artifacts_created,
				variants: variant_names,
			})
		}

		#[cfg(not(feature = "ffmpeg"))]
		{
			Err(ActionError::InvalidInput(
				"Thumbstrip generation feature is not enabled. Please rebuild the daemon with --features ffmpeg".to_string()
			))
		}
	}

	fn action_kind(&self) -> &'static str {
		"media.thumbstrip.generate"
	}
}

crate::register_library_action!(GenerateThumbstripAction, "media.thumbstrip.generate");
