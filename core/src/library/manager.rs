//! Library manager - handles creation, opening, and discovery of libraries

use super::{
	config::{LibraryConfig, LibrarySettings, LibraryStatistics, ThumbnailMetadata},
	error::{LibraryError, Result},
	lock::LibraryLock,
	Library, LIBRARY_CONFIG_VERSION, LIBRARY_EXTENSION,
};

/// Legacy database filename (for migration)
const LEGACY_DB_FILENAME: &str = "database.db";

use super::LIBRARY_DB_FILENAME;
use crate::{
	context::CoreContext,
	device::DeviceManager,
	infra::{
		db::{entities, Database},
		event::{Event, EventBus, LibraryCreationSource},
		job::manager::JobManager,
	},
	service::session::SessionStateService,
	volume::VolumeManager,
};
use chrono::Utc;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Information about a discovered library
#[derive(Debug, Clone)]
pub struct DiscoveredLibrary {
	/// Path to the library directory
	pub path: PathBuf,

	/// Library configuration
	pub config: LibraryConfig,

	/// Whether the library is currently locked
	pub is_locked: bool,
}

/// Manages all Spacedrive libraries
pub struct LibraryManager {
	/// Currently open libraries
	libraries: Arc<RwLock<HashMap<Uuid, Arc<Library>>>>,

	/// Paths to search for libraries
	search_paths: Vec<PathBuf>,

	/// Event bus for library events
	event_bus: Arc<EventBus>,

	/// Dependencies needed from core
	// session: Arc<SessionStateService>,
	volume_manager: Arc<VolumeManager>,
	device_manager: Arc<DeviceManager>,

	/// Filesystem watcher for detecting library changes
	watcher: Arc<RwLock<Option<RecommendedWatcher>>>,

	/// Whether filesystem watching is active
	is_watching: Arc<RwLock<bool>>,

	/// Core context (needed for opening libraries on filesystem events)
	context: Arc<RwLock<Option<Arc<CoreContext>>>>,
}

impl LibraryManager {
	/// Create a new library manager
	pub fn new(
		event_bus: Arc<EventBus>,
		volume_manager: Arc<VolumeManager>,
		device_manager: Arc<DeviceManager>,
	) -> Self {
		// Default search paths
		let mut search_paths = vec![];

		// Add user's home directory
		if let Some(home) = dirs::home_dir() {
			search_paths.push(home.join("Spacedrive").join("Libraries"));
		}

		Self {
			libraries: Arc::new(RwLock::new(HashMap::new())),
			search_paths,
			event_bus,
			volume_manager,
			device_manager,
			watcher: Arc::new(RwLock::new(None)),
			is_watching: Arc::new(RwLock::new(false)),
			context: Arc::new(RwLock::new(None)),
		}
	}

	/// Create a new library manager with a specific libraries directory
	pub fn new_with_dir(
		libraries_dir: PathBuf,
		event_bus: Arc<EventBus>,
		volume_manager: Arc<VolumeManager>,
		device_manager: Arc<DeviceManager>,
	) -> Self {
		let search_paths = vec![libraries_dir];

		Self {
			libraries: Arc::new(RwLock::new(HashMap::new())),
			search_paths,
			event_bus,
			volume_manager,
			device_manager,
			watcher: Arc::new(RwLock::new(None)),
			is_watching: Arc::new(RwLock::new(false)),
			context: Arc::new(RwLock::new(None)),
		}
	}

	/// Add a search path for libraries
	pub fn add_search_path(&mut self, path: PathBuf) {
		if !self.search_paths.contains(&path) {
			self.search_paths.push(path);
		}
	}

	/// Create a new library
	pub async fn create_library(
		&self,
		name: impl Into<String>,
		location: Option<PathBuf>,
		context: Arc<CoreContext>,
	) -> Result<Arc<Library>> {
		self.create_library_internal(name, location, context, true)
			.await
	}

	/// Create a library without auto-initializing sync (for testing)
	pub async fn create_library_no_sync(
		&self,
		name: impl Into<String>,
		location: Option<PathBuf>,
		context: Arc<CoreContext>,
	) -> Result<Arc<Library>> {
		self.create_library_internal(name, location, context, false)
			.await
	}

	/// Create a shared library with a specific UUID (for library sync)
	///
	/// Used when a remote device requests this device to create a library
	/// with the same UUID for syncing purposes
	pub async fn create_library_with_id(
		&self,
		library_id: Uuid,
		name: impl Into<String>,
		description: Option<String>,
		context: Arc<CoreContext>,
	) -> Result<Arc<Library>> {
		let name = name.into();

		// Validate name
		if name.is_empty() {
			return Err(LibraryError::InvalidName(
				"Name cannot be empty".to_string(),
			));
		}

		// Sanitize name for filesystem
		let safe_name = sanitize_filename(&name);

		// Use default library location
		let base_path = self.search_paths.first().cloned().unwrap_or_else(|| {
			dirs::home_dir()
				.unwrap_or_else(|| PathBuf::from("."))
				.join("Spacedrive")
				.join("Libraries")
		});

		// Ensure base path exists
		tokio::fs::create_dir_all(&base_path).await.map_err(|e| {
			LibraryError::IoError(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("Failed to create libraries directory: {}", e),
			))
		})?;

		// Find unique library path
		let library_path = find_unique_library_path(&base_path, &safe_name).await?;

		// Create library directory
		tokio::fs::create_dir_all(&library_path).await?;

		// Initialize library with provided UUID (instead of generating new one)
		self.initialize_library_with_id(
			&library_path,
			library_id,
			name,
			description,
			context.clone(),
		)
		.await?;

		// Open the newly created library
		let library = self.open_library(&library_path, context).await?;

		// Emit event
		self.event_bus.emit(Event::LibraryCreated {
			id: library.id(),
			name: library.name().await,
			path: library_path.clone(),
			source: LibraryCreationSource::Manual,
		});

		Ok(library)
	}

	/// Create library with specific UUID and pre-register an initial device
	/// Used when creating a shared library - the requesting device is pre-registered
	/// so the current device can detect slug collisions and rename itself
	pub async fn create_library_with_id_and_initial_device(
		&self,
		library_id: Uuid,
		name: impl Into<String>,
		description: Option<String>,
		initial_device_id: Uuid,
		initial_device_name: String,
		initial_device_slug: String,
		context: Arc<CoreContext>,
	) -> Result<Arc<Library>> {
		let name = name.into();

		// Validate name
		if name.is_empty() {
			return Err(LibraryError::InvalidName(
				"Name cannot be empty".to_string(),
			));
		}

		// Sanitize name for filesystem
		let safe_name = sanitize_filename(&name);

		// Use default library location
		let base_path = self.search_paths.first().cloned().unwrap_or_else(|| {
			dirs::home_dir()
				.unwrap_or_else(|| PathBuf::from("."))
				.join("Spacedrive")
				.join("Libraries")
		});

		// Ensure base path exists
		tokio::fs::create_dir_all(&base_path).await.map_err(|e| {
			LibraryError::IoError(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("Failed to create libraries directory: {}", e),
			))
		})?;

		// Find unique library path
		let library_path = find_unique_library_path(&base_path, &safe_name).await?;

		// Create library directory
		tokio::fs::create_dir_all(&library_path).await?;

		// Initialize library with provided UUID
		self.initialize_library_with_id(
			&library_path,
			library_id,
			name.clone(),
			description,
			context.clone(),
		)
		.await?;

		// Pre-register the initial device BEFORE opening the library
		// This ensures when ensure_device_registered runs, it detects the collision
		let db_path = library_path.join(LIBRARY_DB_FILENAME);
		let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
		let db_conn = sea_orm::Database::connect(&db_url)
			.await
			.map_err(LibraryError::DatabaseError)?;

		use crate::infra::db::entities;
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, Set};

		let initial_device_model = entities::device::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(initial_device_id),
			name: Set(initial_device_name),
			slug: Set(initial_device_slug),
			os: Set("Desktop".to_string()),
			os_version: Set(None),
			hardware_model: Set(None),
			// Hardware specs - not available for pre-registered devices
			cpu_model: Set(None),
			cpu_architecture: Set(None),
			cpu_cores_physical: Set(None),
			cpu_cores_logical: Set(None),
			cpu_frequency_mhz: Set(None),
			memory_total_bytes: Set(None),
			form_factor: Set(None),
			manufacturer: Set(None),
			gpu_models: Set(None),
			boot_disk_type: Set(None),
			boot_disk_capacity_bytes: Set(None),
			swap_total_bytes: Set(None),
			network_addresses: Set(serde_json::json!([])),
			is_online: Set(false),
			last_seen_at: Set(Utc::now()),
			capabilities: Set(serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			})),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
			sync_enabled: Set(true),
			last_sync_at: Set(None),
		};

		initial_device_model
			.insert(&db_conn)
			.await
			.map_err(LibraryError::DatabaseError)?;

		info!(
			"Pre-registered requesting device {} in library {}",
			initial_device_id, library_id
		);

		// Close the temporary connection
		drop(db_conn);

		// Now open the library (which will call ensure_device_registered for current device)
		let library = self.open_library(&library_path, context).await?;

		// Create default space with Quick Access group
		self.create_default_space(&library).await?;

		// Emit event - this is a synced library from another device
		self.event_bus.emit(Event::LibraryCreated {
			id: library.id(),
			name: library.name().await,
			path: library_path.clone(),
			source: LibraryCreationSource::Sync,
		});

		Ok(library)
	}

	/// Internal library creation with optional sync init
	async fn create_library_internal(
		&self,
		name: impl Into<String>,
		location: Option<PathBuf>,
		context: Arc<CoreContext>,
		auto_init_sync: bool,
	) -> Result<Arc<Library>> {
		let name = name.into();

		// Validate name
		if name.is_empty() {
			return Err(LibraryError::InvalidName(
				"Name cannot be empty".to_string(),
			));
		}

		// Sanitize name for filesystem
		let safe_name = sanitize_filename(&name);

		// Determine base path
		let base_path = location.unwrap_or_else(|| {
			self.search_paths.first().cloned().unwrap_or_else(|| {
				dirs::home_dir()
					.unwrap_or_else(|| PathBuf::from("."))
					.join("Spacedrive")
					.join("Libraries")
			})
		});

		// Ensure base path exists
		tokio::fs::create_dir_all(&base_path).await.map_err(|e| {
			LibraryError::IoError(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("Failed to create libraries directory: {}", e),
			))
		})?;

		// Find unique library path
		let library_path = find_unique_library_path(&base_path, &safe_name).await?;

		// Create library directory
		tokio::fs::create_dir_all(&library_path).await?;

		// Initialize library
		self.initialize_library(&library_path, name.to_string(), context.clone())
			.await?;

		// Open the newly created library
		let library = self.open_library(&library_path, context.clone()).await?;

		// Create default space with Quick Access group
		self.create_default_space(&library).await?;

		// Emit event
		self.event_bus.emit(Event::LibraryCreated {
			id: library.id(),
			name: library.name().await,
			path: library_path,
			source: LibraryCreationSource::Manual,
		});

		Ok(library)
	}

	/// Open a library from a path
	pub async fn open_library(
		&self,
		path: impl AsRef<Path>,
		context: Arc<CoreContext>,
	) -> Result<Arc<Library>> {
		let path = path.as_ref();
		info!("Opening library at {:?}", path);

		// Validate it's a library directory
		if !is_library_directory(path) {
			return Err(LibraryError::NotALibrary(path.to_path_buf()));
		}

		// Acquire lock
		let lock = LibraryLock::acquire(path)?;

		// Load config
		let config_path = path.join("library.json");
		let config = LibraryConfig::load(&config_path).await?;

		// Ensure library ID is set
		if config.id.is_nil() {
			return Err(LibraryError::Other("Library config has nil ID".to_string()));
		}

		// Check if already open
		{
			let libraries = self.libraries.read().await;
			if libraries.contains_key(&config.id) {
				return Err(LibraryError::AlreadyOpen(config.id));
			}
		}

		// Migrate old database.db to library.db if needed
		let old_db_path = path.join(LEGACY_DB_FILENAME);
		let new_db_path = path.join(LIBRARY_DB_FILENAME);

		if old_db_path.exists() {
			if new_db_path.exists() {
				return Err(LibraryError::Other(
					"Both database.db and library.db exist. Please manually delete one."
						.to_string(),
				));
			}

			info!("Migrating database.db to library.db");
			tokio::fs::rename(&old_db_path, &new_db_path)
				.await
				.map_err(|e| LibraryError::Other(format!("Failed to rename database: {}", e)))?;

			// Also rename WAL and SHM files if they exist
			let old_wal = path.join("database.db-wal");
			let new_wal = path.join("library.db-wal");
			if old_wal.exists() {
				let _ = tokio::fs::rename(&old_wal, &new_wal).await;
			}

			let old_shm = path.join("database.db-shm");
			let new_shm = path.join("library.db-shm");
			if old_shm.exists() {
				let _ = tokio::fs::rename(&old_shm, &new_shm).await;
			}
		}

		// Open database
		let db_path = new_db_path;
		let db = Arc::new(Database::open(&db_path).await?);

		// Run migrations to ensure schema is up to date
		db.migrate().await?;

		// Get this device's ID for sync coordination
		let device_id = context
			.device_manager
			.device_id()
			.map_err(|e| LibraryError::Other(format!("Failed to get device ID: {}", e)))?;

		// Create dedicated sync event bus (separate from general event bus)
		let sync_events = Arc::new(crate::infra::sync::SyncEventBus::new());

		// Create transaction manager with both event buses
		let transaction_manager = Arc::new(crate::infra::sync::TransactionManager::new(
			sync_events.clone(),
			self.event_bus.clone(),
		));

		// Create job manager with context
		let job_manager =
			Arc::new(JobManager::new(path.to_path_buf(), context.clone(), config.id).await?);
		job_manager.initialize().await?;

		// Load device cache from library database
		let device_cache = Library::load_device_cache_from_db(&db).await?;

		// Create library instance
		let library = Arc::new(Library {
			path: path.to_path_buf(),
			config: Arc::new(RwLock::new(config.clone())),
			core_context: context.clone(),
			db,
			jobs: job_manager,
			event_bus: self.event_bus.clone(),
			sync_events,
			transaction_manager,
			sync_service: OnceCell::new(),      // Initialized later
			file_sync_service: OnceCell::new(), // Initialized later
			device_cache: Arc::new(std::sync::RwLock::new(device_cache)),
			_lock: lock,
		});

		// Ensure device is registered in this library
		if let Err(e) = self.ensure_device_registered(&library).await {
			warn!("Failed to register device in library {}: {}", config.id, e);
		} else {
			// Reload cache after device registration
			if let Err(e) = library.reload_device_cache().await {
				warn!(
					"Failed to reload device cache after registration for {}: {}",
					config.id, e
				);
			}
		}

		// Register library
		{
			let mut libraries = self.libraries.write().await;
			libraries.insert(config.id, library.clone());
		}

		// Initialize sidecar manager before resuming jobs
		if let Some(sidecar_manager) = context.get_sidecar_manager().await {
			if let Err(e) = sidecar_manager.init_library(&library).await {
				error!(
					"Failed to initialize sidecar manager for library {}: {}",
					config.id, e
				);
			}
		} else {
			warn!("Sidecar manager not available during library open");
		}

		// Now that the library is registered and sidecar manager is initialized, resume interrupted jobs
		// DISABLED: Jobs will remain paused on startup instead of auto-resuming
		// if let Err(e) = library.jobs.resume_interrupted_jobs_after_load().await {
		// 	warn!(
		// 		"Failed to resume interrupted jobs for library {}: {}",
		// 		config.id, e
		// 	);
		// }

		// Initialize sync service if networking is available
		// If networking isn't ready, sync simply won't be initialized until caller does it explicitly
		// TODO: maybe consider checking if networking is enabled rather than just checking if it's available
		if let Some(networking) = context.networking.read().await.as_ref() {
			if let Err(e) = library
				.init_sync_service(device_id, networking.clone())
				.await
			{
				warn!(
					"Failed to initialize sync service for library {}: {}",
					config.id, e
				);
			} else {
				// Wire up network event receiver to PeerSync for connection tracking
				if let Some(sync_service) = library.sync_service() {
					let peer_sync = sync_service.peer_sync();
					let network_events = networking.subscribe_events();
					peer_sync.set_network_events(network_events).await;
					info!(
						"Network event receiver wired to PeerSync for library {}",
						config.id
					);

					// Register library with sync multiplexer
					networking
						.sync_multiplexer()
						.register_library(
							config.id,
							peer_sync.clone(),
							sync_service.backfill_manager().clone(),
						)
						.await;
					info!("Library {} registered with sync multiplexer", config.id);
				}
			}
		} else {
			info!(
				"NetworkingService not available, sync service will be initialized later when networking is ready"
			);
		}

		// Auto-track user-relevant volumes for this library
		info!(
			"Auto-tracking user-relevant volumes for library {}",
			config.name
		);
		if let Err(e) = self.volume_manager.auto_track_user_volumes(&library).await {
			warn!("Failed to auto-track user-relevant volumes: {}", e);
		}

		// Emit event
		let library_name = config.name.clone();
		self.event_bus.emit(Event::LibraryOpened {
			id: config.id,
			name: config.name,
			path: path.to_path_buf(),
		});

		info!("Opened library {} at {:?}", library.id(), path);

		Ok(library)
	}

	/// Close a library
	pub async fn close_library(&self, id: Uuid) -> Result<()> {
		let library = {
			let mut libraries = self.libraries.write().await;
			libraries.remove(&id)
		};

		if let Some(library) = library {
			let name = library.name().await;

			// Shutdown the library gracefully
			if let Err(e) = library.shutdown().await {
				error!("Error during library shutdown: {}", e);
				// Continue with close even if shutdown has errors
			}

			// Emit event
			self.event_bus.emit(Event::LibraryClosed { id, name });

			info!("Closed library {}", id);
			Ok(())
		} else {
			Err(LibraryError::NotFound(id.to_string()))
		}
	}

	/// Get an open library by ID
	pub async fn get_library(&self, id: Uuid) -> Option<Arc<Library>> {
		self.libraries.read().await.get(&id).cloned()
	}

	/// Get all open libraries
	pub async fn get_open_libraries(&self) -> Vec<Arc<Library>> {
		self.libraries.read().await.values().cloned().collect()
	}

	/// List all open libraries
	pub async fn list(&self) -> Vec<Arc<Library>> {
		self.get_open_libraries().await
	}

	/// Load all libraries from the search paths
	pub async fn load_all(&self, context: Arc<CoreContext>) -> Result<usize> {
		let mut loaded_count = 0;

		info!(
			"Searching for libraries in {} paths",
			self.search_paths.len()
		);
		for search_path in &self.search_paths.clone() {
			info!("Checking search path: {:?}", search_path);
			if !search_path.exists() {
				info!("Search path {:?} does not exist, skipping", search_path);
				continue;
			}

			match tokio::fs::read_dir(search_path).await {
				Ok(mut entries) => {
					let mut entry_count = 0;
					while let Some(entry) = entries.next_entry().await? {
						entry_count += 1;
						let path = entry.path();
						info!("Found entry: {:?}", path);

						if is_library_directory(&path) {
							info!("Entry is a library directory: {:?}", path);
							match self.open_library(&path, context.clone()).await {
								Ok(_) => {
									loaded_count += 1;
									info!("Auto-loaded library from {:?}", path);
								}
								Err(LibraryError::AlreadyOpen(_)) => {
									// Library is already open, skip
									info!("Library already open, skipping: {:?}", path);
								}
								Err(e) => {
									// Try to load config to get library ID for the event
									let library_id =
										LibraryConfig::load(&path.join("library.json"))
											.await
											.ok()
											.map(|config| config.id);

									// Determine error type for frontend categorization
									let error_type = match &e {
										LibraryError::DatabaseError(_) => "DatabaseError",
										LibraryError::ConfigError(_) => "ConfigError",
										LibraryError::NotALibrary(_) => "NotALibrary",
										LibraryError::AlreadyInUse => "AlreadyInUse",
										LibraryError::StaleLock => "StaleLock",
										_ => "Unknown",
									}
									.to_string();

									error!(
										"Failed to load library from {:?}: {}. \
										 Library will be monitored and auto-loaded when issue is resolved.",
										path, e
									);

									// Emit event for frontend notification
									self.event_bus.emit(Event::LibraryLoadFailed {
										id: library_id,
										path: path.clone(),
										error: e.to_string(),
										error_type,
									});
								}
							}
						} else {
							info!("Entry is not a library directory: {:?}", path);
						}
					}
					info!("Found {} entries in {:?}", entry_count, search_path);
				}
				Err(e) => {
					warn!("Failed to read directory {:?}: {}", search_path, e);
				}
			}
		}

		Ok(loaded_count)
	}

	/// Close all open libraries
	pub async fn close_all(&self) -> Result<()> {
		let library_ids: Vec<Uuid> = self.libraries.read().await.keys().cloned().collect();

		for id in library_ids {
			if let Err(e) = self.close_library(id).await {
				error!("Failed to close library {}: {}", id, e);
			}
		}

		Ok(())
	}

	/// Scan search paths for libraries
	pub async fn scan_for_libraries(&self) -> Result<Vec<DiscoveredLibrary>> {
		let mut discovered = Vec::new();

		for search_path in &self.search_paths {
			if !search_path.exists() {
				continue;
			}

			let mut entries = tokio::fs::read_dir(search_path).await?;

			while let Some(entry) = entries.next_entry().await? {
				let path = entry.path();

				if is_library_directory(&path) {
					match self.read_library_info(&path).await {
						Ok(info) => discovered.push(info),
						Err(e) => {
							error!("Failed to read library at {:?}: {}", path, e);
						}
					}
				}
			}
		}

		Ok(discovered)
	}

	/// Count .sdlibrary directories in search paths without attempting to load them
	pub async fn count_library_directories(&self) -> usize {
		let mut count = 0;

		for search_path in &self.search_paths {
			if !search_path.exists() {
				continue;
			}

			match tokio::fs::read_dir(search_path).await {
				Ok(mut entries) => {
					while let Some(entry) = entries.next_entry().await.ok().flatten() {
						if is_library_directory(&entry.path()) {
							count += 1;
						}
					}
				}
				Err(e) => {
					warn!("Failed to read directory {:?}: {}", search_path, e);
				}
			}
		}

		count
	}

	/// Initialize a new library directory
	async fn initialize_library(
		&self,
		path: &Path,
		name: String,
		context: Arc<CoreContext>,
	) -> Result<()> {
		// Create subdirectories
		tokio::fs::create_dir_all(path.join("previews")).await?;
		tokio::fs::create_dir_all(path.join("exports")).await?;
		// Virtual Sidecar root (for derivative data linked by Entry/Content IDs)
		tokio::fs::create_dir_all(path.join("sidecars")).await?;

		// Create configuration
		let config = LibraryConfig {
			version: LIBRARY_CONFIG_VERSION,
			id: Uuid::new_v4(),
			name,
			description: None,
			created_at: Utc::now(),
			updated_at: Utc::now(),
			settings: LibrarySettings::default(),
			statistics: LibraryStatistics::default(),
		};

		// Initialize encryption key
		context
			.key_manager
			.get_library_key(config.id)
			.await
			.map_err(|e| {
				LibraryError::Other(format!(
					"Failed to initialize library encryption key: {}",
					e
				))
			})?;

		info!("Initialized encryption key for library '{}'", config.name);

		// Save configuration
		let config_path = path.join("library.json");
		let json = serde_json::to_string_pretty(&config)?;
		tokio::fs::write(config_path, json).await?;

		// Initialize database
		let db_path = path.join(LIBRARY_DB_FILENAME);
		let db = Database::create(&db_path).await?;

		// Run initial migrations
		db.migrate().await?;

		info!("Library '{}' initialized at {:?}", config.name, path);

		Ok(())
	}

	/// Initialize a new library directory with a specific UUID (for shared libraries)
	async fn initialize_library_with_id(
		&self,
		path: &Path,
		library_id: Uuid,
		name: String,
		description: Option<String>,
		context: Arc<CoreContext>,
	) -> Result<()> {
		// Create subdirectories
		tokio::fs::create_dir_all(path.join("previews")).await?;
		tokio::fs::create_dir_all(path.join("exports")).await?;
		tokio::fs::create_dir_all(path.join("sidecars")).await?;

		// Create configuration with provided UUID
		let config = LibraryConfig {
			version: LIBRARY_CONFIG_VERSION,
			id: library_id,
			name,
			description,
			created_at: Utc::now(),
			updated_at: Utc::now(),
			settings: LibrarySettings::default(),
			statistics: LibraryStatistics::default(),
		};

		// Initialize encryption key
		context
			.key_manager
			.get_library_key(config.id)
			.await
			.map_err(|e| {
				LibraryError::Other(format!(
					"Failed to initialize library encryption key: {}",
					e
				))
			})?;

		info!(
			"Initialized encryption key for shared library '{}'",
			config.name
		);

		// Save configuration
		let config_path = path.join("library.json");
		let json = serde_json::to_string_pretty(&config)?;
		tokio::fs::write(config_path, json).await?;

		// Initialize database
		let db_path = path.join(LIBRARY_DB_FILENAME);
		let db = Database::create(&db_path).await?;

		// Run initial migrations
		db.migrate().await?;

		info!(
			"Shared library '{}' initialized at {:?} with ID {}",
			config.name, path, library_id
		);

		Ok(())
	}

	/// Read library information without opening it
	async fn read_library_info(&self, path: &Path) -> Result<DiscoveredLibrary> {
		let config_path = path.join("library.json");
		let config_data = tokio::fs::read_to_string(&config_path).await?;
		let config: LibraryConfig = serde_json::from_str(&config_data)?;

		// Check if locked (but ignore stale locks)
		let lock_path = path.join(".sdlibrary.lock");
		let is_locked = if lock_path.exists() {
			// Use the LibraryLock's stale detection logic
			!LibraryLock::is_lock_stale(&lock_path).unwrap_or(true)
		} else {
			false
		};

		Ok(DiscoveredLibrary {
			path: path.to_path_buf(),
			config,
			is_locked,
		})
	}

	/// Ensure the current device is registered in the library
	async fn ensure_device_registered(&self, library: &Arc<Library>) -> Result<()> {
		let db = library.db();
		let device = self
			.device_manager
			.to_device()
			.map_err(|e| LibraryError::Other(format!("Failed to get device info: {}", e)))?;

		// Check if device exists
		let existing = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device.id))
			.one(db.conn())
			.await
			.map_err(LibraryError::DatabaseError)?;

		use sea_orm::ActiveValue::Set;

		if let Some(existing_device) = existing {
			// Update existing device to pick up any changes (e.g., renamed device)
			let mut device_model: entities::device::ActiveModel = existing_device.into();

			// Update fields that may have changed
			device_model.name = Set(device.name.clone());
			device_model.os_version = Set(device.os_version);
			device_model.hardware_model = Set(device.hardware_model);
			device_model.is_online = Set(true);
			device_model.last_seen_at = Set(Utc::now());
			device_model.updated_at = Set(Utc::now());

			device_model
				.update(db.conn())
				.await
				.map_err(LibraryError::DatabaseError)?;

			debug!("Updated device {} in library {}", device.id, library.id());

			// Broadcast update via sync
			if let Some(_sync_service) = library.sync_service() {
				let updated_model = entities::device::Entity::find()
					.filter(entities::device::Column::Uuid.eq(device.id))
					.one(db.conn())
					.await
					.map_err(LibraryError::DatabaseError)?
					.ok_or_else(|| {
						LibraryError::Other("Device not found after update".to_string())
					})?;

				if let Err(e) = library
					.sync_model(&updated_model, crate::infra::sync::ChangeType::Update)
					.await
				{
					warn!("Failed to sync device update: {}", e);
				}
			}
		} else {
			// First time registration - check if OUR slug conflicts with existing devices
			// Only the joining device renames itself, never rename existing devices
			let existing_slugs: Vec<String> = entities::device::Entity::find()
				.all(db.conn())
				.await
				.map_err(LibraryError::DatabaseError)?
				.iter()
				.map(|d| d.slug.clone())
				.collect();

			// Get current device's effective slug for this library
			let current_slug = self
				.device_manager
				.slug_for_library(library.id())
				.map_err(|e| LibraryError::Other(format!("Failed to get device slug: {}", e)))?;

			let unique_slug = Library::ensure_unique_slug(&current_slug, &existing_slugs);

			// If OUR slug conflicts, store library-specific override
			if unique_slug != current_slug {
				warn!(
					"Device slug collision in library {}. This device will use '{}' instead of '{}' in this library",
					library.id(),
					unique_slug,
					current_slug
				);

				self.device_manager
					.set_library_slug(library.id(), unique_slug.clone())
					.map_err(|e| {
						LibraryError::Other(format!("Failed to set library-specific slug: {}", e))
					})?;
			}

			// Register the device for the first time
			let device_model = entities::device::ActiveModel {
				id: sea_orm::ActiveValue::NotSet,
				uuid: Set(device.id),
				name: Set(device.name.clone()),
				slug: Set(unique_slug.clone()),
				os: Set(device.os.to_string()),
				os_version: Set(device.os_version),
				hardware_model: Set(device.hardware_model),
				// Hardware specs
				cpu_model: Set(device.cpu_model),
				cpu_architecture: Set(device.cpu_architecture),
				cpu_cores_physical: Set(device.cpu_cores_physical),
				cpu_cores_logical: Set(device.cpu_cores_logical),
				cpu_frequency_mhz: Set(device.cpu_frequency_mhz),
				memory_total_bytes: Set(device.memory_total_bytes),
				form_factor: Set(device.form_factor.map(|f| f.to_string())),
				manufacturer: Set(device.manufacturer),
				gpu_models: Set(device.gpu_models.map(|g| serde_json::json!(g))),
				boot_disk_type: Set(device.boot_disk_type),
				boot_disk_capacity_bytes: Set(device.boot_disk_capacity_bytes),
				swap_total_bytes: Set(device.swap_total_bytes),
				network_addresses: Set(serde_json::json!(device.network_addresses)),
				is_online: Set(true),
				last_seen_at: Set(Utc::now()),
				capabilities: Set(serde_json::json!({
					"indexing": true,
					"p2p": true,
					"volume_detection": true
				})),
				created_at: Set(device.created_at),
				sync_enabled: Set(true), // Enable sync by default for this device
				last_sync_at: Set(None),
				updated_at: Set(Utc::now()),
			};

			let inserted_model = device_model
				.insert(db.conn())
				.await
				.map_err(LibraryError::DatabaseError)?;

			info!(
				"Registered device {} in library {}",
				device.id,
				library.id()
			);

			// Broadcast device record via sync
			if let Some(_sync_service) = library.sync_service() {
				if let Err(e) = library
					.sync_model(&inserted_model, crate::infra::sync::ChangeType::Insert)
					.await
				{
					warn!("Failed to sync device registration: {}", e);
				} else {
					info!("Device record broadcast to sync partners");
				}
			}

			// Reload library's device cache
			if let Err(e) = library.reload_device_cache().await {
				warn!("Failed to reload device cache after registration: {}", e);
			}
		}

		Ok(())
	}

	/// Create default space with Quick Access group for new libraries
	///
	/// Uses deterministic UUIDs so all devices create the same default space,
	/// preventing duplicates during sync.
	async fn create_default_space(&self, library: &Arc<Library>) -> Result<()> {
		use crate::domain::{GroupType, ItemType, Space, SpaceGroup, SpaceItem};
		use crate::infra::sync::deterministic_library_default_uuid;
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, NotSet, Set};

		let db = library.db().conn();
		let library_id = library.id();

		// Create default space with deterministic UUID (same library = same UUID on all devices)
		let space_id = deterministic_library_default_uuid(library_id, "space", "All Devices");
		let now = Utc::now();

		let space_model = crate::infra::db::entities::space::ActiveModel {
			id: NotSet,
			uuid: Set(space_id),
			name: Set("All Devices".to_string()),
			icon: Set("Planet".to_string()),
			color: Set("#3B82F6".to_string()),
			order: Set(0),
			created_at: Set(now.into()),
			updated_at: Set(now.into()),
		};

		// Use atomic upsert to handle race conditions with sync
		// If Alice's space syncs to Bob before this runs, the upsert will update instead of failing
		use crate::infra::db::entities::space::{Column, Entity};
		Entity::insert(space_model)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::Name,
						Column::Icon,
						Column::Color,
						Column::Order,
						Column::UpdatedAt,
					])
					.to_owned(),
			)
			.exec(db)
			.await
			.map_err(LibraryError::DatabaseError)?;

		// Query the space back to get the id for creating items/groups
		let space_result = Entity::find()
			.filter(Column::Uuid.eq(space_id))
			.one(db)
			.await
			.map_err(LibraryError::DatabaseError)?
			.ok_or_else(|| LibraryError::Other("Space not found after upsert".to_string()))?;

		info!("Created default space for library {}", library.id());

		// Create space-level items (Overview, Recents, Favorites, File Kinds) - these appear outside groups
		let space_items = vec![
			(ItemType::Overview, "Overview", 0),
			(ItemType::Recents, "Recents", 1),
			(ItemType::Favorites, "Favorites", 2),
			(ItemType::FileKinds, "File Kinds", 3),
		];

		use crate::infra::db::entities::space_item::{Column as ItemColumn, Entity as ItemEntity};

		for (item_type, item_name, order) in space_items {
			let item_type_json = serde_json::to_string(&item_type).map_err(|e| {
				LibraryError::Other(format!("Failed to serialize item_type: {}", e))
			})?;

			let item_uuid = deterministic_library_default_uuid(library_id, "space_item", item_name);

			let item_model = crate::infra::db::entities::space_item::ActiveModel {
				id: NotSet,
				uuid: Set(item_uuid),
				space_id: Set(space_result.id),
				group_id: Set(None), // Space-level items have no group
				entry_id: Set(None), // Default items don't have entries
				item_type: Set(item_type_json),
				order: Set(order),
				created_at: Set(now.into()),
			};

			// Use atomic upsert to handle race conditions with sync
			ItemEntity::insert(item_model)
				.on_conflict(
					sea_orm::sea_query::OnConflict::column(ItemColumn::Uuid)
						.update_columns([
							ItemColumn::GroupId,
							ItemColumn::ItemType,
							ItemColumn::Order,
						])
						.to_owned(),
				)
				.exec(db)
				.await
				.map_err(LibraryError::DatabaseError)?;
		}

		info!(
			"Created default space-level items for library {}",
			library.id()
		);

		use crate::infra::db::entities::space_group::{
			Column as GroupColumn, Entity as GroupEntity,
		};

		// Create Devices group
		let devices_group_id =
			deterministic_library_default_uuid(library_id, "space_group", "Devices");
		let devices_type_json = serde_json::to_string(&GroupType::Devices)
			.map_err(|e| LibraryError::Other(format!("Failed to serialize group_type: {}", e)))?;

		let devices_group_model = crate::infra::db::entities::space_group::ActiveModel {
			id: NotSet,
			uuid: Set(devices_group_id),
			space_id: Set(space_result.id),
			name: Set("Devices".to_string()),
			group_type: Set(devices_type_json),
			is_collapsed: Set(false),
			order: Set(0),
			created_at: Set(now.into()),
		};

		// Use atomic upsert to handle race conditions with sync
		GroupEntity::insert(devices_group_model)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(GroupColumn::Uuid)
					.update_columns([
						GroupColumn::SpaceId,
						GroupColumn::Name,
						GroupColumn::GroupType,
						GroupColumn::IsCollapsed,
						GroupColumn::Order,
					])
					.to_owned(),
			)
			.exec(db)
			.await
			.map_err(LibraryError::DatabaseError)?;

		info!("Created default Devices group for library {}", library.id());

		// Create Locations group
		let locations_group_id =
			deterministic_library_default_uuid(library_id, "space_group", "Locations");
		let locations_type_json = serde_json::to_string(&GroupType::Locations)
			.map_err(|e| LibraryError::Other(format!("Failed to serialize group_type: {}", e)))?;

		let locations_group_model = crate::infra::db::entities::space_group::ActiveModel {
			id: NotSet,
			uuid: Set(locations_group_id),
			space_id: Set(space_result.id),
			name: Set("Locations".to_string()),
			group_type: Set(locations_type_json),
			is_collapsed: Set(false),
			order: Set(1),
			created_at: Set(now.into()),
		};

		// Use atomic upsert to handle race conditions with sync
		GroupEntity::insert(locations_group_model)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(GroupColumn::Uuid)
					.update_columns([
						GroupColumn::SpaceId,
						GroupColumn::Name,
						GroupColumn::GroupType,
						GroupColumn::IsCollapsed,
						GroupColumn::Order,
					])
					.to_owned(),
			)
			.exec(db)
			.await
			.map_err(LibraryError::DatabaseError)?;

		info!(
			"Created default Locations group for library {}",
			library.id()
		);

		// Create Volumes group
		let volumes_group_id =
			deterministic_library_default_uuid(library_id, "space_group", "Volumes");
		let volumes_type_json = serde_json::to_string(&GroupType::Volumes)
			.map_err(|e| LibraryError::Other(format!("Failed to serialize group_type: {}", e)))?;

		let volumes_group_model = crate::infra::db::entities::space_group::ActiveModel {
			id: NotSet,
			uuid: Set(volumes_group_id),
			space_id: Set(space_result.id),
			name: Set("Volumes".to_string()),
			group_type: Set(volumes_type_json),
			is_collapsed: Set(false),
			order: Set(2),
			created_at: Set(now.into()),
		};

		// Use atomic upsert to handle race conditions with sync
		GroupEntity::insert(volumes_group_model)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(GroupColumn::Uuid)
					.update_columns([
						GroupColumn::SpaceId,
						GroupColumn::Name,
						GroupColumn::GroupType,
						GroupColumn::IsCollapsed,
						GroupColumn::Order,
					])
					.to_owned(),
			)
			.exec(db)
			.await
			.map_err(LibraryError::DatabaseError)?;

		info!("Created default Volumes group for library {}", library.id());

		// Create Tags group
		let tags_group_id = deterministic_library_default_uuid(library_id, "space_group", "Tags");
		let tags_type_json = serde_json::to_string(&GroupType::Tags)
			.map_err(|e| LibraryError::Other(format!("Failed to serialize group_type: {}", e)))?;

		let tags_group_model = crate::infra::db::entities::space_group::ActiveModel {
			id: NotSet,
			uuid: Set(tags_group_id),
			space_id: Set(space_result.id),
			name: Set("Tags".to_string()),
			group_type: Set(tags_type_json),
			is_collapsed: Set(false),
			order: Set(3),
			created_at: Set(now.into()),
		};

		// Use atomic upsert to handle race conditions with sync
		GroupEntity::insert(tags_group_model)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(GroupColumn::Uuid)
					.update_columns([
						GroupColumn::SpaceId,
						GroupColumn::Name,
						GroupColumn::GroupType,
						GroupColumn::IsCollapsed,
						GroupColumn::Order,
					])
					.to_owned(),
			)
			.exec(db)
			.await
			.map_err(LibraryError::DatabaseError)?;

		info!("Created default Tags group for library {}", library.id());

		Ok(())
	}

	/// Create default OS-specific locations with IndexMode::None
	async fn create_default_locations(&self, context: Arc<CoreContext>, library: Arc<Library>) {
		use crate::domain::addressing::SdPath;
		use crate::location::{manager::LocationManager, IndexMode};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		use std::path::PathBuf;

		// Get home directory
		let home = match dirs::home_dir() {
			Some(h) => h,
			None => {
				warn!("Failed to get home directory, skipping default location creation");
				return;
			}
		};

		// Get OS-specific default locations
		let default_locations = Self::get_default_locations_for_os(&home);

		// Get current device UUID
		let device_uuid = crate::device::get_current_device_id();

		// Get device record from database
		let db = library.db().conn();
		let device = match entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.one(db)
			.await
		{
			Ok(Some(dev)) => dev,
			Ok(None) => {
				error!("Current device not found in library database");
				return;
			}
			Err(e) => {
				error!("Failed to query device: {}", e);
				return;
			}
		};

		let device_slug = device.slug.clone();
		let device_id = device.id;

		// Create location manager
		let location_manager = LocationManager::new((*self.event_bus).clone());

		// Create each default location with IndexMode::None
		for (name, path) in default_locations {
			// Check if path exists
			if !path.exists() {
				debug!("Skipping non-existent default location: {:?}", path);
				continue;
			}

			let sd_path = SdPath::Physical {
				device_slug: device_slug.clone(),
				path: path.clone(),
			};

			match location_manager
				.add_location(
					library.clone(),
					sd_path,
					Some(name.clone()),
					device_id,
					IndexMode::None,
					None, // No action context
					None, // No job policies
				)
				.await
			{
				Ok((location_id, _)) => {
					info!(
						"Created default location '{}' at {:?} ({})",
						name, path, location_id
					);
				}
				Err(e) => {
					warn!("Failed to create default location '{}': {}", name, e);
				}
			}
		}
	}

	/// Get default locations based on OS
	fn get_default_locations_for_os(home: &PathBuf) -> Vec<(String, PathBuf)> {
		let mut locations = Vec::new();

		if cfg!(target_os = "macos") {
			locations.push(("Desktop".to_string(), home.join("Desktop")));
			locations.push(("Documents".to_string(), home.join("Documents")));
			locations.push(("Downloads".to_string(), home.join("Downloads")));
			locations.push(("Pictures".to_string(), home.join("Pictures")));
			locations.push(("Music".to_string(), home.join("Music")));
			locations.push(("Movies".to_string(), home.join("Movies")));
		} else if cfg!(target_os = "linux") {
			locations.push(("Desktop".to_string(), home.join("Desktop")));
			locations.push(("Documents".to_string(), home.join("Documents")));
			locations.push(("Downloads".to_string(), home.join("Downloads")));
			locations.push(("Pictures".to_string(), home.join("Pictures")));
			locations.push(("Music".to_string(), home.join("Music")));
			locations.push(("Videos".to_string(), home.join("Videos")));
		} else if cfg!(target_os = "windows") {
			locations.push(("Desktop".to_string(), home.join("Desktop")));
			locations.push(("Documents".to_string(), home.join("Documents")));
			locations.push(("Downloads".to_string(), home.join("Downloads")));
			locations.push(("Pictures".to_string(), home.join("Pictures")));
			locations.push(("Music".to_string(), home.join("Music")));
			locations.push(("Videos".to_string(), home.join("Videos")));
		}

		locations
	}

	/// Check if this device created the library (is the only device)
	async fn is_library_creator(&self, library: &Arc<Library>) -> Result<bool> {
		let db = library.db();
		let device_id = self
			.device_manager
			.device_id()
			.map_err(|e| LibraryError::Other(format!("Failed to get device ID: {}", e)))?;

		// Count total devices in the library
		let device_count = entities::device::Entity::find()
			.count(db.conn())
			.await
			.map_err(LibraryError::DatabaseError)?;

		// If this is the only device, it's the creator
		if device_count == 1 {
			// Verify it's actually our device
			let our_device = entities::device::Entity::find()
				.filter(entities::device::Column::Uuid.eq(device_id))
				.one(db.conn())
				.await
				.map_err(LibraryError::DatabaseError)?;

			Ok(our_device.is_some())
		} else {
			// Multiple devices - not the creator
			Ok(false)
		}
	}

	/// Delete a library
	pub async fn delete_library(&self, id: Uuid, delete_data: bool) -> Result<()> {
		let library = self
			.get_library(id)
			.await
			.ok_or(LibraryError::NotFound(id.to_string()))?;

		//remove from library manager
		let mut libraries = self.libraries.write().await;
		libraries.remove(&id);

		let deleted_data_flag = if delete_data {
			library.delete().await?;
			true
		} else {
			false
		};

		// Emit event
		self.event_bus.emit(Event::LibraryDeleted {
			id,
			name: library.name().await,
			deleted_data: deleted_data_flag,
		});

		info!("Deleted library {}", id);

		Ok(())
	}

	/// Start filesystem watching on the libraries directory
	pub async fn start_watching(&self) -> Result<()> {
		if *self.is_watching.read().await {
			warn!("Library watcher is already running");
			return Ok(());
		}

		// Get the primary search path (libraries directory)
		let watch_path = match self.search_paths.first() {
			Some(path) => path.clone(),
			None => {
				warn!("No search paths configured for library manager");
				return Ok(());
			}
		};

		// Ensure the directory exists
		if !watch_path.exists() {
			info!("Creating libraries directory: {:?}", watch_path);
			tokio::fs::create_dir_all(&watch_path).await?;
		}

		info!("Starting library watcher on {:?}", watch_path);

		let (tx, mut rx) = mpsc::channel(100);
		let tx_clone = tx.clone();

		let libraries = self.libraries.clone();
		let event_bus = self.event_bus.clone();
		let is_watching = self.is_watching.clone();
		let context = self.context.clone();
		let watch_path_clone = watch_path.clone();

		// Create filesystem watcher
		let mut watcher = notify::recommended_watcher(
			move |res: std::result::Result<notify::Event, notify::Error>| {
				match res {
					Ok(event) => {
						// Use try_send since we're in a sync context
						if let Err(e) = tx_clone.try_send(event) {
							error!("Failed to send library watcher event: {}", e);
						}
					}
					Err(e) => {
						error!("Library filesystem watcher error: {}", e);
					}
				}
			},
		)?;

		// Configure with polling interval
		watcher.configure(Config::default().with_poll_interval(Duration::from_millis(500)))?;

		// Watch the libraries directory (non-recursive)
		watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

		// Store the watcher
		*self.watcher.write().await = Some(watcher);
		*self.is_watching.write().await = true;

		// Start event processing loop
		tokio::spawn(async move {
			info!("Library watcher event loop started");

			// Debouncing: collect events and process them after a delay
			let mut pending_creates: HashMap<PathBuf, std::time::Instant> = HashMap::new();
			let mut pending_removes: HashMap<PathBuf, std::time::Instant> = HashMap::new();
			let debounce_duration = Duration::from_millis(500);

			loop {
				tokio::select! {
					Some(event) = rx.recv() => {
						let now = std::time::Instant::now();

						for path in &event.paths {
							// Only process .sdlibrary directories
							if !is_library_directory(path) {
								continue;
							}

							match event.kind {
								notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
									debug!("Library create/modify event: {:?}", path);
									pending_creates.insert(path.clone(), now);
									pending_removes.remove(path);
								}
								notify::EventKind::Remove(_) => {
									debug!("Library remove event: {:?}", path);
									pending_removes.insert(path.clone(), now);
									pending_creates.remove(path);
								}
								_ => {}
							}
						}
					}
					_ = tokio::time::sleep(Duration::from_millis(100)) => {
						let now = std::time::Instant::now();

						// Process creates that have been stable for debounce duration
						let mut to_create = Vec::new();
						pending_creates.retain(|path, time| {
							if now.duration_since(*time) >= debounce_duration {
								to_create.push(path.clone());
								false
							} else {
								true
							}
						});

						for path in to_create {
							// Check if the library exists and is valid
							if path.exists() && is_library_directory(&path) {
								debug!("Processing library create: {:?}", path);

								// Get the context
								let ctx = match context.read().await.as_ref() {
									Some(ctx) => ctx.clone(),
									None => {
										warn!("Core context not available, skipping library open");
										continue;
									}
								};

								// Load library config to get ID
								match LibraryConfig::load(&path.join("library.json")).await {
									Ok(config) => {
										// Check if already open
										if libraries.read().await.contains_key(&config.id) {
											debug!("Library {} already open, skipping", config.id);
											continue;
										}

										// Create a temporary LibraryManager to access open_library
										// We can't call self.open_library directly from spawn
										let temp_manager = LibraryManager {
											libraries: libraries.clone(),
											search_paths: vec![watch_path_clone.clone()],
											event_bus: event_bus.clone(),
											volume_manager: ctx.volume_manager.clone(),
											device_manager: ctx.device_manager.clone(),
											watcher: Arc::new(RwLock::new(None)),
											is_watching: Arc::new(RwLock::new(false)),
											context: Arc::new(RwLock::new(None)),
										};

										match temp_manager.open_library(&path, ctx).await {
											Ok(library) => {
												info!("Auto-opened library from filesystem: {} at {:?}", library.id(), path);
											}
											Err(LibraryError::AlreadyOpen(id)) => {
												debug!("Library {} already open", id);
											}
											Err(e) => {
												warn!("Failed to auto-open library from {:?}: {}", path, e);
											}
										}
									}
									Err(e) => {
										warn!("Failed to load library config from {:?}: {}", path, e);
									}
								}
							}
						}

						// Process removes that have been stable for debounce duration
						let mut to_remove = Vec::new();
						pending_removes.retain(|path, time| {
							if now.duration_since(*time) >= debounce_duration {
								to_remove.push(path.clone());
								false
							} else {
								true
							}
						});

						for path in to_remove {
							// Check if the library directory no longer exists
							if !path.exists() {
								debug!("Processing library remove: {:?}", path);

								// Find the library by path
								let libs = libraries.read().await;
								let library_id = libs.iter()
									.find(|(_, lib)| lib.path() == path)
									.map(|(id, _)| *id);
								drop(libs);

								if let Some(id) = library_id {
									// Get context for closing library
									let ctx = match context.read().await.as_ref() {
										Some(ctx) => ctx.clone(),
										None => {
											warn!("Core context not available");
											continue;
										}
									};

									// Create a temporary LibraryManager to access close_library
									let temp_manager = LibraryManager {
										libraries: libraries.clone(),
										search_paths: vec![watch_path_clone.clone()],
										event_bus: event_bus.clone(),
										volume_manager: ctx.volume_manager.clone(),
										device_manager: ctx.device_manager.clone(),
										watcher: Arc::new(RwLock::new(None)),
										is_watching: Arc::new(RwLock::new(false)),
										context: Arc::new(RwLock::new(None)),
									};

									match temp_manager.close_library(id).await {
										Ok(_) => {
											info!("Auto-closed library {} (directory removed)", id);
										}
										Err(e) => {
											warn!("Failed to auto-close library {}: {}", id, e);
										}
									}
								}
							}
						}
					}
				}

				// Check if we should stop
				if !*is_watching.read().await {
					info!("Library watcher shutting down");
					break;
				}
			}

			info!("Library watcher event loop stopped");
		});

		Ok(())
	}

	/// Stop filesystem watching
	pub async fn stop_watching(&self) -> Result<()> {
		if !*self.is_watching.read().await {
			return Ok(());
		}

		info!("Stopping library watcher");

		*self.is_watching.write().await = false;
		*self.watcher.write().await = None;

		info!("Library watcher stopped");

		Ok(())
	}

	/// Set the core context (needed for opening libraries in watcher)
	pub async fn set_context(&self, context: Arc<CoreContext>) {
		*self.context.write().await = Some(context);
	}
}

/// Check if a path is a library directory
fn is_library_directory(path: &Path) -> bool {
	path.extension()
		.and_then(|ext| ext.to_str())
		.map(|ext| ext == LIBRARY_EXTENSION)
		.unwrap_or(false)
}

/// Sanitize a filename for safe filesystem usage
fn sanitize_filename(name: &str) -> String {
	// Replace problematic characters
	name.chars()
		.map(|c| match c {
			'/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
			c if c.is_control() => '-',
			c => c,
		})
		.collect::<String>()
		.trim()
		.to_string()
}

/// Find a unique library path by adding numbers if needed
async fn find_unique_library_path(base_path: &Path, name: &str) -> Result<PathBuf> {
	let mut path = base_path.join(format!("{}.{}", name, LIBRARY_EXTENSION));
	let mut counter = 1;

	while path.exists() {
		path = base_path.join(format!("{} {}.{}", name, counter, LIBRARY_EXTENSION));
		counter += 1;

		if counter > 1000 {
			return Err(LibraryError::Other(
				"Could not find unique library name".to_string(),
			));
		}
	}

	Ok(path)
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_sanitize_filename() {
		assert_eq!(sanitize_filename("My Library"), "My Library");
		assert_eq!(sanitize_filename("My/Library"), "My-Library");
		assert_eq!(sanitize_filename("My\\Library"), "My-Library");
		assert_eq!(sanitize_filename("My:Library"), "My-Library");
		assert_eq!(sanitize_filename("My*Library?"), "My-Library-");
	}

	#[tokio::test]
	async fn test_is_library_directory() {
		assert!(is_library_directory(Path::new(
			"/path/to/My Library.sdlibrary"
		)));
		assert!(!is_library_directory(Path::new("/path/to/My Library")));
		assert!(!is_library_directory(Path::new("/path/to/My Library.txt")));
	}
}
