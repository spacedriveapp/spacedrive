//! Library creation action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        error::{ActionError, ActionResult}, 
        handler::ActionHandler, 
        receipt::ActionReceipt,
    },
    register_action_handler,
};
use async_trait::async_trait;
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryCreateAction {
    pub name: String,
    pub path: Option<PathBuf>,
}

pub struct LibraryCreateHandler;

impl LibraryCreateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LibraryCreateHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infrastructure::actions::Action,
    ) -> ActionResult<()> {
        if let crate::infrastructure::actions::Action::LibraryCreate(action) = action {
            if action.name.trim().is_empty() {
                return Err(ActionError::Validation {
                    field: "name".to_string(),
                    message: "Library name cannot be empty".to_string(),
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
    ) -> ActionResult<ActionReceipt> {
        if let crate::infrastructure::actions::Action::LibraryCreate(action) = action {
            let library_manager = &context.library_manager;
            let new_library = library_manager.create_library(action.name, action.path).await?;

            let library_name = new_library.name().await;
            Ok(ActionReceipt::immediate(
                Uuid::new_v4(),
                Some(serde_json::json!({
                    "library_id": new_library.id(),
                    "name": library_name,
                    "path": new_library.path().display().to_string()
                })),
            ))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infrastructure::actions::Action) -> bool {
        matches!(action, crate::infrastructure::actions::Action::LibraryCreate(_))
    }

    fn supported_actions() -> &'static [&'static str] {
        &["library.create"]
    }
}

// Register this handler
register_action_handler!(LibraryCreateHandler, "library.create");