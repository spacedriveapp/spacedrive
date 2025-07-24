//! File operation command handlers

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::infrastructure::actions::builder::{ActionBuildError, ActionBuilder};
use crate::infrastructure::cli::daemon::services::StateService;
use crate::infrastructure::cli::daemon::types::{DaemonCommand, DaemonResponse};
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
			DaemonCommand::Copy {
				sources,
				destination,
				overwrite,
				verify,
				preserve_timestamps,
				move_files,
			} => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let library_id = library.id();

					// Create the copy input
					let input = crate::operations::files::copy::input::FileCopyInput {
						sources: sources.clone(),
						destination: destination.clone(),
						overwrite,
						verify_checksum: verify,
						preserve_timestamps,
						move_files,
						copy_method: crate::operations::files::copy::input::CopyMethod::Auto,
					};

					// Validate input
					if let Err(errors) = input.validate() {
						return DaemonResponse::Error(format!(
							"Invalid copy operation: {}",
							errors.join("; ")
						));
					}

					// Get the action manager
					match core.context.get_action_manager().await {
						Some(action_manager) => {
							// Create the copy action
							let action = match crate::operations::files::copy::action::FileCopyActionBuilder::from_input(input).build() {
								Ok(action) => action,
								Err(e) => {
									return DaemonResponse::Error(format!("Failed to build copy action: {}", e));
								}
							};

							// Create the full Action enum
							let full_action = crate::infrastructure::actions::Action::FileCopy {
								library_id,
								action,
							};

							// Dispatch the action
							match action_manager.dispatch(full_action).await {
								Ok(output) => {
									// For now, just return success
									// TODO: Extract job ID when FileCopy action returns it
									DaemonResponse::Ok
								}
								Err(e) => DaemonResponse::Error(format!(
									"Failed to start copy operation: {}",
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

			// Indexing operations
			DaemonCommand::Browse {
				path,
				scope,
				content,
			} => {
				// Browse is a read-only operation that doesn't persist anything
				// For now, we'll do a simple directory listing
				match std::fs::read_dir(&path) {
					Ok(entries) => {
						let mut browse_entries = Vec::new();
						let mut total_files = 0;
						let mut total_dirs = 0;

						for entry in entries.flatten() {
							let metadata = entry.metadata().ok();
							let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);

							if is_dir {
								total_dirs += 1;
							} else {
								total_files += 1;
							}

							let file_name = entry.file_name().to_string_lossy().to_string();
							let file_path = entry.path();

							let size = if !is_dir {
								metadata.as_ref().map(|m| m.len())
							} else {
								None
							};

							let modified =
								metadata
									.as_ref()
									.and_then(|m| m.modified().ok())
									.map(|time| {
										// Convert to human-readable format
										chrono::DateTime::<chrono::Utc>::from(time)
											.format("%Y-%m-%d %H:%M:%S")
											.to_string()
									});

							let file_type = if is_dir {
								Some("directory".to_string())
							} else {
								// Simple file type detection based on extension
								file_path
									.extension()
									.and_then(|ext| ext.to_str())
									.map(|ext| ext.to_lowercase())
							};

							browse_entries.push(
								crate::infrastructure::cli::daemon::types::common::BrowseEntry {
									name: file_name,
									path: file_path,
									is_dir,
									size,
									modified,
									file_type,
								},
							);
						}

						// Sort entries: directories first, then files, alphabetically
						browse_entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
							(true, false) => std::cmp::Ordering::Less,
							(false, true) => std::cmp::Ordering::Greater,
							_ => a.name.cmp(&b.name),
						});

						DaemonResponse::BrowseResults {
							path,
							entries: browse_entries,
							total_files,
							total_dirs,
						}
					}
					Err(e) => DaemonResponse::Error(format!("Failed to browse path: {}", e)),
				}
			}

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

									// Dispatch LocationIndexAction for each location
									for location in locations {
										let action = crate::infrastructure::actions::Action::LocationIndex {
											library_id,
											action: crate::operations::locations::index::action::LocationIndexAction {
												location_id: location.id,
												mode: location.index_mode.into(),
											},
										};

										match action_manager.dispatch(action).await {
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
									// Create LocationIndexAction
									let action = crate::infrastructure::actions::Action::LocationIndex {
										library_id,
										action: crate::operations::locations::index::action::LocationIndexAction {
											location_id,
											mode: crate::operations::indexing::IndexMode::Content,
										},
									};

									// Dispatch the action
									match action_manager.dispatch(action).await {
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
		)
	}
}
