//! File duplicate detection action handler

use crate::{
    context::CoreContext,
    infra::action::{
        error::{ActionError, ActionResult},
    },
    register_action_handler,
    domain::addressing::{SdPath, SdPathBatch},
};
use super::job::{DuplicateDetectionJob, DetectionMode};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateDetectionAction {
    pub library_id: uuid::Uuid,
    pub paths: Vec<std::path::PathBuf>,
    pub algorithm: String,
    pub threshold: f64,
}

impl DuplicateDetectionAction {
    /// Create a new duplicate detection action
    pub fn new(library_id: uuid::Uuid, paths: Vec<std::path::PathBuf>, algorithm: String, threshold: f64) -> Self {
        Self {
            library_id,
            paths,
            algorithm,
            threshold,
        }
    }
}

pub struct DuplicateDetectionHandler;

impl DuplicateDetectionHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for DuplicateDetectionHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infra::action::Action,
    ) -> ActionResult<()> {
        if let crate::infra::action::Action::DetectDuplicates { action, .. } = action {
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
        if let crate::infra::action::Action::DetectDuplicates { library_id, action } = action {
            let library_manager = &context.library_manager;

            let library = library_manager.get_library(library_id).await
                .ok_or(ActionError::Internal(format!("Library not found: {}", library_id)))?;

            // Convert paths to SdPath and create job
            let search_paths = action.paths
                .into_iter()
                .map(|path| SdPath::local(path))
                .collect();

            // Parse algorithm to detection mode
            let mode = match action.algorithm.as_str() {
                "content_hash" => DetectionMode::ContentHash,
                "size_only" => DetectionMode::SizeOnly,
                "name_and_size" => DetectionMode::NameAndSize,
                "deep_scan" => DetectionMode::DeepScan,
                _ => DetectionMode::ContentHash, // default
            };

            let job = DuplicateDetectionJob::new(SdPathBatch::new(search_paths), mode);

            // Dispatch the job directly
            let job_handle = library
                .jobs()
                .dispatch(job)
                .await
                .map_err(ActionError::Job)?;

            Ok("Duplicate detection job dispatched successfully".to_string())
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infra::action::Action) -> bool {
        matches!(action, crate::infra::action::Action::DetectDuplicates { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["file.detect_duplicates"]
    }
}

register_action_handler!(DuplicateDetectionHandler, "file.detect_duplicates");

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for DuplicateDetectionAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library is pre-validated by ActionManager - no boilerplate!

        // Create duplicate detection job
        let mode = DetectionMode::from_algorithm(&self.algorithm, self.threshold);
        let job = DuplicateDetectionJob::new(self.paths, mode);

        // Dispatch job and return handle directly
        let job_handle = library
            .jobs()
            .dispatch(job)
            .await
            .map_err(ActionError::Job)?;

        Ok(job_handle)
    }

    fn action_kind(&self) -> &'static str {
        "file.detect_duplicates"
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