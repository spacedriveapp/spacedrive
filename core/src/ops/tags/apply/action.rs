//! Apply semantic tags action

use super::{input::{ApplyTagsInput, TagTargets}, output::ApplyTagsOutput};
use crate::{
	context::CoreContext,
	domain::tag::{TagApplication, TagSource},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::metadata::manager::UserMetadataManager,
};
use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyTagsAction {
	input: ApplyTagsInput,
}

impl ApplyTagsAction {
	pub fn new(input: ApplyTagsInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for ApplyTagsAction {
	type Input = ApplyTagsInput;
	type Output = ApplyTagsOutput;

	fn from_input(input: ApplyTagsInput) -> Result<Self, String> {
		input.validate()?;
		Ok(ApplyTagsAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let metadata_manager = UserMetadataManager::new(Arc::new(db.conn().clone()));
		let device_id = library.id(); // Use library ID as device ID

		let mut warnings = Vec::new();
		let mut successfully_tagged_count = 0;

		// Create tag applications from input
		let tag_applications: Vec<TagApplication> = self
			.input
			.tag_ids
			.iter()
			.map(|&tag_id| {
				let source = self.input.source.clone().unwrap_or(TagSource::User);
				let confidence = self.input.confidence.unwrap_or(1.0);
				let instance_attributes =
					self.input.instance_attributes.clone().unwrap_or_default();

				TagApplication {
					tag_id,
					applied_context: self.input.applied_context.clone(),
					applied_variant: None,
					confidence,
					source,
					instance_attributes,
					created_at: Utc::now(),
					device_uuid: device_id,
				}
			})
			.collect();

		// Collect affected entry UUIDs for resource events
		let mut affected_entry_uuids = Vec::new();

		// Handle both content-based and entry-based tagging
		match &self.input.targets {
			TagTargets::Content(content_ids) => {
				// Content-based tagging: apply to content identity (tags all instances)
				for &content_id in content_ids {
					match metadata_manager
						.apply_semantic_tags_to_content(content_id, tag_applications.clone(), device_id)
						.await
					{
						Ok(models) => {
							successfully_tagged_count += 1;
							// Sync each user_metadata_tag model (for cross-device sync)
							for model in models {
								library
									.sync_model(&model, crate::infra::sync::ChangeType::Insert)
									.await
									.map_err(|e| ActionError::Internal(format!("Failed to sync tag association: {}", e)))?;
							}

							// Find all entries with this content_id to emit resource events
							use crate::infra::db::entities::{content_identity, entry};
							use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

							if let Ok(Some(ci)) = content_identity::Entity::find()
								.filter(content_identity::Column::Uuid.eq(content_id))
								.one(db.conn())
								.await
							{
								if let Ok(entries) = entry::Entity::find()
									.filter(entry::Column::ContentId.eq(ci.id))
									.all(db.conn())
									.await
								{
									affected_entry_uuids.extend(entries.into_iter().filter_map(|e| e.uuid));
								}
							}
						}
						Err(e) => {
							warnings.push(format!("Failed to tag content {}: {}", content_id, e));
						}
					}
				}
			}
			TagTargets::Entry(entry_ids) => {
				// Entry-based tagging: apply to specific entry instance
				for &entry_id in entry_ids {
					// Look up actual entry UUID from entry ID
					let entry_uuid = lookup_entry_uuid(&db.conn(), entry_id)
						.await
						.map_err(|e| {
							ActionError::Internal(format!("Failed to lookup entry UUID: {}", e))
						})?;
					match metadata_manager
						.apply_semantic_tags_to_entry(entry_uuid, tag_applications.clone(), device_id)
						.await
					{
						Ok(models) => {
							successfully_tagged_count += 1;
							// Sync each user_metadata_tag model (for cross-device sync)
							for model in models {
								library
									.sync_model(&model, crate::infra::sync::ChangeType::Insert)
									.await
									.map_err(|e| ActionError::Internal(format!("Failed to sync tag association: {}", e)))?;
							}

							// Track this entry for resource events
							affected_entry_uuids.push(entry_uuid);
						}
						Err(e) => {
							warnings.push(format!("Failed to tag entry {}: {}", entry_id, e));
						}
					}
				}
			}
		}

		// Emit resource events for affected files (frontend reactivity)
		if !affected_entry_uuids.is_empty() {
			let resource_manager = crate::domain::ResourceManager::new(
				Arc::new(db.conn().clone()),
				_context.events.clone(),
			);
			if let Err(e) = resource_manager
				.emit_resource_events("file", affected_entry_uuids)
				.await
			{
				tracing::warn!("Failed to emit file resource events after tagging: {}", e);
			}
		}

		let output = ApplyTagsOutput::success(
			successfully_tagged_count,
			self.input.tag_ids.len(),
			self.input.tag_ids.clone(),
			vec![], // TODO: Return target IDs if needed
		);

		if !warnings.is_empty() {
			Ok(output.with_warnings(warnings))
		} else {
			Ok(output)
		}
	}

	fn action_kind(&self) -> &'static str {
		"tags.apply"
	}
}

// Register library action
crate::register_library_action!(ApplyTagsAction, "tags.apply");

/// Look up entry UUID from entry database ID
async fn lookup_entry_uuid(db: &DatabaseConnection, entry_id: i32) -> Result<Uuid, String> {
	use crate::infra::db::entities::entry;

	let entry_model = entry::Entity::find_by_id(entry_id)
		.one(db)
		.await
		.map_err(|e| format!("Database error: {}", e))?
		.ok_or_else(|| format!("Entry with ID {} not found", entry_id))?;

	entry_model
		.uuid
		.ok_or_else(|| format!("Entry {} has no UUID assigned", entry_id))
}
