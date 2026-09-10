//! Create semantic tag action

use super::{input::CreateTagInput, output::CreateTagOutput};
use crate::infra::sync::ChangeType;
use crate::{
	context::CoreContext,
	domain::tag::{PrivacyLevel, Tag, TagApplication, TagSource, TagType},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::metadata::manager::UserMetadataManager,
	ops::tags::apply::input::TagTargets,
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

			let cache = _context.ephemeral_cache();

			match targets {
				TagTargets::Content(content_ids) => {
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

						affected_entry_uuids.extend(
							cache
								.copies_of_content(content_id)
								.await
								.into_iter()
								.map(|copy| copy.record_uuid),
						);
					}
				}
				TagTargets::File(record_uuids) => {
					let mut missing = Vec::new();
					for &record_uuid in record_uuids {
						if cache.path_of_record(record_uuid).await.is_none() {
							missing.push(record_uuid);
						}
					}
					if !missing.is_empty() {
						return Err(ActionError::InvalidInput(format!(
							"Files not indexed: {missing:?}"
						)));
					}

					for &record_uuid in record_uuids {
						let models = metadata_manager
							.apply_semantic_tags_to_entry(
								record_uuid,
								vec![tag_application.clone()],
								device_id,
							)
							.await
							.map_err(|e| {
								ActionError::Internal(format!("Failed to apply tag to file: {}", e))
							})?;

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
						affected_entry_uuids.push(record_uuid);
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
