//! File duplicate detection action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        error::{ActionError, ActionResult}, 
        handler::ActionHandler, 
        receipt::ActionReceipt,
    },
    register_action_handler,
    shared::types::{SdPath, SdPathBatch},
};
use super::job::{DuplicateDetectionJob, DetectionMode};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateDetectionAction {
    pub paths: Vec<std::path::PathBuf>,
    pub algorithm: String,
    pub threshold: f64,
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
        action: &crate::infrastructure::actions::Action,
    ) -> ActionResult<()> {
        if let crate::infrastructure::actions::Action::DetectDuplicates { action, .. } = action {
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
        action: crate::infrastructure::actions::Action,
    ) -> ActionResult<ActionReceipt> {
        if let crate::infrastructure::actions::Action::DetectDuplicates { library_id, action } = action {
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

            Ok(ActionReceipt::job_based(Uuid::new_v4(), job_handle))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &crate::infrastructure::actions::Action) -> bool {
        matches!(action, crate::infrastructure::actions::Action::DetectDuplicates { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["file.detect_duplicates"]
    }
}

register_action_handler!(DuplicateDetectionHandler, "file.detect_duplicates");