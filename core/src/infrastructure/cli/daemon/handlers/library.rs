//! Library command handlers

use async_trait::async_trait;
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

use crate::Core;
use crate::infrastructure::cli::daemon::services::StateService;
use crate::infrastructure::cli::daemon::types::{
	DaemonCommand, DaemonResponse, LibraryInfo,
};

use super::CommandHandler;

/// Handler for library commands
pub struct LibraryHandler;

#[async_trait]
impl CommandHandler for LibraryHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::CreateLibrary { name, path } => {
				match core
					.libraries
					.create_library(&name, path, core.context.clone())
					.await
				{
					Ok(library) => {
						// Auto-select the newly created library
						let library_id = library.id();
						let library_path = library.path().to_path_buf();
						
						// Try to set the new library as current
						if let Err(e) = state_service
							.switch_library(library_id, library_path.clone())
							.await
						{
							warn!("Failed to auto-select new library: {}", e);
						}
						
						DaemonResponse::LibraryCreated {
							id: library_id,
							name: name.clone(),  // Use the name passed in instead of reading from library
							path: library_path,
						}
					},
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			}

			DaemonCommand::ListLibraries => {
				let libraries = core.libraries.list().await;
				let infos: Vec<LibraryInfo> =
					futures::future::join_all(libraries.into_iter().map(|lib| async move {
						LibraryInfo {
							id: lib.id(),
							name: lib.name().await,
							path: lib.path().to_path_buf(),
						}
					}))
					.await;

				DaemonResponse::Libraries(infos)
			}

			DaemonCommand::GetCurrentLibrary => {
				if let Some(library) = state_service.get_current_library(core).await {
					DaemonResponse::CurrentLibrary(Some(LibraryInfo {
						id: library.id(),
						name: library.name().await,
						path: library.path().to_path_buf(),
					}))
				} else {
					DaemonResponse::CurrentLibrary(None)
				}
			}

			DaemonCommand::SwitchLibrary { id } => {
				let libraries = core.libraries.list().await;
				if let Some(library) = libraries.iter().find(|lib| lib.id() == id) {
					match state_service
						.switch_library(library.id(), library.path().to_path_buf())
						.await
					{
						Ok(_) => DaemonResponse::Ok,
						Err(e) => {
							warn!("Failed to save CLI state: {}", e);
							DaemonResponse::Ok // Still return Ok as the switch was successful
						}
					}
				} else {
					DaemonResponse::Error("Library not found".to_string())
				}
			}

			_ => DaemonResponse::Error("Invalid command for library handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::CreateLibrary { .. }
				| DaemonCommand::ListLibraries
				| DaemonCommand::GetCurrentLibrary
				| DaemonCommand::SwitchLibrary { .. }
		)
	}
}