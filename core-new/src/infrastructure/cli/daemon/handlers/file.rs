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
									// Extract job ID if available
									if let Some(job_id) =
										output.data.get("job_id").and_then(|v| v.as_str())
									{
										if let Ok(uuid) = job_id.parse::<Uuid>() {
											DaemonResponse::CopyStarted {
												job_id: uuid,
												sources_count: sources.len(),
											}
										} else {
											DaemonResponse::Ok
										}
									} else {
										DaemonResponse::Ok
									}
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
			DaemonCommand::QuickScan {
				path,
				scope,
				ephemeral,
			} => {
				// TODO: Implement quick scan
				DaemonResponse::Error("Quick scan not yet implemented".to_string())
			}

			DaemonCommand::Browse {
				path,
				scope,
				content,
			} => {
				// TODO: Implement browse
				DaemonResponse::Error("Browse not yet implemented".to_string())
			}

			DaemonCommand::IndexPath {
				path,
				mode,
				scope,
				depth,
				create_location,
			} => {
				// TODO: Implement index path
				DaemonResponse::Error("Index path not yet implemented".to_string())
			}

			DaemonCommand::IndexAll { force } => {
				// TODO: Implement index all
				DaemonResponse::Error("Index all not yet implemented".to_string())
			}

			DaemonCommand::IndexLocation { location, force } => {
				// TODO: Implement index location
				DaemonResponse::Error("Index location not yet implemented".to_string())
			}

			_ => DaemonResponse::Error("Invalid command for file handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::Copy { .. }
				| DaemonCommand::QuickScan { .. }
				| DaemonCommand::Browse { .. }
				| DaemonCommand::IndexPath { .. }
				| DaemonCommand::IndexAll { .. }
				| DaemonCommand::IndexLocation { .. }
		)
	}
}
