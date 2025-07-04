//! Location index action handler

use crate::{
    context::CoreContext,
    operations::{
        actions::{
            Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
        },
        indexing::{IndexMode as CoreIndexMode, IndexScope, job::IndexerJobConfig},
    },
    register_action_handler,
    shared::types::SdPath,
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub struct LocationIndexHandler;

impl LocationIndexHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LocationIndexHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionReceipt> {
        if let Action::LocationIndex { library_id, location_id, mode } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Convert action mode to core indexing mode
            let core_mode = match mode {
                crate::operations::actions::IndexMode::Shallow => CoreIndexMode::Shallow,
                crate::operations::actions::IndexMode::Deep => CoreIndexMode::Deep,
                crate::operations::actions::IndexMode::Sync => CoreIndexMode::Deep, // Treat sync as deep for indexing
            };

            // We need to get the location record to get the path
            // For now, let's create a placeholder SdPath - in a real implementation,
            // we'd query the database to get the location's actual path
            let location_path = SdPath::local("/placeholder"); // This should be the actual location path
            
            // Create indexer job configuration
            let indexer_config = IndexerJobConfig {
                location_id: Some(location_id),
                path: location_path,
                mode: core_mode,
                scope: IndexScope::Recursive,
                persistence: crate::operations::indexing::IndexPersistence::Persistent,
                max_depth: None,
            };

            // Dispatch an indexing job
            let job_params = serde_json::to_value(&indexer_config)
                .map_err(ActionError::JsonSerialization)?;

            let job_handle = library
                .jobs()
                .dispatch_by_name("indexer", job_params)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionReceipt::job_based(Uuid::new_v4(), job_handle))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LocationIndex { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["location.index"]
    }
}

// Register this handler
register_action_handler!(LocationIndexHandler, "location.index");