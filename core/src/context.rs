//! Shared context providing access to core application components.

use crate::{
	config::JobLoggingConfig,
	crypto::key_manager::KeyManager,
	device::DeviceManager,
	filetype::FileTypeRegistry,
	infra::action::manager::ActionManager,
	infra::event::EventBus,
	infra::source_dirs::SourceDirs,
	infra::sync::TransactionManager,
	library::LibraryManager,
	ops::indexing::startup::StartupIndexingGate,
	ops::indexing::VolumeIndex,
	ops::navigation::FocusRegistry,
	ops::processes::ProcessManager,
	service::external_tools::ExternalTools,
	service::network::{NetworkingService, RemoteJobCache},
	service::session::SessionStateService,
	service::thumbs::ThumbService,
	service::watcher::FsWatcherService,
	volume::VolumeManager,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct CoreContext {
	pub events: Arc<EventBus>,
	pub device_manager: Arc<DeviceManager>,
	pub library_manager: Arc<RwLock<Option<Arc<LibraryManager>>>>,
	pub volume_manager: Arc<VolumeManager>,
	pub key_manager: Arc<KeyManager>,
	// This is wrapped in an RwLock to allow it to be set after initialization
	pub action_manager: Arc<RwLock<Option<Arc<ActionManager>>>>,
	pub networking: Arc<RwLock<Option<Arc<NetworkingService>>>>,
	#[cfg(feature = "wasm")]
	pub plugin_manager: Arc<RwLock<Option<Arc<RwLock<crate::infra::extension::PluginManager>>>>>,
	pub fs_watcher: Arc<RwLock<Option<Arc<FsWatcherService>>>>,
	pub process_manager: Arc<RwLock<Option<Arc<ProcessManager>>>>,
	// Every attached drive mapped in memory, plus the source registry and stores
	pub volume_index: Arc<VolumeIndex>,
	// Where each client window is looking; in-memory, never persisted
	pub navigation_focus: Arc<FocusRegistry>,
	// One automatic discovery pass per library and daemon session
	pub startup_indexing: Arc<StartupIndexingGate>,
	// The thumbnail hot tier; owns every tile cache writer on this machine
	pub thumbs: Arc<ThumbService>,
	// Optional software discovered on this machine, shared by every consumer
	pub external_tools: Arc<ExternalTools>,
	// Remote job cache for cross-device job visibility
	pub remote_job_cache: Arc<RemoteJobCache>,
	// Job logging configuration
	pub job_logging_config: Option<JobLoggingConfig>,
	pub job_logs_dir: Option<PathBuf>,
	// Data directory path (for reset and cleanup operations)
	pub data_dir: PathBuf,
	// Previewed plans kept under handles for browsing; in-memory, never persisted
	pub plans: Arc<crate::ops::files::plan::PlanHandles>,
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
		crate::config::mark_own_data_dir(&data_dir);

		// The cache's persistence must follow this context's data dir, so
		// --data-dir/--instance daemons never read or write the default
		// installation's source registry and snapshots.
		let sources_dir = data_dir.join("sources");
		let volume_index = Arc::new(
			VolumeIndex::with_sources_dir(Some(sources_dir.clone()))
				.expect("Failed to create the volume index"),
		);
		let external_tools = Arc::new(ExternalTools::new());
		// The hot tier reads the same per-source layout the index writes into,
		// so a source's tiles sit beside its snapshot and its store.
		let thumbs = ThumbService::new(
			SourceDirs::new(sources_dir).ok(),
			volume_index.clone(),
			events.clone(),
			external_tools.clone(),
		);

		Self {
			events,
			device_manager,
			library_manager: Arc::new(RwLock::new(library_manager)),
			volume_manager,
			key_manager,
			action_manager: Arc::new(RwLock::new(None)),
			networking: Arc::new(RwLock::new(None)),
			#[cfg(feature = "wasm")]
			plugin_manager: Arc::new(RwLock::new(None)),
			fs_watcher: Arc::new(RwLock::new(None)),
			process_manager: Arc::new(RwLock::new(None)),
			volume_index,
			navigation_focus: Arc::new(FocusRegistry::new()),
			startup_indexing: Arc::new(StartupIndexingGate::default()),
			thumbs,
			external_tools,
			remote_job_cache: Arc::new(RemoteJobCache::new()),
			job_logging_config: None,
			job_logs_dir: None,
			data_dir,
			plans: Arc::new(crate::ops::files::plan::PlanHandles::default()),
		}
	}

	/// Get the volume index
	pub fn volume_index(&self) -> &Arc<VolumeIndex> {
		&self.volume_index
	}

	/// The file type registry every lookup shares: the built-in types plus
	/// the kinds of every loaded extension, swapped by the plugin manager.
	pub fn file_type_registry(&self) -> Arc<FileTypeRegistry> {
		FileTypeRegistry::current()
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

	pub async fn set_process_manager(&self, manager: Arc<ProcessManager>) {
		*self.process_manager.write().await = Some(manager);
	}

	/// Helper method to get the process manager
	pub async fn processes(&self) -> Option<Arc<ProcessManager>> {
		self.process_manager.read().await.clone()
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
}
