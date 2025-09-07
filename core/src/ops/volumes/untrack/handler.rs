//! Handler for volume untracking action

use crate::{
    context::CoreContext,
    infra::action::{
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
                let library = context
                    .library_manager
                    .get_library(action.library_id)
                    .await
                    .ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

                // Untrack the volume from the database
                context
                    .volume_manager
                    .untrack_volume(&library, &action.fingerprint)
                    .await
                    .map_err(|e| match e {
                        crate::volume::VolumeError::NotTracked(_) => {
                            ActionError::InvalidInput("Volume is not tracked in this library".to_string())
                        }
                        crate::volume::VolumeError::Database(msg) => {
                            ActionError::Internal(format!("Database error: {}", msg))
                        }
                        _ => ActionError::Internal(e.to_string()),
                    })?;

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