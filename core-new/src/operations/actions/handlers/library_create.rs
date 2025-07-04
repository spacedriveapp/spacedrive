//! Library creation action handler

use crate::{
    context::CoreContext,
    operations::actions::{
        Action, error::ActionResult, handler::ActionHandler, receipt::ActionReceipt,
    },
    register_action_handler,
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub struct LibraryCreateHandler;

impl LibraryCreateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LibraryCreateHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionReceipt> {
        if let Action::LibraryCreate { name, path } = action {
            let library_manager = &context.library_manager;
            let new_library = library_manager.create_library(name, path).await?;

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
            Err(crate::operations::actions::error::ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LibraryCreate { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["library.create"]
    }
}

// Register this handler
register_action_handler!(LibraryCreateHandler, "library.create");