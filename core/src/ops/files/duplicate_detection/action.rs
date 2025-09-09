//! File duplicate detection action handler

use crate::{
    context::CoreContext,
    infra::{
        action::{error::ActionError, LibraryAction},
        job::handle::JobHandle,
    },
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

// Old ActionHandler implementation removed

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for DuplicateDetectionAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library is pre-validated by ActionManager - no boilerplate!

        // Create duplicate detection job
        let mode = match self.algorithm.as_str() {
            "size_only" => DetectionMode::SizeOnly,
            "name_and_size" => DetectionMode::NameAndSize,
            "deep_scan" => DetectionMode::DeepScan,
            _ => DetectionMode::ContentHash,
        };

        let search_paths = self.paths
            .into_iter()
            .map(|p| crate::domain::addressing::SdPath::local(p))
            .collect::<Vec<_>>();
        let search_paths = crate::domain::addressing::SdPathBatch { paths: search_paths };

        let job = DuplicateDetectionJob::new(search_paths, mode);

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