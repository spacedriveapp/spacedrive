//! Command handlers for the daemon

use async_trait::async_trait;
use std::sync::Arc;

use crate::Core;
use crate::infra::cli::daemon::services::StateService;
use crate::infra::cli::daemon::types::{DaemonCommand, DaemonResponse};

pub mod core;
pub mod file;
pub mod job;
pub mod library;
pub mod location;
pub mod network;
pub mod system;
pub mod volume;

pub use self::core::CoreHandler;
pub use file::FileHandler;
pub use job::JobHandler;
pub use library::LibraryHandler;
pub use location::LocationHandler;
pub use network::NetworkHandler;
pub use system::SystemHandler;
pub use volume::VolumeHandler;

/// Trait for command handlers
#[async_trait]
pub trait CommandHandler: Send + Sync {
	/// Handle a command and return a response
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse;

	/// Check if this handler can handle the given command
	fn can_handle(&self, cmd: &DaemonCommand) -> bool;
}

/// Registry for command handlers
pub struct HandlerRegistry {
	handlers: Vec<Box<dyn CommandHandler>>,
}

impl HandlerRegistry {
	/// Create a new handler registry with all handlers
	pub fn new(
		start_time: std::time::Instant,
		shutdown_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
		data_dir: std::path::PathBuf,
	) -> Self {
		let handlers: Vec<Box<dyn CommandHandler>> = vec![
			Box::new(CoreHandler::new(start_time, shutdown_tx)),
			Box::new(LibraryHandler),
			Box::new(LocationHandler),
			Box::new(JobHandler),
			Box::new(FileHandler),
			Box::new(NetworkHandler),
			Box::new(SystemHandler::new(data_dir)),
			Box::new(VolumeHandler),
		];

		Self { handlers }
	}

	/// Handle a command by finding the appropriate handler
	pub async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		for handler in &self.handlers {
			if handler.can_handle(&cmd) {
				return handler.handle(cmd, core, state_service).await;
			}
		}

		DaemonResponse::Error("No handler found for command".to_string())
	}
}