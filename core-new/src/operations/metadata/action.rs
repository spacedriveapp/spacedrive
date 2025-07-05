//! Metadata operations action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        error::{ActionError, ActionResult}, 
        handler::ActionHandler, 
        output::ActionOutput,
    },
    register_action_handler,
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetadataAction {
    pub paths: Vec<std::path::PathBuf>,
    pub extract_exif: bool,
    pub extract_xmp: bool,
}

pub struct MetadataHandler;

impl MetadataHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for MetadataHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infrastructure::actions::Action,
    ) -> ActionResult<()> {
        if let crate::infrastructure::actions::Action::MetadataOperation { action, .. } = action {
            if action.paths.is_empty() {
                return Err(ActionError::Validation {
                    field: "paths".to_string(),
                    message: "At least one path must be specified".to_string(),
                });
            }
            Ok(())
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: crate::infrastructure::actions::Action,
    ) -> ActionResult<ActionOutput> {
        if let crate::infrastructure::actions::Action::MetadataOperation { library_id, action } = action {
            let library_manager = &context.library_manager;
            
            let job_params = serde_json::json!({
                "paths": action.paths,
                "extract_exif": action.extract_exif,
                "extract_xmp": action.extract_xmp
            });

            let library = library_manager.get_library(library_id).await
                .ok_or(ActionError::Internal(format!("Library not found: {}", library_id)))?;

            let job_handle = library
                .jobs()
                .dispatch_by_name("extract_metadata", job_params)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionOutput::Success)
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infrastructure::actions::Action) -> bool {
        matches!(action, crate::infrastructure::actions::Action::MetadataOperation { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["metadata.extract"]
    }
}

register_action_handler!(MetadataHandler, "metadata.extract");