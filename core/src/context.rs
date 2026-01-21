//! Shared context providing access to core application components.

use crate::{
	config::JobLoggingConfig,
	crypto::key_manager::KeyManager,
	device::DeviceManager,
	filetype::FileTypeRegistry,
	infra::action::manager::ActionManager,
	infra::event::EventBus,
	infra::sync::TransactionManager,
	library::LibraryManager,
	ops::indexing::ephemeral::EphemeralIndexCache,
	service::network::{NetworkingService, RemoteJobCache},
	service::session::SessionStateService,
	service::sidecar_manager::SidecarManager,
	service::watcher::FsWatcherService,
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
	pub key_manager: Arc<KeyManager>,
	// This is wrapped in an RwLock to allow it to be set after initialization
	pub sidecar_manager: Arc<RwLock<Option<Arc<SidecarManager>>>>,
	pub action_manager: Arc<RwLock<Option<Arc<ActionManager>>>>,
	pub networking: Arc<RwLock<Option<Arc<NetworkingService>>>>,
	#[cfg(feature = "wasm")]
	pub plugin_manager: Arc<RwLock<Option<Arc<RwLock<crate::infra::extension::PluginManager>>>>>,
	pub fs_watcher: Arc<RwLock<Option<Arc<FsWatcherService>>>>,
	// Ephemeral index cache for unmanaged paths
	pub ephemeral_index_cache: Arc<EphemeralIndexCache>,
	// Remote job cache for cross-device job visibility
	pub remote_job_cache: Arc<RemoteJobCache>,
	// File type registry (loaded once at startup, never changes)
	pub file_type_registry: Arc<FileTypeRegistry>,
	// Job logging configuration
	pub job_logging_config: Option<JobLoggingConfig>,
	pub job_logs_dir: Option<PathBuf>,
	// Data directory path (for reset and cleanup operations)
	pub data_dir: PathBuf,
}

impl CoreContext {
	/// Create a new context with the given components
	pub fn new(
		events: Arc<EventBus>,
		device_manager: Arc<DeviceManager>,
		library_manager: Option<Arc<LibraryManager>>,
		volume_manager: Arc<VolumeManager>,
		key_manager: Arc<KeyManager>,
		data_dir: PathBuf,
	) -> Self {
		Self {
			events,
			device_manager,
			library_manager: Arc::new(RwLock::new(library_manager)),
			volume_manager,
			key_manager,
			sidecar_manager: Arc::new(RwLock::new(None)),
			action_manager: Arc::new(RwLock::new(None)),
			networking: Arc::new(RwLock::new(None)),
			#[cfg(feature = "wasm")]
			plugin_manager: Arc::new(RwLock::new(None)),
			fs_watcher: Arc::new(RwLock::new(None)),
			ephemeral_index_cache: Arc::new(
				EphemeralIndexCache::new().expect("Failed to create ephemeral index cache"),
			),
			remote_job_cache: Arc::new(RemoteJobCache::new()),
			file_type_registry: Arc::new(FileTypeRegistry::new()),
			job_logging_config: None,
			job_logs_dir: None,
			data_dir,
		}
	}

	/// Get the ephemeral index cache
	pub fn ephemeral_cache(&self) -> &Arc<EphemeralIndexCache> {
		&self.ephemeral_index_cache
	}

	/// Get the file type registry
	pub fn file_type_registry(&self) -> &Arc<FileTypeRegistry> {
		&self.file_type_registry
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

	/// Helper method for services to get the filesystem watcher
	pub async fn get_fs_watcher(&self) -> Option<Arc<FsWatcherService>> {
		self.fs_watcher.read().await.clone()
	}

	/// Method for Core to set filesystem watcher after it's initialized
	pub async fn set_fs_watcher(&self, watcher: Arc<FsWatcherService>) {
		*self.fs_watcher.write().await = Some(watcher);
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
	#[cfg(feature = "wasm")]
	pub async fn set_plugin_manager(
		&self,
		plugin_manager: Arc<RwLock<crate::infra::extension::PluginManager>>,
	) {
		*self.plugin_manager.write().await = Some(plugin_manager);
	}

	/// Get plugin manager
	#[cfg(feature = "wasm")]
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
