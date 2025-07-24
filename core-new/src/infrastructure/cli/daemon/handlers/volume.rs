//! Volume command handler for the daemon

use async_trait::async_trait;
use std::sync::Arc;

use crate::{
    Core,
    infrastructure::{
        actions::{Action, manager::ActionManager},
        cli::{
            commands::VolumeCommands,
            daemon::{
                services::StateService,
                types::{DaemonCommand, DaemonResponse},
            },
        },
    },
    operations::volumes::{
        track::action::VolumeTrackAction,
        untrack::action::VolumeUntrackAction,
        speed_test::action::VolumeSpeedTestAction,
    },
    volume::VolumeFingerprint,
};

use super::CommandHandler;

/// Handler for volume commands
pub struct VolumeHandler;

#[async_trait]
impl CommandHandler for VolumeHandler {
    async fn handle(
        &self,
        cmd: DaemonCommand,
        core: &Arc<Core>,
        _state_service: &Arc<StateService>,
    ) -> DaemonResponse {
        match cmd {
            DaemonCommand::Volume(volume_cmd) => match volume_cmd {
                VolumeCommands::List => {
                    // Get all volumes
                    let volumes = core.volumes.get_all_volumes().await;
                    DaemonResponse::VolumeList(volumes)
                }
                
                VolumeCommands::Get { fingerprint } => {
                    let fingerprint = VolumeFingerprint(fingerprint);
                    match core.volumes.get_volume(&fingerprint).await {
                        Some(volume) => DaemonResponse::Volume(volume),
                        None => DaemonResponse::Error("Volume not found".to_string()),
                    }
                }
                
                VolumeCommands::Track { library_id, fingerprint, name } => {
                    let action = Action::VolumeTrack {
                        action: VolumeTrackAction {
                            fingerprint: VolumeFingerprint(fingerprint),
                            library_id,
                            name,
                        },
                    };
                    
                    match core.context.get_action_manager().await {
                        Some(action_manager) => {
                            match action_manager.dispatch(action).await {
                                Ok(output) => DaemonResponse::ActionOutput(output),
                                Err(e) => DaemonResponse::Error(format!("Failed to track volume: {}", e)),
                            }
                        }
                        None => DaemonResponse::Error("Action manager not initialized".to_string()),
                    }
                }
                
                VolumeCommands::Untrack { library_id, fingerprint } => {
                    let action = Action::VolumeUntrack {
                        action: VolumeUntrackAction {
                            fingerprint: VolumeFingerprint(fingerprint),
                            library_id,
                        },
                    };
                    
                    match core.context.get_action_manager().await {
                        Some(action_manager) => {
                            match action_manager.dispatch(action).await {
                                Ok(output) => DaemonResponse::ActionOutput(output),
                                Err(e) => DaemonResponse::Error(format!("Failed to untrack volume: {}", e)),
                            }
                        }
                        None => DaemonResponse::Error("Action manager not initialized".to_string()),
                    }
                }
                
                VolumeCommands::SpeedTest { fingerprint } => {
                    let action = Action::VolumeSpeedTest {
                        action: VolumeSpeedTestAction {
                            fingerprint: VolumeFingerprint(fingerprint),
                        },
                    };
                    
                    match core.context.get_action_manager().await {
                        Some(action_manager) => {
                            match action_manager.dispatch(action).await {
                                Ok(output) => DaemonResponse::ActionOutput(output),
                                Err(e) => DaemonResponse::Error(format!("Failed to run speed test: {}", e)),
                            }
                        }
                        None => DaemonResponse::Error("Action manager not initialized".to_string()),
                    }
                }
                
                VolumeCommands::Refresh => {
                    match core.volumes.refresh_volumes().await {
                        Ok(_) => {
                            let volumes = core.volumes.get_all_volumes().await;
                            DaemonResponse::VolumeList(volumes)
                        }
                        Err(e) => DaemonResponse::Error(format!("Failed to refresh volumes: {}", e)),
                    }
                }
            },
            _ => DaemonResponse::Error("Invalid command for volume handler".to_string()),
        }
    }

    fn can_handle(&self, cmd: &DaemonCommand) -> bool {
        matches!(cmd, DaemonCommand::Volume(_))
    }
}