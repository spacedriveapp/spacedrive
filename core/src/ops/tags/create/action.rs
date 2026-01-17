//! Create semantic tag action

use super::{
	input::{ApplyToTargets, CreateTagInput},
	output::CreateTagOutput,
};
use crate::infra::sync::ChangeType;
use crate::{
	context::CoreContext,
	domain::tag::{PrivacyLevel, Tag, TagApplication, TagSource, TagType},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::metadata::manager::UserMetadataManager,
	ops::tags::manager::TagManager,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagAction {
	input: CreateTagInput,
}

impl CreateTagAction {
	pub fn new(input: CreateTagInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for CreateTagAction {
	type Input = CreateTagInput;
	type Output = CreateTagOutput;

	fn from_input(input: CreateTagInput) -> Result<Self, String> {
		input.validate()?;
		Ok(CreateTagAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let semantic_tag_manager = TagManager::new(Arc::new(db.conn().clone()));

		// Get current device ID from library context
		let device_id = library.id(); // Use library ID as device ID

		// Create the semantic tag with all optional fields
		let tag_entity = semantic_tag_manager
			.create_tag_entity_full(
				self.input.canonical_name.clone(),
				self.input.namespace.clone(),
				self.input.display_name.clone(),
				self.input.formal_name.clone(),
				self.input.abbreviation.clone(),
				self.input.aliases.clone(),
				self.input.tag_type,
				self.input.color.clone(),
				self.input.icon.clone(),
				self.input.description.clone(),
				self.input.is_organizational_anchor.unwrap_or(false),
				self.input.privacy_level,
				self.input.search_weight,
				self.input.attributes.clone(),
				device_id,
			)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to create tag: {}", e)))?;

		library
			.sync_model(&tag_entity, ChangeType::Insert)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to sync tag: {}", e)))?;

		// Emit resource event for the new tag (sidebar reactivity)
		let resource_manager = crate::domain::ResourceManager::new(
			Arc::new(library.db().conn().clone()),
			_context.events.clone(),
		);
		resource_manager
			.emit_resource_events("tag", vec![tag_entity.uuid])
			.await
			.map_err(|e| {
				ActionError::Internal(format!("Failed to emit tag resource event: {}", e))
			})?;

		// If apply_to is provided, apply the tag to those targets
		if let Some(targets) = &self.input.apply_to {
			let metadata_manager = UserMetadataManager::new(Arc::new(library.db().conn().clone()));

			// Create a tag application for this newly created tag
			let tag_application = TagApplication {
				tag_id: tag_entity.uuid,
				applied_context: None,
				applied_variant: None,
				confidence: 1.0,
				source: TagSource::User,
				instance_attributes: Default::default(),
				created_at: Utc::now(),
				device_uuid: device_id,
			};

			let mut affected_entry_uuids = Vec::new();

			match targets {
				ApplyToTargets::Content(content_ids) => {
					// Apply to content identities (all instances)
					for &content_id in content_ids {
						let models = metadata_manager
							.apply_semantic_tags_to_content(
								content_id,
								vec![tag_application.clone()],
								device_id,
							)
							.await
							.map_err(|e| {
								ActionError::Internal(format!(
									"Failed to apply tag to content: {}",
									e
								))
							})?;

						// Sync each user_metadata_tag model (for cross-device sync)
						for model in models {
							library
								.sync_model(&model, ChangeType::Insert)
								.await
								.map_err(|e| {
									ActionError::Internal(format!(
										"Failed to sync tag association: {}",
										e
									))
								})?;
						}

						// Find all entries with this content_id for resource events
						use crate::infra::db::entities::{content_identity, entry};
						use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

						if let Ok(Some(ci)) = content_identity::Entity::find()
							.filter(content_identity::Column::Uuid.eq(content_id))
							.one(library.db().conn())
							.await
						{
							if let Ok(entries) = entry::Entity::find()
								.filter(entry::Column::ContentId.eq(ci.id))
								.all(library.db().conn())
								.await
							{
								affected_entry_uuids
									.extend(entries.into_iter().filter_map(|e| e.uuid));
							}
						}
					}
				}
				ApplyToTargets::Entry(entry_ids) => {
					// Apply to specific entries
					for &entry_id in entry_ids {
						// Look up entry UUID from database ID
						let entry_uuid = lookup_entry_uuid(&library.db().conn(), entry_id)
							.await
							.map_err(|e| {
								ActionError::Internal(format!("Failed to lookup entry UUID: {}", e))
							})?;

						// Apply the tag
						let models = metadata_manager
							.apply_semantic_tags_to_entry(
								entry_uuid,
								vec![tag_application.clone()],
								device_id,
							)
							.await
							.map_err(|e| {
								ActionError::Internal(format!(
									"Failed to apply tag to entry: {}",
									e
								))
							})?;

						// Sync each user_metadata_tag model (for cross-device sync)
						for model in models {
							library
								.sync_model(&model, ChangeType::Insert)
								.await
								.map_err(|e| {
									ActionError::Internal(format!(
										"Failed to sync tag association: {}",
										e
									))
								})?;
						}

						// Track this entry for resource events
						affected_entry_uuids.push(entry_uuid);
					}
				}
			}

			// Emit resource events for affected files (frontend reactivity)
			if !affected_entry_uuids.is_empty() {
				let resource_manager = crate::domain::ResourceManager::new(
					Arc::new(library.db().conn().clone()),
					_context.events.clone(),
				);
				if let Err(e) = resource_manager
					.emit_resource_events("file", affected_entry_uuids)
					.await
				{
					tracing::warn!(
						"Failed to emit file resource events after tag creation: {}",
						e
					);
				}
			}
		}

		Ok(CreateTagOutput::from_entity(&tag_entity))
	}

	fn action_kind(&self) -> &'static str {
		"tags.create"
	}
}

// Register library action
crate::register_library_action!(CreateTagAction, "tags.create");

/// Look up entry UUID from entry database ID
async fn lookup_entry_uuid(
	db: &sea_orm::DatabaseConnection,
	entry_id: i32,
) -> Result<Uuid, String> {
	use crate::infra::db::entities::entry;
	use sea_orm::EntityTrait;

	let entry_model = entry::Entity::find_by_id(entry_id)
		.one(db)
		.await
		.map_err(|e| format!("Database error: {}", e))?
		.ok_or_else(|| format!("Entry with ID {} not found", entry_id))?;

	entry_model
		.uuid
		.ok_or_else(|| format!("Entry {} has no UUID assigned", entry_id))
}
