//! Library rename action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        error::{ActionError, ActionResult},
        handler::ActionHandler,
        output::ActionOutput,
        Action,
    },
    library::LibraryConfig,
    register_action_handler,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRenameAction {
    pub library_id: Uuid,
    pub new_name: String,
}

pub struct LibraryRenameHandler;

impl LibraryRenameHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LibraryRenameHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::LibraryRename { action, .. } = action {
            if action.new_name.is_empty() {
                return Err(ActionError::Validation {
                    field: "new_name".to_string(),
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
        action: Action,
    ) -> ActionResult<ActionOutput> {
        if let Action::LibraryRename { library_id, action } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Get current config
            let old_config = library.config().await;
            let old_name = old_config.name.clone();
            
            // Update the library name using update_config
            library.update_config(|config| {
                config.name = action.new_name.clone();
            }).await
                .map_err(|e| ActionError::Internal(format!("Failed to save config: {}", e)))?;

            let output = super::output::LibraryRenameOutput {
                library_id,
                old_name,
                new_name: action.new_name,
            };

            Ok(ActionOutput::from_trait(output))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LibraryRename { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["library.rename"]
    }
}

register_action_handler!(LibraryRenameHandler, "library.rename");