//! Content analysis action handler

use crate::{
    context::CoreContext,
    infra::actions::{
        error::{ActionError, ActionResult},
        handler::ActionHandler,
        output::ActionOutput,
    },
    register_action_handler,
};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentAction {
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

#[async_trait]
impl ActionHandler for ContentHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &crate::infra::actions::Action,
    ) -> ActionResult<()> {
        // TODO: Re-enable when ContentAnalysis variant is added back
        Err(ActionError::Internal("ContentAnalysis action not yet implemented".to_string()))
    }

    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: crate::infra::actions::Action,
    ) -> ActionResult<ActionOutput> {
        // TODO: Re-enable when ContentAnalysis variant is added back
        Err(ActionError::Internal("ContentAnalysis action not yet implemented".to_string()))
    }

    fn can_handle(&self, action: &crate::infra::actions::Action) -> bool {
        // TODO: Re-enable when ContentAnalysis variant is added back
        false
    }

    fn supported_actions() -> &'static [&'static str] {
        &["content.analyze"]
    }
}

register_action_handler!(ContentHandler, "content.analyze");