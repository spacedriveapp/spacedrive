//! File copy action handler

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

pub struct FileCopyHandler;

impl FileCopyHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for FileCopyHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::FileCopy { sources, destination, .. } = action {
            if sources.is_empty() {
                return Err(ActionError::Validation {
                    field: "sources".to_string(),
                    message: "At least one source file must be specified".to_string(),
                });
            }

            // Additional validation could include:
            // - Check if source files exist
            // - Check permissions
            // - Check if destination is valid
            // - Check if it would be a cross-device operation

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
        if let Action::FileCopy { sources, destination, options } = action {
            // Get the appropriate library (we'll need to determine this from the sources)
            // For now, let's assume we have a method to get the library manager
            let library_manager = &context.library_manager;
            
            // We need to determine which library this operation should run in
            // This could be determined by the source paths or passed explicitly
            // For now, let's use a placeholder approach
            
            // Convert our action options to job options
            let job_params = serde_json::json!({
                "sources": sources,
                "destination": destination,
                "options": {
                    "overwrite": options.overwrite,
                    "verify_checksum": options.verify_integrity,
                    "preserve_timestamps": options.preserve_attributes,
                    "delete_after_copy": false,
                    "move_mode": null
                }
            });

            // Get a library to run the job in (this would need proper library resolution)
            // For now, let's try to get the first available library or return an error
            let libraries = library_manager.get_open_libraries().await;
            let library = libraries.first()
                .ok_or(ActionError::Internal("No libraries available".to_string()))?;

            // Dispatch the file copy job
            let job_handle = library
                .jobs()
                .dispatch_by_name("file_copy", job_params)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionReceipt::job_based(Uuid::new_v4(), job_handle))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::FileCopy { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["file.copy"]
    }
}

// Register this handler
register_action_handler!(FileCopyHandler, "file.copy");