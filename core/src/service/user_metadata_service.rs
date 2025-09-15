//! User Metadata Service
//!
//! Service for managing user-applied metadata including semantic tags, simple tags, 
//! labels, notes, and other organizational data. This service bridges between the
//! old simple tag system and the new semantic tagging architecture.

use crate::domain::{
    user_metadata::{UserMetadata, Tag, Label},
    semantic_tag::{TagApplication, TagSource, TagError},
};
use crate::infra::db::{entities::*, DbPool};
use crate::service::semantic_tag_service::SemanticTagService;
use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, DbConn,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Service for managing user metadata including semantic tagging
#[derive(Clone)]
pub struct UserMetadataService {
    db: Arc<DbPool>,
    semantic_tag_service: Arc<SemanticTagService>,
}

impl UserMetadataService {
    pub fn new(db: Arc<DbPool>) -> Self {
        let semantic_tag_service = Arc::new(SemanticTagService::new(db.clone()));
        
        Self {
            db,
            semantic_tag_service,
        }
    }
    
    /// Get user metadata for an entry (creates if doesn't exist)
    pub async fn get_or_create_metadata(&self, entry_id: i32) -> Result<UserMetadata, TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // First try to find existing metadata
        if let Some(metadata) = self.get_metadata_by_entry_id(entry_id).await? {
            return Ok(metadata);
        }
        
        // Create new metadata if it doesn't exist
        let metadata_uuid = Uuid::new_v4();
        let new_metadata = user_metadata::ActiveModel {
            uuid: Set(metadata_uuid),
            description: Set(None),
            album: Set(None),
            artist: Set(None),
            genre: Set(None),
            title: Set(None),
            year: Set(None),
            rating: Set(None),
            color: Set(None),
            comments: Set(None),
            tags: Set(Some(serde_json::json!([]).into())), // Empty JSON array
            is_important: Set(Some(false)),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };
        
        let result = new_metadata.insert(&*db).await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // Update entry to link to this metadata
        let entry_model = Entry::find()
            .filter(entry::Column::Id.eq(entry_id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?
            .ok_or(TagError::DatabaseError("Entry not found".to_string()))?;
        
        let mut entry_active: entry::ActiveModel = entry_model.into();
        entry_active.metadata_id = Set(Some(result.id));
        entry_active.update(&*db).await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // Return the new metadata
        Ok(UserMetadata::new(metadata_uuid))
    }
    
    /// Get user metadata for an entry by entry ID
    pub async fn get_metadata_by_entry_id(&self, entry_id: i32) -> Result<Option<UserMetadata>, TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // Find the entry and its metadata
        let entry_model = Entry::find()
            .filter(entry::Column::Id.eq(entry_id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        if let Some(entry) = entry_model {
            if let Some(metadata_id) = entry.metadata_id {
                let metadata_model = UserMetadata::find()
                    .filter(user_metadata::Column::Id.eq(metadata_id))
                    .one(&*db)
                    .await
                    .map_err(|e| TagError::DatabaseError(e.to_string()))?;
                
                if let Some(model) = metadata_model {
                    return Ok(Some(self.model_to_domain(model).await?));
                }
            }
        }
        
        Ok(None)
    }
    
    /// Apply semantic tags to an entry
    pub async fn apply_semantic_tags(
        &self,
        entry_id: i32,
        tag_applications: Vec<TagApplication>,
        device_uuid: Uuid,
    ) -> Result<(), TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // Ensure metadata exists for this entry
        let metadata = self.get_or_create_metadata(entry_id).await?;
        
        // Get the database ID for the user metadata
        let metadata_model = UserMetadata::find()
            .filter(user_metadata::Column::Uuid.eq(metadata.id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?
            .ok_or(TagError::DatabaseError("UserMetadata not found".to_string()))?;
        
        // Convert tag UUIDs to database IDs
        let tag_uuids: Vec<Uuid> = tag_applications.iter().map(|app| app.tag_id).collect();
        let tag_models = SemanticTag::find()
            .filter(semantic_tag::Column::Uuid.is_in(tag_uuids))
            .all(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let uuid_to_db_id: HashMap<Uuid, i32> = tag_models
            .into_iter()
            .map(|m| (m.uuid, m.id))
            .collect();
        
        // Insert tag applications
        for app in tag_applications {
            if let Some(&tag_db_id) = uuid_to_db_id.get(&app.tag_id) {
                let tag_application = user_metadata_semantic_tag::ActiveModel {
                    user_metadata_id: Set(metadata_model.id),
                    tag_id: Set(tag_db_id),
                    applied_context: Set(app.applied_context),
                    applied_variant: Set(app.applied_variant),
                    confidence: Set(app.confidence),
                    source: Set(app.source.as_str().to_string()),
                    instance_attributes: Set(if app.instance_attributes.is_empty() {
                        None
                    } else {
                        Some(serde_json::to_value(&app.instance_attributes).unwrap().into())
                    }),
                    created_at: Set(app.created_at),
                    updated_at: Set(Utc::now()),
                    device_uuid: Set(device_uuid),
                };
                
                // Insert or update if exists
                if let Err(_) = tag_application.insert(&*db).await {
                    // If insert fails due to unique constraint, update existing
                    let existing = UserMetadataSemanticTag::find()
                        .filter(user_metadata_semantic_tag::Column::UserMetadataId.eq(metadata_model.id))
                        .filter(user_metadata_semantic_tag::Column::TagId.eq(tag_db_id))
                        .one(&*db)
                        .await
                        .map_err(|e| TagError::DatabaseError(e.to_string()))?;
                    
                    if let Some(existing_model) = existing {
                        let mut update_model: user_metadata_semantic_tag::ActiveModel = existing_model.into();
                        update_model.applied_context = Set(app.applied_context);
                        update_model.applied_variant = Set(app.applied_variant);
                        update_model.confidence = Set(app.confidence);
                        update_model.source = Set(app.source.as_str().to_string());
                        update_model.instance_attributes = Set(if app.instance_attributes.is_empty() {
                            None
                        } else {
                            Some(serde_json::to_value(&app.instance_attributes).unwrap().into())
                        });
                        update_model.updated_at = Set(Utc::now());
                        update_model.device_uuid = Set(device_uuid);
                        
                        update_model.update(&*db).await
                            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
                    }
                }
            }
        }
        
        // Record usage patterns for AI learning
        self.semantic_tag_service.record_tag_usage(&tag_applications).await?;
        
        Ok(())
    }
    
    /// Remove semantic tags from an entry
    pub async fn remove_semantic_tags(
        &self,
        entry_id: i32,
        tag_ids: &[Uuid],
    ) -> Result<(), TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // Get metadata for this entry
        let metadata = self.get_metadata_by_entry_id(entry_id).await?;
        if metadata.is_none() {
            return Ok(()); // No metadata means no tags to remove
        }
        
        let metadata = metadata.unwrap();
        let metadata_model = UserMetadata::find()
            .filter(user_metadata::Column::Uuid.eq(metadata.id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?
            .ok_or(TagError::DatabaseError("UserMetadata not found".to_string()))?;
        
        // Get database IDs for tags to remove
        let tag_models = SemanticTag::find()
            .filter(semantic_tag::Column::Uuid.is_in(tag_ids))
            .all(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let tag_db_ids: Vec<i32> = tag_models.into_iter().map(|m| m.id).collect();
        
        // Remove tag applications
        UserMetadataSemanticTag::delete_many()
            .filter(user_metadata_semantic_tag::Column::UserMetadataId.eq(metadata_model.id))
            .filter(user_metadata_semantic_tag::Column::TagId.is_in(tag_db_ids))
            .exec(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Get all semantic tags applied to an entry
    pub async fn get_semantic_tags_for_entry(&self, entry_id: i32) -> Result<Vec<TagApplication>, TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        // Get metadata for this entry
        let metadata = self.get_metadata_by_entry_id(entry_id).await?;
        if metadata.is_none() {
            return Ok(Vec::new());
        }
        
        let metadata = metadata.unwrap();
        let metadata_model = UserMetadata::find()
            .filter(user_metadata::Column::Uuid.eq(metadata.id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?
            .ok_or(TagError::DatabaseError("UserMetadata not found".to_string()))?;
        
        // Get all tag applications for this metadata
        let tag_applications = UserMetadataSemanticTag::find()
            .filter(user_metadata_semantic_tag::Column::UserMetadataId.eq(metadata_model.id))
            .all(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let mut results = Vec::new();
        
        for app_model in tag_applications {
            // Get the semantic tag
            let tag_model = SemanticTag::find()
                .filter(semantic_tag::Column::Id.eq(app_model.tag_id))
                .one(&*db)
                .await
                .map_err(|e| TagError::DatabaseError(e.to_string()))?;
            
            if let Some(tag) = tag_model {
                let instance_attributes: HashMap<String, serde_json::Value> = app_model.instance_attributes
                    .as_ref()
                    .and_then(|json| serde_json::from_value(json.clone()).ok())
                    .unwrap_or_default();
                
                let source = TagSource::from_str(&app_model.source)
                    .unwrap_or(TagSource::User);
                
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
        // Parse legacy JSON tags
        let legacy_tags: Vec<Tag> = model.tags
            .as_ref()
            .and_then(|json| serde_json::from_value(json.clone()).ok())
            .unwrap_or_default();
        
        // TODO: Get semantic tags - for now just use legacy tags
        // In the future, this would combine both simple and semantic tags
        
        Ok(UserMetadata {
            id: model.uuid,
            tags: legacy_tags,
            labels: Vec::new(), // TODO: Implement labels if needed
            notes: model.comments,
            favorite: model.is_important.unwrap_or(false),
            hidden: false, // TODO: Add hidden field to database if needed
            custom_fields: serde_json::json!({}),
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
    
    /// Update notes for an entry
    pub async fn update_notes(
        &self,
        entry_id: i32,
        notes: Option<String>,
    ) -> Result<(), TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let metadata = self.get_or_create_metadata(entry_id).await?;
        
        let metadata_model = UserMetadata::find()
            .filter(user_metadata::Column::Uuid.eq(metadata.id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?
            .ok_or(TagError::DatabaseError("UserMetadata not found".to_string()))?;
        
        let mut active_model: user_metadata::ActiveModel = metadata_model.into();
        active_model.comments = Set(notes);
        active_model.updated_at = Set(Utc::now());
        
        active_model.update(&*db).await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Set favorite status for an entry
    pub async fn set_favorite(
        &self,
        entry_id: i32,
        is_favorite: bool,
    ) -> Result<(), TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let metadata = self.get_or_create_metadata(entry_id).await?;
        
        let metadata_model = UserMetadata::find()
            .filter(user_metadata::Column::Uuid.eq(metadata.id))
            .one(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?
            .ok_or(TagError::DatabaseError("UserMetadata not found".to_string()))?;
        
        let mut active_model: user_metadata::ActiveModel = metadata_model.into();
        active_model.is_important = Set(Some(is_favorite));
        active_model.updated_at = Set(Utc::now());
        
        active_model.update(&*db).await
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
        
        self.apply_semantic_tags(entry_id, vec![tag_application], device_uuid).await
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
        
        self.apply_semantic_tags(entry_id, tag_applications, device_uuid).await
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
        
        self.apply_semantic_tags(entry_id, tag_applications, device_uuid).await
    }
    
    /// Find entries by semantic tags (supports hierarchy)
    pub async fn find_entries_by_semantic_tags(
        &self,
        tag_ids: &[Uuid],
        include_descendants: bool,
    ) -> Result<Vec<i32>, TagError> {
        let db = self.db.get_connection().await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let mut search_tag_ids = tag_ids.to_vec();
        
        // If including descendants, add all descendant tags
        if include_descendants {
            for &tag_id in tag_ids {
                let descendants = self.semantic_tag_service.get_descendants(tag_id).await?;
                search_tag_ids.extend(descendants.into_iter().map(|tag| tag.id));
            }
        }
        
        // Get database IDs for all tags
        let tag_models = SemanticTag::find()
            .filter(semantic_tag::Column::Uuid.is_in(search_tag_ids))
            .all(&*db)
            .await
            .map_err(|e| TagError::DatabaseError(e.to_string()))?;
        
        let tag_db_ids: Vec<i32> = tag_models.into_iter().map(|m| m.id).collect();
        
        if tag_db_ids.is_empty() {
            return Ok(Vec::new());
        }
        
        // Find all metadata that has these tags applied
        let tagged_metadata = UserMetadataSemanticTag::find()
            .filter(user_metadata_semantic_tag::Column::TagId.is_in(tag_db_ids))
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