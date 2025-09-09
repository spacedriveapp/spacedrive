//! Indexing action handler

use crate::{
    context::CoreContext,
    infra::action::{
        error::{ActionError, ActionResult},
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
    pub library_id: uuid::Uuid,
    pub paths: Vec<std::path::PathBuf>,
    pub recursive: bool,
    pub include_hidden: bool,
}

impl IndexingAction {
    /// Create a new indexing action
    pub fn new(library_id: uuid::Uuid, paths: Vec<std::path::PathBuf>, recursive: bool, include_hidden: bool) -> Self {
        Self {
            library_id,
            paths,
            recursive,
            include_hidden,
        }
    }
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
        action: &crate::infra::action::Action,
    ) -> ActionResult<()> {
        if let crate::infra::action::Action::Index { action, .. } = action {
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
        action: crate::infra::action::Action,
    ) -> ActionResult<String> {
        if let crate::infra::action::Action::Index { library_id, action } = action {
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

            Ok("Indexing job dispatched successfully".to_string())
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infra::action::Action) -> bool {
        matches!(action, crate::infra::action::Action::Index { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["indexing.index"]
    }
}

register_action_handler!(IndexingHandler, "indexing.index");

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for IndexingAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library is pre-validated by ActionManager - no boilerplate!

        // Create indexer job
        let scope = if self.recursive {
            IndexScope::Recursive
        } else {
            IndexScope::SingleDirectory
        };

        let job = IndexerJob::new(self.paths, scope, self.include_hidden);

        // Dispatch job and return handle directly
        let job_handle = library
            .jobs()
            .dispatch(job)
            .await
            .map_err(ActionError::Job)?;

        Ok(job_handle)
    }

    fn action_kind(&self) -> &'static str {
        "indexing.index"
    }

    fn library_id(&self) -> Uuid {
        self.library_id
    }

    async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
        // Library existence already validated by ActionManager - no boilerplate!

        // Validate paths
        if self.paths.is_empty() {
            return Err(ActionError::Validation {
                field: "paths".to_string(),
                message: "At least one path must be specified".to_string(),
            });
        }

        Ok(())
    }
}