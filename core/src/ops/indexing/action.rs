//! Indexing action handler

use crate::{
    context::CoreContext,
    infra::{
        action::{error::ActionError, LibraryAction},
        job::handle::JobHandle,
    },
    domain::addressing::SdPath,
};
use super::job::{IndexerJob, IndexMode, IndexScope};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexingAction {
    pub library_id: uuid::Uuid,
    pub paths: Vec<std::path::PathBuf>,
    pub recursive: bool,
    pub include_hidden: bool,
}

impl IndexingAction {
    /// Create a new indexing action
    pub fn new(library_id: uuid::Uuid, paths: Vec<std::path::PathBuf>, recursive: bool, include_hidden: bool) -> Self {
        Self {
            library_id,
            paths,
            recursive,
            include_hidden,
        }
    }
}

pub struct IndexingHandler;

impl IndexingHandler {
    pub fn new() -> Self {
        Self
    }
}

// Old ActionHandler implementation removed

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for IndexingAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library is pre-validated by ActionManager - no boilerplate!

        // Create indexer job config for ephemeral browsing of the provided paths
        // If multiple paths provided, index each in separate jobs sequentially
        // For now, take the first path
        let first_path = self.paths.get(0)
            .cloned()
            .ok_or(ActionError::Validation { field: "paths".to_string(), message: "At least one path must be specified".to_string() })?;

        let sd_path = crate::domain::addressing::SdPath::local(first_path);
        let scope = if self.recursive { crate::ops::indexing::job::IndexScope::Recursive } else { crate::ops::indexing::job::IndexScope::Current };
        let config = crate::ops::indexing::job::IndexerJobConfig::ephemeral_browse(sd_path, scope);
        let job = crate::ops::indexing::job::IndexerJob::new(config);

        // Dispatch job and return handle directly
        let job_handle = library
            .jobs()
            .dispatch(job)
            .await
            .map_err(ActionError::Job)?;

        Ok(job_handle)
    }

    fn action_kind(&self) -> &'static str {
        "indexing.index"
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