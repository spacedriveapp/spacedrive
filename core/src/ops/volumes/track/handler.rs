//! Handler for volume tracking action

use crate::{
    context::CoreContext,
    infra::action::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, output::ActionOutput,
    },
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct VolumeTrackHandler;

impl VolumeTrackHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for VolumeTrackHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        match action {
            Action::VolumeTrack { action } => {
                // Execute the action using the volume manager from context
                let library = context
                    .library_manager
                    .get_library(action.library_id)
                    .await
                    .ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

                let volume = context
                    .volume_manager
                    .get_volume(&action.fingerprint)
                    .await
                    .ok_or_else(|| ActionError::InvalidInput("Volume not found".to_string()))?;

                if !volume.is_mounted {
                    return Err(ActionError::InvalidInput(
                        "Cannot track unmounted volume".to_string()
                    ));
                }

                // Track the volume in the database
                let tracked = context
                    .volume_manager
                    .track_volume(&library, &action.fingerprint, action.name.clone())
                    .await
                    .map_err(|e| match e {
                        crate::volume::VolumeError::AlreadyTracked(_) => {
                            ActionError::InvalidInput("Volume is already tracked in this library".to_string())
                        }
                        crate::volume::VolumeError::NotFound(_) => {
                            ActionError::InvalidInput("Volume not found".to_string())
                        }
                        crate::volume::VolumeError::Database(msg) => {
                            ActionError::Internal(format!("Database error: {}", msg))
                        }
                        _ => ActionError::Internal(e.to_string()),
                    })?;

                Ok(ActionOutput::VolumeTracked {
                    fingerprint: action.fingerprint,
                    library_id: action.library_id,
                    volume_name: tracked.display_name.unwrap_or(volume.name),
                })
            }
            _ => Err(ActionError::InvalidActionType),
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::VolumeTrack { .. })
    }

    fn supported_actions() -> &'static [&'static str]
    where
        Self: Sized
    {
        &["volume.track"]
    }
}

// Register the handler
crate::register_action_handler!(VolumeTrackHandler, "volume.track");