//! Create semantic tag action

use super::{input::CreateTagInput, output::CreateTagOutput};
use crate::{
    context::CoreContext,
    domain::semantic_tag::{SemanticTag, TagType, PrivacyLevel},
    infra::action::{error::ActionError, LibraryAction},
    library::Library,
    service::semantic_tag_service::SemanticTagService,
};
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
        let semantic_tag_service = SemanticTagService::new(Arc::new(db.conn().clone()));

        // Get current device ID from library context
        let device_id = library.id(); // Use library ID as device ID

        // Create the semantic tag
        let mut tag = semantic_tag_service
            .create_tag(
                self.input.canonical_name.clone(),
                self.input.namespace.clone(),
                device_id,
            )
            .await
            .map_err(|e| ActionError::Internal(format!("Failed to create tag: {}", e)))?;

        // Apply optional fields from input
        if let Some(display_name) = self.input.display_name {
            tag.display_name = Some(display_name);
        }

        if let Some(formal_name) = self.input.formal_name {
            tag.formal_name = Some(formal_name);
        }

        if let Some(abbreviation) = self.input.abbreviation {
            tag.abbreviation = Some(abbreviation);
        }

        if !self.input.aliases.is_empty() {
            tag.aliases = self.input.aliases.clone();
        }

        if let Some(tag_type) = self.input.tag_type {
            tag.tag_type = tag_type;
        }

        if let Some(color) = self.input.color {
            tag.color = Some(color);
        }

        if let Some(icon) = self.input.icon {
            tag.icon = Some(icon);
        }

        if let Some(description) = self.input.description {
            tag.description = Some(description);
        }

        if let Some(is_anchor) = self.input.is_organizational_anchor {
            tag.is_organizational_anchor = is_anchor;
        }

        if let Some(privacy_level) = self.input.privacy_level {
            tag.privacy_level = privacy_level;
        }

        if let Some(search_weight) = self.input.search_weight {
            tag.search_weight = search_weight;
        }

        if let Some(attributes) = self.input.attributes {
            tag.attributes = attributes;
        }

        // TODO: Update the tag in database with the modified fields
        // For now, the basic tag was already created

        Ok(CreateTagOutput::from_tag(&tag))
    }

    fn action_kind(&self) -> &'static str {
        "tags.create"
    }

    async fn validate(&self, _library: &Arc<Library>, _context: Arc<CoreContext>) -> Result<(), ActionError> {
        self.input.validate().map_err(|msg| ActionError::Validation {
            field: "input".to_string(),
            message: msg,
        })?;

        Ok(())
    }
}

// Register library action
crate::register_library_action!(CreateTagAction, "tags.create");