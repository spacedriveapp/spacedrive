//! Volume command handler for the daemon

use async_trait::async_trait;
use std::sync::Arc;

use crate::{
	infrastructure::{
		actions::{manager::ActionManager, Action},
		cli::{
			commands::VolumeCommands,
			daemon::{
				services::StateService,
				types::{DaemonCommand, DaemonResponse},
			},
		},
	},
	operations::volumes::{
		speed_test::action::VolumeSpeedTestAction, track::action::VolumeTrackAction,
		untrack::action::VolumeUntrackAction,
	},
	volume::VolumeFingerprint,
	Core,
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
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::Volume(volume_cmd) => match volume_cmd {
				VolumeCommands::List => {
					// Get all volumes
					let volumes = core.volumes.get_all_volumes().await;

					// Get current library to check track status
					if let Some(library) = state_service.get_current_library(core).await {
						// Get tracked volumes for this library
						let tracked_volumes = match core.volumes.get_tracked_volumes(&library).await
						{
							Ok(tracked) => tracked,
							Err(_) => Vec::new(),
						};

						// Create a map of fingerprint -> tracked info for quick lookup
						let tracked_map: std::collections::HashMap<_, _> = tracked_volumes
							.into_iter()
							.map(|tv| (tv.fingerprint.clone(), tv))
							.collect();

						// Combine volume info with track status
						let volume_info_list: Vec<_> = volumes
							.into_iter()
							.map(|volume| {
								let is_tracked = tracked_map.contains_key(&volume.fingerprint);
								let tracked_name = tracked_map
									.get(&volume.fingerprint)
									.and_then(|tv| tv.display_name.clone());

								serde_json::json!({
									"volume": volume,
									"is_tracked": is_tracked,
									"tracked_name": tracked_name
								})
							})
							.collect();

						DaemonResponse::VolumeListWithTracking(volume_info_list)
					} else {
						// No current library, just return basic volume list
						DaemonResponse::VolumeList(volumes)
					}
				}

				VolumeCommands::Get { fingerprint } => {
					let fingerprint = VolumeFingerprint(fingerprint);
					match core.volumes.get_volume(&fingerprint).await {
						Some(volume) => DaemonResponse::Volume(volume),
						None => DaemonResponse::Error("Volume not found".to_string()),
					}
				}

				VolumeCommands::Track { fingerprint, name } => {
					// Get current library from CLI state
					if let Some(library) = state_service.get_current_library(core).await {
						let library_id = library.id();

						let action = Action::VolumeTrack {
							action: VolumeTrackAction {
								fingerprint: VolumeFingerprint(fingerprint),
								library_id,
								name,
							},
						};

						match core.context.get_action_manager().await {
							Some(action_manager) => match action_manager.dispatch(action).await {
								Ok(output) => DaemonResponse::ActionOutput(output),
								Err(e) => {
									DaemonResponse::Error(format!("Failed to track volume: {}", e))
								}
							},
							None => {
								DaemonResponse::Error("Action manager not initialized".to_string())
							}
						}
					} else {
						DaemonResponse::Error("No current library set. Use 'spacedrive library switch <id>' to select a library.".to_string())
					}
				}

				VolumeCommands::Untrack { fingerprint } => {
					// Get current library from CLI state
					if let Some(library) = state_service.get_current_library(core).await {
						let library_id = library.id();

						let action = Action::VolumeUntrack {
							action: VolumeUntrackAction {
								fingerprint: VolumeFingerprint(fingerprint),
								library_id,
							},
						};

						match core.context.get_action_manager().await {
							Some(action_manager) => match action_manager.dispatch(action).await {
								Ok(output) => DaemonResponse::ActionOutput(output),
								Err(e) => DaemonResponse::Error(format!(
									"Failed to untrack volume: {}",
									e
								)),
							},
							None => {
								DaemonResponse::Error("Action manager not initialized".to_string())
							}
						}
					} else {
						DaemonResponse::Error("No current library set. Use 'spacedrive library switch <id>' to select a library.".to_string())
					}
				}

				VolumeCommands::SpeedTest { fingerprint } => {
					let action = Action::VolumeSpeedTest {
						action: VolumeSpeedTestAction {
							fingerprint: VolumeFingerprint(fingerprint),
						},
					};

					match core.context.get_action_manager().await {
						Some(action_manager) => match action_manager.dispatch(action).await {
							Ok(output) => DaemonResponse::ActionOutput(output),
							Err(e) => {
								DaemonResponse::Error(format!("Failed to run speed test: {}", e))
							}
						},
						None => DaemonResponse::Error("Action manager not initialized".to_string()),
					}
				}

				VolumeCommands::Refresh => match core.volumes.refresh_volumes().await {
					Ok(_) => {
						let volumes = core.volumes.get_all_volumes().await;
						DaemonResponse::VolumeList(volumes)
					}
					Err(e) => DaemonResponse::Error(format!("Failed to refresh volumes: {}", e)),
				},

				VolumeCommands::FixNames => {
					if let Some(library) = state_service.get_current_library(core).await {
						match core.volumes.update_empty_display_names(&library).await {
							Ok(count) => {
								if count > 0 {
									tracing::info!("Updated display names for {} volumes", count);
								} else {
									tracing::info!("No volumes with empty display names found");
								}
								DaemonResponse::Ok
							}
							Err(e) => DaemonResponse::Error(format!(
								"Failed to update display names: {}",
								e
							)),
						}
					} else {
						DaemonResponse::Error("No library selected".to_string())
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
