//! File validation action handler

use crate::{
    context::CoreContext,
    infra::action::{
        error::{ActionError, ActionResult},
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
    pub library_id: uuid::Uuid,
    pub paths: Vec<std::path::PathBuf>,
    pub verify_checksums: bool,
    pub deep_scan: bool,
}

impl ValidationAction {
    /// Create a new file validation action
    pub fn new(library_id: uuid::Uuid, paths: Vec<std::path::PathBuf>, verify_checksums: bool, deep_scan: bool) -> Self {
        Self {
            library_id,
            paths,
            verify_checksums,
            deep_scan,
        }
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
    ) -> ActionResult<String> {
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

            Ok("File validation job dispatched successfully".to_string())
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

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for ValidationAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library is pre-validated by ActionManager - no boilerplate!

        // Create validation job
        let mode = if self.deep_scan {
            ValidationMode::Deep
        } else {
            ValidationMode::Shallow
        };

        let job = ValidationJob::new(self.paths, mode, self.verify_checksums);

        // Dispatch job and return handle directly
        let job_handle = library
            .jobs()
            .dispatch(job)
            .await
            .map_err(ActionError::Job)?;

        Ok(job_handle)
    }

    fn action_kind(&self) -> &'static str {
        "file.validate"
    }

    fn library_id(&self) -> Uuid {
        self.library_id
    }

    async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
        // Library existence already validated by ActionManager - no boilerplate!

        // Validate paths
        if self.paths.is_empty() {
            return Err(ActionError::Validation {
                field: "paths".to_string(),
                message: "At least one path must be specified".to_string(),
            });
        }

        Ok(())
    }
}