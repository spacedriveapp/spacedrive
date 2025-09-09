//! Content analysis action handler

use crate::{
    context::CoreContext,
    infra::{
        action::{
            error::ActionError,
            LibraryAction,
        },
        job::handle::JobHandle,
    },
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentAction {
    pub library_id: uuid::Uuid,
    pub paths: Vec<std::path::PathBuf>,
    pub analyze_content: bool,
    pub extract_metadata: bool,
}

pub struct ContentHandler;

impl ContentHandler {
    pub fn new() -> Self {
        Self
    }
}



// Add library_id to ContentAction
impl ContentAction {
    /// Create a new content analysis action
    pub fn new(library_id: uuid::Uuid, paths: Vec<std::path::PathBuf>, analyze_content: bool, extract_metadata: bool) -> Self {
        Self {
            library_id,
            paths,
            analyze_content,
            extract_metadata,
        }
    }
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for ContentAction {
    type Output = JobHandle;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // TODO: Implement content analysis job dispatch
        Err(ActionError::Internal("ContentAnalysis action not yet implemented".to_string()))
    }

    fn action_kind(&self) -> &'static str {
        "content.analyze"
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