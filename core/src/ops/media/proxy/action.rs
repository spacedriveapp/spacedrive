//! Proxy generation action handlers

use super::processor::ProxyProcessor;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	ops::indexing::{path_resolver::PathResolver, processor::ProcessorEntry},
};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Generate proxy for a single video file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct GenerateProxyInput {
	/// UUID of the entry to generate proxy for
	pub entry_uuid: Uuid,
	/// Proxy resolution (scrubbing, ultra_low, quick, editing)
	pub resolution: Option<String>,
	/// Force regeneration even if proxy exists
	pub force: bool,
	/// Use hardware acceleration if available
	pub use_hardware_accel: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct GenerateProxyOutput {
	/// Number of proxies generated
	pub generated_count: usize,
	/// Variant names that were generated
	pub variants: Vec<String>,
	/// Total encoding time in seconds
	pub encoding_time_secs: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerateProxyAction {
	input: GenerateProxyInput,
}

impl GenerateProxyAction {
	pub fn new(input: GenerateProxyInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for GenerateProxyAction {
	type Input = GenerateProxyInput;
	type Output = GenerateProxyOutput;

	fn from_input(input: GenerateProxyInput) -> Result<Self, String> {
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

		// Create proxy processor with custom settings
		let use_hw = self.input.use_hardware_accel.unwrap_or(true);

		let mut processor = ProxyProcessor::new(library.clone())
			.with_enabled(true) // Enable for manual triggering
			.with_hardware_accel(use_hw);

		// Parse resolution if provided
		if let Some(resolution_str) = &self.input.resolution {
			let settings = serde_json::json!({
				"enabled": true,
				"resolution": resolution_str,
			});
			processor = processor
				.with_settings(&settings)
				.map_err(|e| ActionError::Internal(format!("Invalid settings: {}", e)))?;
		}

		// Check if processor should run
		if !processor.should_process(&proc_entry) {
			return Err(ActionError::Internal(
				"File type does not support proxies (not a video) or file too large".to_string(),
			));
		}

		// Time the operation
		let start = std::time::Instant::now();

		// Process the file
		let result = processor
			.process(db, &proc_entry)
			.await
			.map_err(|e| ActionError::Internal(format!("Proxy generation failed: {}", e)))?;

		let encoding_time = start.elapsed().as_secs();

		if !result.success {
			return Err(ActionError::Internal(
				result.error.unwrap_or_else(|| "Unknown error".to_string()),
			));
		}

		// Get variant name
		let variant_names = vec![processor.variant.variant.as_str().to_string()];

		Ok(GenerateProxyOutput {
			generated_count: result.artifacts_created,
			variants: variant_names,
			encoding_time_secs: encoding_time,
		})
	}

	fn action_kind(&self) -> &'static str {
		"media.proxy.generate"
	}
}

crate::register_library_action!(GenerateProxyAction, "media.proxy.generate");
