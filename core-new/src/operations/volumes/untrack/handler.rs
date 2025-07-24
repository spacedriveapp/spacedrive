//! Handler for volume untracking action

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, output::ActionOutput,
    },
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct VolumeUntrackHandler;

impl VolumeUntrackHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for VolumeUntrackHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        match action {
            Action::VolumeUntrack { action } => {
                // Verify library exists
                let _library = context
                    .library_manager
                    .get_library(action.library_id)
                    .await
                    .ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;
                    
                // TODO: Implement actual volume untracking from library
                
                Ok(ActionOutput::VolumeUntracked {
                    fingerprint: action.fingerprint,
                    library_id: action.library_id,
                })
            }
            _ => Err(ActionError::InvalidActionType),
        }
    }
    
    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::VolumeUntrack { .. })
    }
    
    fn supported_actions() -> &'static [&'static str]
    where
        Self: Sized
    {
        &["volume.untrack"]
    }
}

// Register the handler
crate::register_action_handler!(VolumeUntrackHandler, "volume.untrack");