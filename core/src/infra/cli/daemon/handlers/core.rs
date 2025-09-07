//! Core command handlers (ping, shutdown, status)

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

use crate::infra::cli::daemon::services::StateService;
use crate::infra::cli::daemon::types::{DaemonCommand, DaemonResponse, DaemonStatus};
use crate::Core;

use super::CommandHandler;

/// Handler for core daemon commands
pub struct CoreHandler {
	start_time: std::time::Instant,
	shutdown_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

impl CoreHandler {
	pub fn new(
		start_time: std::time::Instant,
		shutdown_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
	) -> Self {
		Self {
			start_time,
			shutdown_tx,
		}
	}
}

#[async_trait]
impl CommandHandler for CoreHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::Ping => DaemonResponse::Pong,

			DaemonCommand::Shutdown => {
				// Gracefully shutdown core (this will close all libraries and cleanup locks)
				if let Err(e) = core.shutdown().await {
					error!("Error during core shutdown: {}", e);
				}

				// Trigger daemon shutdown
				let mut shutdown_guard = self.shutdown_tx.lock().await;
				if let Some(tx) = shutdown_guard.take() {
					let _ = tx.send(());
				}

				DaemonResponse::Ok
			}

			DaemonCommand::GetStatus => {
				let current_library = state_service.get_current_library_id().await;

				// TODO: Get actual job and location counts
				DaemonResponse::Status(DaemonStatus {
					version: env!("CARGO_PKG_VERSION").to_string(),
					uptime_secs: self.start_time.elapsed().as_secs(),
					current_library,
					active_jobs: 0,     // TODO: Get from job manager
					total_locations: 0, // TODO: Get from location manager
				})
			}

			_ => DaemonResponse::Error("Invalid command for core handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::Ping | DaemonCommand::Shutdown | DaemonCommand::GetStatus
		)
	}
}
