//! User Metadata Service
//!
//! Service for managing user-applied metadata including semantic tags, simple tags,
//! labels, notes, and other organizational data. This service bridges between the
//! old simple tag system and the new semantic tagging architecture.

use crate::domain::{
	tag::{TagApplication, TagError, TagSource},
	user_metadata::UserMetadata,
};
use crate::infra::db::entities::*;
use crate::ops::tags::manager::TagManager;
use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, EntityTrait, NotSet, QueryFilter, Set};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Service for managing user metadata including semantic tagging
#[derive(Clone)]
pub struct UserMetadataManager {
	db: Arc<DatabaseConnection>,
	semantic_tag_service: Arc<TagManager>,
}

impl UserMetadataManager {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		let semantic_tag_service = Arc::new(TagManager::new(db.clone()));

		Self {
			db,
			semantic_tag_service,
		}
	}

	/// Get or create entry-scoped metadata (legacy method)
	pub async fn get_or_create_metadata(&self, entry_uuid: Uuid) -> Result<UserMetadata, TagError> {
		self.get_or_create_entry_metadata(entry_uuid).await
	}

	/// Get or create entry-scoped metadata (tags specific to this file instance)
	pub async fn get_or_create_entry_metadata(
		&self,
		entry_uuid: Uuid,
	) -> Result<UserMetadata, TagError> {
		let db = &*self.db;

		// First try to find existing metadata
		if let Some(metadata) = self.get_metadata_by_entry_uuid(entry_uuid).await? {
			return Ok(metadata);
		}

		// Create new entry-scoped metadata if it doesn't exist
		let metadata_uuid = Uuid::new_v4();
		let new_metadata = user_metadata::ActiveModel {
			id: NotSet,
			uuid: Set(metadata_uuid),
			entry_uuid: Set(Some(entry_uuid)),
			content_identity_uuid: Set(None),
			notes: Set(None),
			favorite: Set(false),
			hidden: Set(false),
			custom_data: Set(serde_json::json!({})),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
		};

		new_metadata
			.insert(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(UserMetadata::new(metadata_uuid))
	}

	/// Get or create content-scoped metadata (tags apply to all instances of this content)
	pub async fn get_or_create_content_metadata(
		&self,
		content_identity_uuid: Uuid,
	) -> Result<UserMetadata, TagError> {
		let db = &*self.db;

		// First try to find existing metadata
		if let Some(metadata) = self
			.get_metadata_by_content_uuid(content_identity_uuid)
			.await?
		{
			return Ok(metadata);
		}

		// Create new content-scoped metadata if it doesn't exist
		let metadata_uuid = Uuid::new_v4();
		let new_metadata = user_metadata::ActiveModel {
			id: NotSet,
			uuid: Set(metadata_uuid),
			entry_uuid: Set(None),
			content_identity_uuid: Set(Some(content_identity_uuid)),
			notes: Set(None),
			favorite: Set(false),
			hidden: Set(false),
			custom_data: Set(serde_json::json!({})),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
		};

		new_metadata
			.insert(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(UserMetadata::new(metadata_uuid))
	}

	/// Get user metadata for an entry by entry UUID
	pub async fn get_metadata_by_entry_uuid(
		&self,
		entry_uuid: Uuid,
	) -> Result<Option<UserMetadata>, TagError> {
		let db = &*self.db;

		// Find metadata by entry UUID
		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::EntryUuid.eq(entry_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		if let Some(model) = metadata_model {
			return Ok(Some(self.model_to_domain(model).await?));
		}

		Ok(None)
	}

	/// Get user metadata for content by content identity UUID
	pub async fn get_metadata_by_content_uuid(
		&self,
		content_identity_uuid: Uuid,
	) -> Result<Option<UserMetadata>, TagError> {
		let db = &*self.db;

		// Find metadata by content identity UUID
		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::ContentIdentityUuid.eq(content_identity_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		if let Some(model) = metadata_model {
			return Ok(Some(self.model_to_domain(model).await?));
		}

		Ok(None)
	}

	/// Apply semantic tags to a content identity (tags all instances of this content)
	/// Returns the created user_metadata_tag models for syncing
	pub async fn apply_semantic_tags_to_content(
		&self,
		content_identity_uuid: Uuid,
		tag_applications: Vec<TagApplication>,
		device_uuid: Uuid,
	) -> Result<Vec<user_metadata_tag::Model>, TagError> {
		let db = &*self.db;

		// Get or create content-scoped metadata
		let metadata = self
			.get_or_create_content_metadata(content_identity_uuid)
			.await?;

		// Get the database ID for the user metadata
		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::Uuid.eq(metadata.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::DatabaseError(
				"UserMetadata not found".to_string(),
			))?;

		self.apply_tags_to_metadata(metadata_model.id, &tag_applications, device_uuid)
			.await
	}

	/// Apply semantic tags to a specific entry (tags only this instance)
	/// Returns the created user_metadata_tag models for syncing
	pub async fn apply_semantic_tags_to_entry(
		&self,
		entry_uuid: Uuid,
		tag_applications: Vec<TagApplication>,
		device_uuid: Uuid,
	) -> Result<Vec<user_metadata_tag::Model>, TagError> {
		let db = &*self.db;

		// Get or create entry-scoped metadata
		let metadata = self.get_or_create_entry_metadata(entry_uuid).await?;

		// Get the database ID for the user metadata
		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::Uuid.eq(metadata.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::DatabaseError(
				"UserMetadata not found".to_string(),
			))?;

		self.apply_tags_to_metadata(metadata_model.id, &tag_applications, device_uuid)
			.await
	}

	/// Apply semantic tags to an entry (legacy method - uses entry-scoped)
	/// Returns the created user_metadata_tag models for syncing
	pub async fn apply_semantic_tags(
		&self,
		entry_uuid: Uuid,
		tag_applications: Vec<TagApplication>,
		device_uuid: Uuid,
	) -> Result<Vec<user_metadata_tag::Model>, TagError> {
		self.apply_semantic_tags_to_entry(entry_uuid, tag_applications, device_uuid)
			.await
	}

	/// Internal: Apply tags to a metadata record (shared logic)
	/// Returns the created/updated user_metadata_tag models for syncing
	async fn apply_tags_to_metadata(
		&self,
		metadata_db_id: i32,
		tag_applications: &[TagApplication],
		device_uuid: Uuid,
	) -> Result<Vec<user_metadata_tag::Model>, TagError> {
		let db = &*self.db;

		let mut created_models = Vec::new();

		// Convert tag UUIDs to database IDs
		let tag_uuids: Vec<Uuid> = tag_applications.iter().map(|app| app.tag_id).collect();
		let tag_models = crate::infra::db::entities::Tag::find()
			.filter(crate::infra::db::entities::tag::Column::Uuid.is_in(tag_uuids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let uuid_to_db_id: HashMap<Uuid, i32> =
			tag_models.into_iter().map(|m| (m.uuid, m.id)).collect();

		// Insert tag applications
		for app in tag_applications {
			if let Some(&tag_db_id) = uuid_to_db_id.get(&app.tag_id) {
				let tag_application = user_metadata_tag::ActiveModel {
					id: NotSet,
					user_metadata_id: Set(metadata_db_id),
					tag_id: Set(tag_db_id),
					applied_context: Set(app.applied_context.clone()),
					applied_variant: Set(app.applied_variant.clone()),
					confidence: Set(app.confidence),
					source: Set(app.source.as_str().to_string()),
					instance_attributes: Set(if app.instance_attributes.is_empty() {
						None
					} else {
						Some(
							serde_json::to_value(&app.instance_attributes)
								.unwrap()
								.into(),
						)
					}),
					created_at: Set(app.created_at),
					updated_at: Set(Utc::now()),
					device_uuid: Set(device_uuid),
					uuid: Set(Uuid::new_v4()),
					version: Set(1),
				};

				// Insert or update if exists
				let model = match tag_application.clone().insert(&*db).await {
					Ok(model) => model,
					Err(_) => {
						// If insert fails due to unique constraint, update existing
						let existing = user_metadata_tag::Entity::find()
							.filter(user_metadata_tag::Column::UserMetadataId.eq(metadata_db_id))
							.filter(user_metadata_tag::Column::TagId.eq(tag_db_id))
							.one(&*db)
							.await
							.map_err(|e| TagError::DatabaseError(e.to_string()))?;

						if let Some(existing_model) = existing {
							let mut update_model: user_metadata_tag::ActiveModel =
								existing_model.into();
							update_model.applied_context = Set(app.applied_context.clone());
							update_model.applied_variant = Set(app.applied_variant.clone());
							update_model.confidence = Set(app.confidence);
							update_model.source = Set(app.source.as_str().to_string());
							update_model.instance_attributes =
								Set(if app.instance_attributes.is_empty() {
									None
								} else {
									Some(
										serde_json::to_value(&app.instance_attributes)
											.unwrap()
											.into(),
									)
								});
							update_model.updated_at = Set(Utc::now());
							update_model.device_uuid = Set(device_uuid);
							update_model.version = Set(update_model.version.unwrap() + 1);

							update_model
								.update(&*db)
								.await
								.map_err(|e| TagError::DatabaseError(e.to_string()))?
						} else {
							continue;
						}
					}
				};

				created_models.push(model);
			}
		}

		// Record usage patterns for AI learning
		self.semantic_tag_service
			.record_tag_usage(&tag_applications)
			.await?;

		Ok(created_models)
	}

	/// Remove semantic tags from an entry
	pub async fn remove_semantic_tags(
		&self,
		entry_id: i32,
		tag_ids: &[Uuid],
	) -> Result<(), TagError> {
		let db = &*self.db;

		// Get metadata for this entry
		let metadata = self.get_metadata_by_entry_uuid(Uuid::new_v4()).await?; // TODO: Look up actual UUID
		if metadata.is_none() {
			return Ok(()); // No metadata means no tags to remove
		}

		let metadata = metadata.unwrap();
		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::Uuid.eq(metadata.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::DatabaseError(
				"UserMetadata not found".to_string(),
			))?;

		// Get database IDs for tags to remove
		let tag_models = crate::infra::db::entities::tag::Entity::find()
			.filter(
				crate::infra::db::entities::tag::Column::Uuid
					.is_in(tag_ids.iter().map(|id| *id).collect::<Vec<_>>()),
			)
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let tag_db_ids: Vec<i32> = tag_models.into_iter().map(|m| m.id).collect();

		// Remove tag applications
		user_metadata_tag::Entity::delete_many()
			.filter(user_metadata_tag::Column::UserMetadataId.eq(metadata_model.id))
			.filter(user_metadata_tag::Column::TagId.is_in(tag_db_ids))
			.exec(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(())
	}

	/// Get all semantic tags applied to an entry
	pub async fn get_semantic_tags_for_entry(
		&self,
		entry_id: i32,
	) -> Result<Vec<TagApplication>, TagError> {
		let db = &*self.db;

		// Get metadata for this entry
		let metadata = self.get_metadata_by_entry_uuid(Uuid::new_v4()).await?; // TODO: Look up actual UUID
		if metadata.is_none() {
			return Ok(Vec::new());
		}

		let metadata = metadata.unwrap();
		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::Uuid.eq(metadata.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::DatabaseError(
				"UserMetadata not found".to_string(),
			))?;

		// Get all tag applications for this metadata
		let tag_applications = user_metadata_tag::Entity::find()
			.filter(user_metadata_tag::Column::UserMetadataId.eq(metadata_model.id))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let mut results = Vec::new();

		for app_model in tag_applications {
			// Get the semantic tag
			let tag_model = crate::infra::db::entities::Tag::find()
				.filter(crate::infra::db::entities::tag::Column::Id.eq(app_model.tag_id))
				.one(&*db)
				.await
				.map_err(|e| TagError::DatabaseError(e.to_string()))?;

			if let Some(tag) = tag_model {
				let instance_attributes: HashMap<String, serde_json::Value> = app_model
					.instance_attributes
					.as_ref()
					.and_then(|json| serde_json::from_value(json.clone()).ok())
					.unwrap_or_default();

				let source = TagSource::from_str(&app_model.source).unwrap_or(TagSource::User);

				results.push(TagApplication {
					tag_id: tag.uuid,
					applied_context: app_model.applied_context,
					applied_variant: app_model.applied_variant,
					confidence: app_model.confidence,
					source,
					instance_attributes,
					created_at: app_model.created_at,
					device_uuid: app_model.device_uuid,
				});
			}
		}

		Ok(results)
	}

	/// Convert database model to domain model
	async fn model_to_domain(&self, model: user_metadata::Model) -> Result<UserMetadata, TagError> {
		Ok(UserMetadata {
			id: model.uuid,
			notes: model.notes,
			favorite: model.favorite,
			hidden: model.hidden,
			custom_fields: model.custom_data,
			created_at: model.created_at,
			updated_at: model.updated_at,
		})
	}

	/// Update notes for an entry
	pub async fn update_notes(
		&self,
		entry_uuid: Uuid,
		notes: Option<String>,
	) -> Result<(), TagError> {
		let db = &*self.db;

		let metadata = self.get_or_create_metadata(entry_uuid).await?;

		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::Uuid.eq(metadata.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::DatabaseError(
				"UserMetadata not found".to_string(),
			))?;

		let mut active_model: user_metadata::ActiveModel = metadata_model.into();
		active_model.notes = Set(notes);
		active_model.updated_at = Set(Utc::now());

		active_model
			.update(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(())
	}

	/// Set favorite status for an entry
	pub async fn set_favorite(&self, entry_id: i32, is_favorite: bool) -> Result<(), TagError> {
		let db = &*self.db;

		let metadata = self.get_or_create_metadata(Uuid::new_v4()).await?; // TODO: Look up actual UUID

		let metadata_model = user_metadata::Entity::find()
			.filter(user_metadata::Column::Uuid.eq(metadata.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::DatabaseError(
				"UserMetadata not found".to_string(),
			))?;

		let mut active_model: user_metadata::ActiveModel = metadata_model.into();
		active_model.favorite = Set(is_favorite);
		active_model.updated_at = Set(Utc::now());

		active_model
			.update(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(())
	}

	/// Apply a single semantic tag to an entry
	pub async fn apply_semantic_tag(
		&self,
		entry_id: i32,
		tag_id: Uuid,
		source: TagSource,
		device_uuid: Uuid,
		confidence: Option<f32>,
		context: Option<String>,
	) -> Result<(), TagError> {
		let tag_application = TagApplication {
			tag_id,
			applied_context: context,
			applied_variant: None,
			confidence: confidence.unwrap_or(1.0),
			source,
			instance_attributes: HashMap::new(),
			created_at: Utc::now(),
			device_uuid,
		};

		self.apply_semantic_tags(Uuid::new_v4(), vec![tag_application], device_uuid)
			.await
			.map(|_| ()) // TODO: Look up actual UUID and sync models
	}

	/// Apply multiple semantic tags to an entry (user-applied)
	pub async fn apply_user_semantic_tags(
		&self,
		entry_id: i32,
		tag_ids: &[Uuid],
		device_uuid: Uuid,
	) -> Result<(), TagError> {
		let tag_applications: Vec<TagApplication> = tag_ids
			.iter()
			.map(|&tag_id| TagApplication::user_applied(tag_id, device_uuid))
			.collect();

		self.apply_semantic_tags(Uuid::new_v4(), tag_applications, device_uuid)
			.await
			.map(|_| ()) // TODO: Look up actual UUID and sync models
	}

	/// Apply AI-suggested semantic tags with confidence scores
	pub async fn apply_ai_semantic_tags(
		&self,
		entry_id: i32,
		ai_suggestions: Vec<(Uuid, f32, String)>, // (tag_id, confidence, context)
		device_uuid: Uuid,
	) -> Result<(), TagError> {
		let tag_applications: Vec<TagApplication> = ai_suggestions
			.into_iter()
			.map(|(tag_id, confidence, context)| {
				let mut app = TagApplication::ai_applied(tag_id, confidence, device_uuid);
				app.applied_context = Some(context);
				app
			})
			.collect();

		self.apply_semantic_tags(Uuid::new_v4(), tag_applications, device_uuid)
			.await
			.map(|_| ()) // TODO: Look up actual UUID and sync models
	}

	/// Find entries by semantic tags (supports hierarchy)
	pub async fn find_entries_by_semantic_tags(
		&self,
		tag_ids: &[Uuid],
		include_descendants: bool,
	) -> Result<Vec<i32>, TagError> {
		let db = &*self.db;

		let mut search_tag_ids = tag_ids.to_vec();

		// If including descendants, add all descendant tags
		if include_descendants {
			for &tag_id in tag_ids {
				let descendants = self.semantic_tag_service.get_descendants(tag_id).await?;
				search_tag_ids.extend(descendants.into_iter().map(|tag| tag.id));
			}
		}

		// Get database IDs for all tags
		let tag_models = crate::infra::db::entities::Tag::find()
			.filter(crate::infra::db::entities::tag::Column::Uuid.is_in(search_tag_ids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let tag_db_ids: Vec<i32> = tag_models.into_iter().map(|m| m.id).collect();

		if tag_db_ids.is_empty() {
			return Ok(Vec::new());
		}

		// Find all metadata that has these tags applied
		let tagged_metadata = user_metadata_tag::Entity::find()
			.filter(user_metadata_tag::Column::TagId.is_in(tag_db_ids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let metadata_ids: Vec<i32> = tagged_metadata
			.into_iter()
			.map(|m| m.user_metadata_id)
			.collect();

		if metadata_ids.is_empty() {
			return Ok(Vec::new());
		}

		// Find entries that reference this metadata
		let entries = Entry::find()
			.filter(entry::Column::MetadataId.is_in(metadata_ids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(entries.into_iter().map(|e| e.id).collect())
	}
}

impl TagSource {
	pub fn as_str(&self) -> &'static str {
		match self {
			TagSource::User => "user",
			TagSource::AI => "ai",
			TagSource::Import => "import",
			TagSource::Sync => "sync",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"user" => Some(TagSource::User),
			"ai" => Some(TagSource::AI),
			"import" => Some(TagSource::Import),
			"sync" => Some(TagSource::Sync),
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_tag_application_creation() {
		let tag_id = Uuid::new_v4();
		let device_id = Uuid::new_v4();

		let user_app = TagApplication::user_applied(tag_id, device_id);
		assert_eq!(user_app.source, TagSource::User);
		assert_eq!(user_app.confidence, 1.0);

		let ai_app = TagApplication::ai_applied(tag_id, 0.85, device_id);
		assert_eq!(ai_app.source, TagSource::AI);
		assert_eq!(ai_app.confidence, 0.85);
	}
}
