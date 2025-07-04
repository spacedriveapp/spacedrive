//! File delete action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
    },
    register_action_handler,
    shared::types::{SdPath, SdPathBatch},
};
use super::job::{DeleteOptions, DeleteJob, DeleteMode};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteAction {
    pub targets: Vec<PathBuf>,
    pub options: DeleteOptions,
}

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
        if let Action::FileDelete { library_id: _, action } = action {
            if action.targets.is_empty() {
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
        if let Action::FileDelete { library_id, action } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;
            
            // Create job instance directly
            let targets = action.targets
                .into_iter()
                .map(|path| SdPath::local(path))
                .collect();

            let mode = if action.options.permanent {
                DeleteMode::Permanent
            } else {
                DeleteMode::Trash
            };

            let job = DeleteJob::new(SdPathBatch::new(targets), mode);

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
        matches!(action, Action::FileDelete { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["file.delete"]
    }
}

// Register this handler
register_action_handler!(FileDeleteHandler, "file.delete");