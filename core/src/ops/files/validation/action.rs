//! File validation action handler

use crate::{
    context::CoreContext,
    infra::{
        action::{error::ActionError, LibraryAction},
        job::handle::JobHandle,
    },
};
use super::job::{ValidationJob, ValidationMode};
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

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for ValidationAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Create validation job
        let mode = if self.deep_scan {
            ValidationMode::Complete
        } else if self.verify_checksums {
            ValidationMode::Integrity
        } else {
            ValidationMode::Basic
        };

        // Convert paths into SdPathBatch
        let targets = self.paths
            .into_iter()
            .map(|p| crate::domain::addressing::SdPath::local(p))
            .collect::<Vec<_>>();
        let targets = crate::domain::addressing::SdPathBatch { paths: targets };

        let job = ValidationJob::new(targets, mode);

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