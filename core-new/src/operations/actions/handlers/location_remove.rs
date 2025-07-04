//! Location remove action handler

use crate::{
    context::CoreContext,
    location::manager::LocationManager,
    operations::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
    },
    register_action_handler,
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

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
    ) -> ActionResult<ActionReceipt> {
        if let Action::LocationRemove { library_id, location_id } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Remove the location
            let location_manager = LocationManager::new(context.events.as_ref().clone());
            location_manager
                .remove_location(&library, location_id)
                .await
                .map_err(|e| ActionError::Internal(e.to_string()))?;

            Ok(ActionReceipt::immediate(
                Uuid::new_v4(),
                Some(serde_json::json!({
                    "location_id": location_id,
                    "removed": true
                })),
            ))
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