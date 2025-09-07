//! Library deletion action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, output::ActionOutput,
    },
    register_action_handler,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDeleteAction {
    // Library deletion doesn't need additional fields beyond library_id
}

pub struct LibraryDeleteHandler;

impl LibraryDeleteHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LibraryDeleteHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        if let Action::LibraryDelete(action) = action {
            // For now, library deletion is not implemented in the library manager
            // This would need to be implemented as a proper method
            Err(ActionError::Internal("Library deletion not yet implemented".to_string()))
        } else {
            Err(crate::infrastructure::actions::error::ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LibraryDelete(_))
    }

    fn supported_actions() -> &'static [&'static str] {
        &["library.delete"]
    }
}

// Register this handler
register_action_handler!(LibraryDeleteHandler, "library.delete");