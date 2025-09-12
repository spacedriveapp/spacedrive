//! Shared context providing access to core application components.

//! Shared context providing access to core application components.

use crate::{config::JobLoggingConfig, device::DeviceManager, infra::action::manager::ActionManager,
	infra::event::EventBus, crypto::library_key_manager::LibraryKeyManager, library::LibraryManager,
	service::network::NetworkingService, volume::VolumeManager, infra::daemon::state::SessionStateService,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

/// Shared context providing access to core application components.
#[derive(Clone)]
pub struct CoreContext {
	pub events: Arc<EventBus>,
	pub device_manager: Arc<DeviceManager>,
	pub library_manager: Arc<LibraryManager>,
	pub volume_manager: Arc<VolumeManager>,
	pub library_key_manager: Arc<LibraryKeyManager>,
	// This is wrapped in an RwLock to allow it to be set after initialization
	pub action_manager: Arc<RwLock<Option<Arc<ActionManager>>>>,
	pub networking: Arc<RwLock<Option<Arc<NetworkingService>>>>,
	// Job logging configuration
	pub job_logging_config: Option<JobLoggingConfig>,
	pub job_logs_dir: Option<PathBuf>,
    pub session_state: Arc<SessionStateService>,
}

impl CoreContext {
	/// Create a new context with the given components
	pub fn new(
		events: Arc<EventBus>,
		device_manager: Arc<DeviceManager>,
		library_manager: Arc<LibraryManager>,
		volume_manager: Arc<VolumeManager>,
		library_key_manager: Arc<LibraryKeyManager>,
        session_state: Arc<SessionStateService>,
	) -> Self {
		Self {
			events,
			device_manager,
			library_manager,
			volume_manager,
			library_key_manager,
			action_manager: Arc::new(RwLock::new(None)),
			networking: Arc::new(RwLock::new(None)),
			job_logging_config: None,
			job_logs_dir: None,
            session_state,
		}
	}

	/// Set job logging configuration
	pub fn set_job_logging(&mut self, config: JobLoggingConfig, logs_dir: PathBuf) {
		self.job_logging_config = Some(config);
		self.job_logs_dir = Some(logs_dir);
	}

	/// Helper method for services to get the networking service
	pub async fn get_networking(&self) -> Option<Arc<NetworkingService>> {
		self.networking.read().await.clone()
	}

	/// Method for Core to set networking after it's initialized
	pub async fn set_networking(&self, networking: Arc<NetworkingService>) {
		*self.networking.write().await = Some(networking);
	}

	/// Helper method to get the action manager
	pub async fn get_action_manager(&self) -> Option<Arc<ActionManager>> {
		self.action_manager.read().await.clone()
	}

	/// Method for Core to set action manager after it's initialized
	pub async fn set_action_manager(&self, action_manager: Arc<ActionManager>) {
		*self.action_manager.write().await = Some(action_manager);
	}
}
