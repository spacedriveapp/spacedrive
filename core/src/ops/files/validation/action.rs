//! File validation action handler

use crate::{
    context::CoreContext,
    infra::action::{
        error::{ActionError, ActionResult},
        handler::ActionHandler,
        output::ActionOutput,
    },
    register_action_handler,
    domain::addressing::{SdPath, SdPathBatch},
};
use super::job::{ValidationJob, ValidationMode};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationAction {
    pub paths: Vec<std::path::PathBuf>,
    pub verify_checksums: bool,
    pub deep_scan: bool,
}

pub struct ValidationHandler;

impl ValidationHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for ValidationHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infra::action::Action,
    ) -> ActionResult<()> {
        if let crate::infra::action::Action::FileValidate { action, .. } = action {
            if action.paths.is_empty() {
                return Err(ActionError::Validation {
                    field: "paths".to_string(),
                    message: "At least one path must be specified".to_string(),
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
        action: crate::infra::action::Action,
    ) -> ActionResult<ActionOutput> {
        if let crate::infra::action::Action::FileValidate { library_id, action } = action {
            let library_manager = &context.library_manager;

            let library = library_manager.get_library(library_id).await
                .ok_or(ActionError::Internal(format!("Library not found: {}", library_id)))?;

            // Convert paths to SdPath and create job
            let targets = action.paths
                .into_iter()
                .map(|path| SdPath::local(path))
                .collect();

            // Determine validation mode based on action parameters
            let mode = if action.deep_scan {
                ValidationMode::Complete
            } else if action.verify_checksums {
                ValidationMode::Integrity
            } else {
                ValidationMode::Basic
            };

            let job = ValidationJob::new(SdPathBatch::new(targets), mode);

            // Dispatch the job directly
            let job_handle = library
                .jobs()
                .dispatch(job)
                .await
                .map_err(ActionError::Job)?;

            Ok(ActionOutput::success("File validation job dispatched successfully"))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infra::action::Action) -> bool {
        matches!(action, crate::infra::action::Action::FileValidate { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["file.validate"]
    }
}

register_action_handler!(ValidationHandler, "file.validate");