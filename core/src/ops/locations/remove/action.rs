//! Location remove action handler

use crate::{
    context::CoreContext,
    location::manager::LocationManager,
    infra::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, output::ActionOutput,
    },
    register_action_handler,
};
use super::output::LocationRemoveOutput;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRemoveAction {
    pub location_id: Uuid,
}

pub struct LocationRemoveHandler;

impl LocationRemoveHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LocationRemoveHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        if let Action::LocationRemove { library_id, action } = action {
            let library_manager = &context.library_manager;

            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Remove the location
            let location_manager = LocationManager::new(context.events.as_ref().clone());
            location_manager
                .remove_location(&library, action.location_id)
                .await
                .map_err(|e| ActionError::Internal(e.to_string()))?;

            let output = LocationRemoveOutput::new(action.location_id, None);
            Ok(ActionOutput::from_trait(output))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LocationRemove { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["location.remove"]
    }
}

// Register this handler
register_action_handler!(LocationRemoveHandler, "location.remove");