//! Handler for volume speed test action

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        Action, error::{ActionError, ActionResult}, handler::ActionHandler, output::ActionOutput,
    },
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct VolumeSpeedTestHandler;

impl VolumeSpeedTestHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for VolumeSpeedTestHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        match action {
            Action::VolumeSpeedTest { action } => {
                // Run speed test through volume manager
                context
                    .volume_manager
                    .run_speed_test(&action.fingerprint)
                    .await
                    .map_err(|e| ActionError::Internal(e.to_string()))?;
                    
                // Get updated volume with results
                let volume = context
                    .volume_manager
                    .get_volume(&action.fingerprint)
                    .await
                    .ok_or_else(|| ActionError::InvalidInput("Volume not found after speed test".to_string()))?;
                    
                Ok(ActionOutput::VolumeSpeedTested {
                    fingerprint: action.fingerprint,
                    read_speed_mbps: volume.read_speed_mbps.map(|v| v as u32),
                    write_speed_mbps: volume.write_speed_mbps.map(|v| v as u32),
                })
            }
            _ => Err(ActionError::InvalidActionType),
        }
    }
    
    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::VolumeSpeedTest { .. })
    }
    
    fn supported_actions() -> &'static [&'static str]
    where
        Self: Sized
    {
        &["volume.speed_test"]
    }
}

// Register the handler
crate::register_action_handler!(VolumeSpeedTestHandler, "volume.speed_test");