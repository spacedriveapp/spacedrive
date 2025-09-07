//! Volume command handler for the daemon

use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

use crate::{
	infrastructure::{
		actions::{manager::ActionManager, Action},
		cli::{
			commands::VolumeCommands,
			daemon::{
				services::StateService,
				types::{DaemonCommand, DaemonResponse, VolumeListItem},
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
				VolumeCommands::List {
					include_system,
					type_filter,
					show_types,
					show_offline,
				} => {
					// Get all currently detected volumes
					let volumes = core.volumes.get_all_volumes().await;

					// Get current library to check track status
					if let Some(library) = state_service.get_current_library(core).await {
						// Update offline status for tracked volumes
						if let Err(e) = core.volumes.update_offline_volumes(&library).await {
							debug!("Failed to update offline volumes: {}", e);
						}

						// Get tracked volumes for this library
						let tracked_volumes = match core.volumes.get_tracked_volumes(&library).await
						{
							Ok(tracked) => tracked,
							Err(_) => Vec::new(),
						};

						// Create a map of fingerprint -> tracked info for quick lookup
						let tracked_map: std::collections::HashMap<_, _> = tracked_volumes
							.iter()
							.map(|tv| (tv.fingerprint.clone(), tv))
							.collect();

						let mut volume_list_items = Vec::new();

						// Add currently detected volumes
						for volume in volumes {
							let is_tracked = tracked_map.contains_key(&volume.fingerprint);
							let tracked_name = tracked_map
								.get(&volume.fingerprint)
								.and_then(|tv| tv.display_name.clone());

							volume_list_items.push(VolumeListItem {
								volume,
								is_tracked,
								tracked_name,
								is_online: true,    // Currently detected volumes are online
								last_seen_at: None, // Not applicable for online volumes
							});
						}

						// Add offline tracked volumes if requested
						if show_offline {
							for tracked_volume in &tracked_volumes {
								// Skip if this volume is already in the online list by fingerprint
								if volume_list_items.iter().any(|item| {
									item.volume.fingerprint == tracked_volume.fingerprint
								}) {
									continue;
								}

								// Skip if we already added this offline volume (deduplicate offline volumes only)
								let already_added_offline = volume_list_items.iter().any(|item| {
									!item.is_online && // Only check against other offline volumes
									item.volume.device_id == tracked_volume.device_id &&
									item.volume.mount_point.to_string_lossy() == tracked_volume.mount_point.clone().unwrap_or_default()
								});

								if already_added_offline {
									continue;
								}

								// Convert tracked volume to offline volume
								let offline_volume = tracked_volume.to_offline_volume();

								volume_list_items.push(VolumeListItem {
									volume: offline_volume,
									is_tracked: true,
									tracked_name: tracked_volume.display_name.clone(),
									is_online: tracked_volume.is_online,
									last_seen_at: Some(tracked_volume.last_seen_at),
								});
							}
						}

						DaemonResponse::VolumeListWithTracking(volume_list_items)
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
					if let Some(library) = state_service.get_current_library(core).await {
						// Try to resolve short ID to full fingerprint
						let resolved_fingerprint = if let Some(volume) =
							core.volumes.get_volume_by_short_id(&fingerprint).await
						{
							volume.fingerprint
						} else {
							// Try as full fingerprint
							VolumeFingerprint::from_hex(fingerprint)
						};

						let action = Action::VolumeTrack {
							action: VolumeTrackAction {
								fingerprint: resolved_fingerprint,
								library_id: library.id(),
								name,
							},
						};

						match core.context.get_action_manager().await {
							Some(action_manager) => match action_manager.dispatch(action).await {
								Ok(action_output) => DaemonResponse::ActionOutput(action_output),
								Err(e) => {
									DaemonResponse::Error(format!("Failed to track volume: {}", e))
								}
							},
							None => {
								DaemonResponse::Error("Action manager not initialized".to_string())
							}
						}
					} else {
						DaemonResponse::Error(
							"No library selected. Use 'library switch' to select a library first."
								.to_string(),
						)
					}
				}

				VolumeCommands::Untrack { fingerprint } => {
					if let Some(library) = state_service.get_current_library(core).await {
						// Try to resolve short ID to full fingerprint
						let resolved_fingerprint = if let Some(volume) =
							core.volumes.get_volume_by_short_id(&fingerprint).await
						{
							volume.fingerprint
						} else {
							// Try as full fingerprint
							VolumeFingerprint::from_hex(fingerprint)
						};

						let action = Action::VolumeUntrack {
							action: VolumeUntrackAction {
								fingerprint: resolved_fingerprint,
								library_id: library.id(),
							},
						};

						match core.context.get_action_manager().await {
							Some(action_manager) => match action_manager.dispatch(action).await {
								Ok(action_output) => DaemonResponse::ActionOutput(action_output),
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
						DaemonResponse::Error(
							"No library selected. Use 'library switch' to select a library first."
								.to_string(),
						)
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
