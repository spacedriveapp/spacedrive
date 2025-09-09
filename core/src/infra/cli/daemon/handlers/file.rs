//! File operation command handlers

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::infra::action::builder::{ActionBuildError, ActionBuilder};
use crate::infra::cli::daemon::services::StateService;
use crate::infra::cli::daemon::types::{DaemonCommand, DaemonResponse};
use crate::Core;

use super::CommandHandler;

/// Handler for file operation commands
pub struct FileHandler;

#[async_trait]
impl CommandHandler for FileHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::Index(mut input) => {
				// Execute typed indexing directly
				if let Some(library) = state_service.get_current_library(core).await {
					// Inject current library id
					input.library_id = library.id();
					let action = crate::ops::indexing::IndexingAction::new(input);
					match core.execute_library_action(action).await {
						Ok(_handle) => DaemonResponse::Ok,
						Err(e) => DaemonResponse::Error(format!("Indexing failed: {}", e)),
					}
				} else {
					DaemonResponse::Error(
						"No library available. Create or open a library first.".to_string(),
					)
				}
			}
			DaemonCommand::Copy(mut input) => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					// Inject current library id
					input.library_id = Some(library.id());

					// Build action from typed input
					let builder = crate::ops::files::copy::action::FileCopyActionBuilder::from_input(input);
					let action = match builder.build() {
						Ok(a) => a,
						Err(e) => return DaemonResponse::Error(format!("Failed to create copy action: {}", e)),
					};

					match core.execute_library_action(action).await {
						Ok(_job) => DaemonResponse::Ok,
						Err(e) => DaemonResponse::Error(format!("Failed to start copy operation: {}", e)),
					}
				} else {
					DaemonResponse::Error(
						"No library available. Create or open a library first.".to_string(),
					)
				}
			}
			DaemonCommand::LocationRescan(mut action) => {
				if let Some(library) = state_service.get_current_library(core).await {
					// Inject current library id
					action.library_id = library.id();
					match core.execute_library_action(action).await {
						Ok(_out) => DaemonResponse::Ok,
						Err(e) => DaemonResponse::Error(format!("Location rescan failed: {}", e)),
					}
				} else {
					DaemonResponse::Error("No library available. Create or open a library first.".to_string())
				}
			}

			// Indexing operations (legacy Browse removed in favor of typed Index)

			DaemonCommand::IndexAll { force } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let library_id = library.id();

					// Get the action manager
					match core.context.get_action_manager().await {
						Some(action_manager) => {
							// Create LocationManager
							let location_manager =
								crate::location::manager::LocationManager::new((*core.events).clone());

							// Get all locations for the library
							match location_manager.list_locations(&library).await {
								Ok(locations) => {
									if locations.is_empty() {
										return DaemonResponse::Error(
											"No locations found in library".to_string(),
										);
									}

									let location_count = locations.len();
									let mut success_count = 0;
									let mut errors = Vec::new();

									// Dispatch LocationRescanAction for each location
									for location in locations {
										let action = crate::ops::locations::rescan::action::LocationRescanAction {
											library_id,
											location_id: location.id,
											full_rescan: force,
										};

										match core.execute_library_action(action).await {
											Ok(_output) => {
												success_count += 1;
											}
											Err(e) => {
												errors.push(format!(
													"Location {}: {}",
													location.id, e
												));
											}
										}
									}

									if errors.is_empty() {
										DaemonResponse::Ok
									} else {
										DaemonResponse::Error(format!(
											"Indexed {}/{} locations. Errors: {}",
											success_count,
											location_count,
											errors.join("; ")
										))
									}
								}
								Err(e) => DaemonResponse::Error(format!(
									"Failed to list locations: {}",
									e
								)),
						}
					}
					None => DaemonResponse::Error("Action manager not available".to_string()),
					}
				} else {
					DaemonResponse::Error(
						"No library available. Create or open a library first.".to_string(),
					)
				}
			}

			DaemonCommand::IndexLocation { location, force } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let library_id = library.id();

					// Parse location ID
					match location.parse::<Uuid>() {
						Ok(location_id) => {
							// Get the action manager
							match core.context.get_action_manager().await {
								Some(action_manager) => {
									// Create LocationRescanAction
									let action = crate::ops::locations::rescan::action::LocationRescanAction {
										library_id,
										location_id,
										full_rescan: force,
									};

									// Dispatch the action
									match core.execute_library_action(action).await {
										Ok(_output) => {
											DaemonResponse::LocationIndexed { location_id }
										}
										Err(e) => DaemonResponse::Error(format!(
											"Failed to index location: {}",
											e
										)),
									}
								}
								None => DaemonResponse::Error(
									"Action manager not available".to_string(),
								),
							}
						}
						Err(_) => {
							DaemonResponse::Error(format!("Invalid location ID: {}", location))
						}
					}
				} else {
					DaemonResponse::Error(
						"No library available. Create or open a library first.".to_string(),
					)
				}
			}

			_ => DaemonResponse::Error("Invalid command for file handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::Copy { .. }
				| DaemonCommand::Browse { .. }
				| DaemonCommand::IndexAll { .. }
				| DaemonCommand::IndexLocation { .. }
				| DaemonCommand::LocationRescan { .. }
		)
	}
}
