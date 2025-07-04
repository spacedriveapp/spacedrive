//! File delete action handler

use crate::{
    context::CoreContext,
    operations::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
    },
    register_action_handler,
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub struct FileDeleteHandler;

impl FileDeleteHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for FileDeleteHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::FileDelete { targets, .. } = action {
            if targets.is_empty() {
                return Err(ActionError::Validation {
                    field: "targets".to_string(),
                    message: "At least one target file must be specified".to_string(),
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
    ) -> ActionResult<ActionReceipt> {
        if let Action::FileDelete { targets, options } = action {
            let library_manager = &context.library_manager;
            
            // Convert our action to job parameters
            let job_params = serde_json::json!({
                "targets": targets,
                "options": {
                    "permanent": options.permanent,
                    "recursive": options.recursive
                }
            });

            // Get a library to run the job in
            let libraries = library_manager.get_open_libraries().await;
            let library = libraries.first()
                .ok_or(ActionError::Internal("No libraries available".to_string()))?;

            // Dispatch the file delete job
            let job_handle = library
                .jobs()
                .dispatch_by_name("delete_files", job_params)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionReceipt::job_based(Uuid::new_v4(), job_handle))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::FileDelete { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["file.delete"]
    }
}

// Register this handler
register_action_handler!(FileDeleteHandler, "file.delete");