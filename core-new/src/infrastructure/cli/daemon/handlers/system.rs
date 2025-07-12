//! System monitoring command handlers

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use crate::Core;
use crate::infrastructure::cli::daemon::services::StateService;
use crate::infrastructure::cli::daemon::types::{DaemonCommand, DaemonResponse};

use super::CommandHandler;

/// Handler for system monitoring commands
pub struct SystemHandler {
	data_dir: PathBuf,
}

impl SystemHandler {
	pub fn new(data_dir: PathBuf) -> Self {
		Self { data_dir }
	}
}

#[async_trait]
impl CommandHandler for SystemHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		_core: &Arc<Core>,
		_state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::SubscribeEvents => {
				// TODO: Implement event subscription
				DaemonResponse::Error("Event subscription not yet implemented".to_string())
			}

			_ => DaemonResponse::Error("Invalid command for system handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(cmd, DaemonCommand::SubscribeEvents)
	}
}