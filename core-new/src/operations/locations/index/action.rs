//! Location index action handler

use crate::{
    context::CoreContext,
    infrastructure::{
        actions::{
            Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
        },
    },
    operations::{
        indexing::{IndexMode, job::IndexerJob},
    },
    register_action_handler,
    shared::types::SdPath,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationIndexAction {
    pub location_id: Uuid,
    pub mode: IndexMode,
}

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
        if let Action::LocationIndex { library_id, action } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // TODO: In a real implementation, we'd query the database to get the location's actual path
            // For now, let's create a placeholder SdPath  
            let location_path = SdPath::local("/placeholder"); // This should be the actual location path
            
            // Create indexer job directly
            let job = IndexerJob::from_location(action.location_id, location_path, action.mode);

            // Dispatch the job directly
            let job_handle = library
                .jobs()
                .dispatch(job)
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