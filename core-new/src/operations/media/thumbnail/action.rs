//! Thumbnail generation action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        error::{ActionError, ActionResult}, 
        handler::ActionHandler, 
        output::ActionOutput,
    },
    register_action_handler,
};
use super::job::{ThumbnailJob, ThumbnailJobConfig};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailAction {
    pub paths: Vec<std::path::PathBuf>,
    pub size: u32,
    pub quality: u8,
}

pub struct ThumbnailHandler;

impl ThumbnailHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for ThumbnailHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infrastructure::actions::Action,
    ) -> ActionResult<()> {
        if let crate::infrastructure::actions::Action::GenerateThumbnails { action, .. } = action {
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
        if let crate::infrastructure::actions::Action::GenerateThumbnails { library_id, action } = action {
            let library_manager = &context.library_manager;
            
            let library = library_manager.get_library(library_id).await
                .ok_or(ActionError::Internal(format!("Library not found: {}", library_id)))?;

            // Create thumbnail job config
            let config = ThumbnailJobConfig {
                sizes: vec![action.size],
                quality: action.quality,
                regenerate: false,
                ..Default::default()
            };

            // TODO: Convert paths to entry IDs by querying the database
            // For now, create a job that processes all suitable entries
            let job = ThumbnailJob::new(config);

            // Dispatch the job directly
            let job_handle = library
                .jobs()
                .dispatch(job)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionOutput::success("Thumbnail generation job dispatched successfully"))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infrastructure::actions::Action) -> bool {
        matches!(action, crate::infrastructure::actions::Action::GenerateThumbnails { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["media.thumbnail"]
    }
}

register_action_handler!(ThumbnailHandler, "media.thumbnail");