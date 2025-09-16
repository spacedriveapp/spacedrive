//! Apply semantic tags action

use super::{input::ApplyTagsInput, output::ApplyTagsOutput};
use crate::{
    context::CoreContext,
    domain::tag::{TagApplication, TagSource},
    infra::action::{error::ActionError, LibraryAction},
    library::Library,
    ops::metadata::user_metadata_manager::UserMetadataManager,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use chrono::Utc;
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
        let mut successfully_tagged_entries = Vec::new();

        // Create tag applications from input
        let tag_applications: Vec<TagApplication> = self.input.tag_ids
            .iter()
            .map(|&tag_id| {
                let source = self.input.source.clone().unwrap_or(TagSource::User);
                let confidence = self.input.confidence.unwrap_or(1.0);
                let instance_attributes = self.input.instance_attributes
                    .clone()
                    .unwrap_or_default();

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

        // Apply tags to each entry
        for entry_id in &self.input.entry_ids {
            // Look up actual entry UUID from entry ID
            let entry_uuid = lookup_entry_uuid(&db.conn(), *entry_id).await
                .map_err(|e| ActionError::Internal(format!("Failed to lookup entry UUID: {}", e)))?;
            match metadata_manager
                .apply_semantic_tags(entry_uuid, tag_applications.clone(), device_id)
                .await
            {
                Ok(()) => {
                    successfully_tagged_entries.push(*entry_id);
                }
                Err(e) => {
                    warnings.push(format!("Failed to tag entry {}: {}", entry_id, e));
                }
            }
        }

        let output = ApplyTagsOutput::success(
            successfully_tagged_entries.len(),
            self.input.tag_ids.len(),
            self.input.tag_ids.clone(),
            successfully_tagged_entries,
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

    async fn validate(&self, _library: &Arc<Library>, _context: Arc<CoreContext>) -> Result<(), ActionError> {
        self.input.validate().map_err(|msg| ActionError::Validation {
            field: "input".to_string(),
            message: msg,
        })?;

        // TODO: Validate that tag IDs exist
        // TODO: Validate that entry IDs exist

        Ok(())
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

    entry_model.uuid
        .ok_or_else(|| format!("Entry {} has no UUID assigned", entry_id))
}