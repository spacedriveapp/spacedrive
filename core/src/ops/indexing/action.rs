//! Indexing action handler

use crate::{
    context::CoreContext,
    infra::actions::{
        error::{ActionError, ActionResult},
        handler::ActionHandler,
        output::ActionOutput,
    },
    register_action_handler,
    domain::addressing::SdPath,
};
use super::job::{IndexerJob, IndexMode, IndexScope};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexingAction {
    pub paths: Vec<std::path::PathBuf>,
    pub recursive: bool,
    pub include_hidden: bool,
}

pub struct IndexingHandler;

impl IndexingHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for IndexingHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infra::actions::Action,
    ) -> ActionResult<()> {
        if let crate::infra::actions::Action::Index { action, .. } = action {
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
        action: crate::infra::actions::Action,
    ) -> ActionResult<ActionOutput> {
        if let crate::infra::actions::Action::Index { library_id, action } = action {
            let library_manager = &context.library_manager;

            let library = library_manager.get_library(library_id).await
                .ok_or(ActionError::Internal(format!("Library not found: {}", library_id)))?;

            // TODO: For multiple paths, we might want to create multiple jobs or handle this differently
            // For now, just take the first path
            let first_path = action.paths.into_iter().next()
                .ok_or(ActionError::Validation {
                    field: "paths".to_string(),
                    message: "At least one path must be specified".to_string(),
                })?;

            // Create indexer job directly
            // TODO: Need location_id - for now using a placeholder
            let job = IndexerJob::from_location(
                Uuid::new_v4(), // placeholder location_id
                SdPath::local(first_path),
                IndexMode::Content // default mode
            );

            // Dispatch the job directly
            let job_handle = library
                .jobs()
                .dispatch(job)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionOutput::success("Indexing job dispatched successfully"))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infra::actions::Action) -> bool {
        matches!(action, crate::infra::actions::Action::Index { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["indexing.index"]
    }
}

register_action_handler!(IndexingHandler, "indexing.index");