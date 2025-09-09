//! Location command handlers

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::infra::cli::daemon::services::StateService;
use crate::infra::cli::daemon::types::{DaemonCommand, DaemonResponse, LocationInfo};
// ActionOutput enum removed - using native output types now
use crate::Core;

use super::CommandHandler;

/// Handler for location commands
pub struct LocationHandler;

#[async_trait]
impl CommandHandler for LocationHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::AddLocation { path, name } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let library_id = library.id();

					// Get the action manager
					match core.context.get_action_manager().await {
						Some(action_manager) => {
							// Create the location add action
							let action = crate::ops::locations::add::action::LocationAddAction {
								library_id,
								path: path.clone(),
								name,
								mode: crate::ops::indexing::IndexMode::Content,
							};

							// Dispatch the action
							match core.execute_library_action(action).await {
								Ok(_output) => {
									// Placeholder: return success; proper output can include IDs
									let location_id = uuid::Uuid::new_v4();
									DaemonResponse::LocationAdded {
										location_id,
										job_id: "".to_string(),
									}
								}
								Err(e) => {
									DaemonResponse::Error(format!("Failed to add location: {}", e))
								}
							}
						}
						None => DaemonResponse::Error("Action manager not available".to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::ListLocations => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					// For listing, we can directly query the database since it's a read operation
					use crate::infra::db::entities;
					use crate::ops::indexing::PathResolver;
					use sea_orm::EntityTrait;

					match entities::location::Entity::find()
						.all(library.db().conn())
						.await
					{
						Ok(locations) => {
							let mut infos = Vec::new();
							for loc in locations {
								let path = match PathResolver::get_full_path(library.db().conn(), loc.entry_id).await {
									Ok(p) => p,
									Err(_) => PathBuf::from("<unknown>"),
								};
								infos.push(LocationInfo {
									id: loc.uuid,
									name: loc.name.unwrap_or_default(),
									path,
									status: if loc.scan_state == "1" {
										"active"
									} else {
										"idle"
									}
									.to_string(),
								});
							}

							DaemonResponse::Locations(infos)
						}
						Err(e) => DaemonResponse::Error(format!("Failed to list locations: {}", e)),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::RemoveLocation { id } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let library_id = library.id();

					// Get the action manager
					match core.context.get_action_manager().await {
						Some(action_manager) => {
							// Create the location remove action
							let action = crate::ops::locations::remove::action::LocationRemoveAction {
								library_id,
								location_id: id,
							};

							// Dispatch the action
							match core.execute_library_action(action).await {
								Ok(_) => DaemonResponse::Ok,
								Err(e) => DaemonResponse::Error(format!(
									"Failed to remove location: {}",
									e
								)),
							}
						}
						None => DaemonResponse::Error("Action manager not available".to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::RescanLocation { id } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let library_id = library.id();

					// Get the action manager
					match core.context.get_action_manager().await {
						Some(action_manager) => {
							// Create LocationRescanAction
							let action = crate::ops::locations::rescan::action::LocationRescanAction {
								library_id,
								location_id: id,
								full_rescan: false,
							};

							// Dispatch the action
							match core.execute_library_action(action).await {
								Ok(_output) => {
									// For now, just return success
									DaemonResponse::Ok
								}
								Err(e) => DaemonResponse::Error(format!(
									"Failed to start rescan: {}",
									e
								)),
							}
						}
						None => DaemonResponse::Error("Action manager not available".to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			_ => DaemonResponse::Error("Invalid command for location handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::AddLocation { .. }
				| DaemonCommand::ListLocations
				| DaemonCommand::RemoveLocation { .. }
				| DaemonCommand::RescanLocation { .. }
		)
	}
}
