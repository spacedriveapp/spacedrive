//! Shared context providing access to core application components.

use crate::{
	config::JobLoggingConfig, crypto::library_key_manager::LibraryKeyManager,
	device::DeviceManager, infra::action::manager::ActionManager, infra::event::EventBus,
	infra::sync::TransactionManager, library::LibraryManager, service::network::NetworkingService,
	service::session::SessionStateService, service::sidecar_manager::SidecarManager,
	volume::VolumeManager,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, RwLock};

#[derive(Clone)]
pub struct CoreContext {
	pub events: Arc<EventBus>,
	pub device_manager: Arc<DeviceManager>,
	pub library_manager: Arc<RwLock<Option<Arc<LibraryManager>>>>,
	pub volume_manager: Arc<VolumeManager>,
	pub library_key_manager: Arc<LibraryKeyManager>,
	// This is wrapped in an RwLock to allow it to be set after initialization
	pub sidecar_manager: Arc<RwLock<Option<Arc<SidecarManager>>>>,
	pub action_manager: Arc<RwLock<Option<Arc<ActionManager>>>>,
	pub networking: Arc<RwLock<Option<Arc<NetworkingService>>>>,
	pub plugin_manager: Arc<RwLock<Option<Arc<RwLock<crate::infra::extension::PluginManager>>>>>,
	// Job logging configuration
	pub job_logging_config: Option<JobLoggingConfig>,
	pub job_logs_dir: Option<PathBuf>,
	// pub session: Arc<SessionStateService>,
}

impl CoreContext {
	/// Create a new context with the given components
	pub fn new(
		events: Arc<EventBus>,
		device_manager: Arc<DeviceManager>,
		library_manager: Option<Arc<LibraryManager>>,
		volume_manager: Arc<VolumeManager>,
		library_key_manager: Arc<LibraryKeyManager>,
	) -> Self {
		Self {
			events,
			device_manager,
			library_manager: Arc::new(RwLock::new(library_manager)),
			volume_manager,
			library_key_manager,
			sidecar_manager: Arc::new(RwLock::new(None)),
			action_manager: Arc::new(RwLock::new(None)),
			networking: Arc::new(RwLock::new(None)),
			plugin_manager: Arc::new(RwLock::new(None)),
			job_logging_config: None,
			job_logs_dir: None,
		}
	}

	/// Get the library manager
	pub async fn libraries(&self) -> Arc<LibraryManager> {
		self.library_manager.read().await.clone().unwrap()
	}

	/// Get a library by ID
	pub async fn get_library(&self, id: uuid::Uuid) -> Option<Arc<crate::library::Library>> {
		self.libraries().await.get_library(id).await
	}

	/// Get the primary library
	pub async fn get_primary_library(&self) -> Option<Arc<crate::library::Library>> {
		// TODO: Remove this function, for now a temp fix just get the first library
		// This is mostly used in the file sharing service
		self.libraries().await.list().await.first().cloned()
	}

	/// Method for Core to set library manager after it's initialized
	pub async fn set_libraries(&self, library_manager: Arc<LibraryManager>) {
		*self.library_manager.write().await = Some(library_manager);
	}

	/// Set job logging configuration
	pub fn set_job_logging(&mut self, config: JobLoggingConfig, logs_dir: Option<PathBuf>) {
		self.job_logging_config = Some(config);
		self.job_logs_dir = logs_dir;
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

	/// Method for Core to set plugin manager after it's initialized
	pub async fn set_plugin_manager(
		&self,
		plugin_manager: Arc<RwLock<crate::infra::extension::PluginManager>>,
	) {
		*self.plugin_manager.write().await = Some(plugin_manager);
	}

	/// Get plugin manager
	pub async fn get_plugin_manager(
		&self,
	) -> Option<Arc<RwLock<crate::infra::extension::PluginManager>>> {
		self.plugin_manager.read().await.clone()
	}

	/// Helper method to get the sidecar manager
	pub async fn get_sidecar_manager(&self) -> Option<Arc<SidecarManager>> {
		self.sidecar_manager.read().await.clone()
	}

	/// Method for Core to set sidecar manager after it's initialized
	pub async fn set_sidecar_manager(&self, sidecar_manager: Arc<SidecarManager>) {
		*self.sidecar_manager.write().await = Some(sidecar_manager);
	}
}
