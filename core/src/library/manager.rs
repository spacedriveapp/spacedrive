//! Library manager - handles creation, opening, and discovery of libraries

use super::{
	config::{LibraryConfig, LibrarySettings, LibraryStatistics},
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
	ops::indexing::volume_index::LiveVolumes,
	service::session::SessionStateService,
	volume::VolumeManager,
};
use chrono::Utc;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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

	/// Library directories a create call is still building. The directory
	/// watcher sees a new `.sdlibrary` folder before its creator has finished
	/// initializing and opening it; opening it a second time from the watcher
	/// runs the migrations twice on one database and fights over the lock, so
	/// the watcher leaves these paths to their creator.
	creating: Arc<Mutex<HashSet<PathBuf>>>,
}

/// Marks a library path as being created until dropped.
pub(crate) struct CreationGuard {
	creating: Arc<Mutex<HashSet<PathBuf>>>,
	path: PathBuf,
}

impl Drop for CreationGuard {
	fn drop(&mut self) {
		if let Ok(mut creating) = self.creating.lock() {
			creating.remove(&self.path);
		}
	}
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
			creating: Arc::new(Mutex::new(HashSet::new())),
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
			creating: Arc::new(Mutex::new(HashSet::new())),
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

		let _creating = self.mark_creating(library_path.clone()).await;

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
		initial_device_os: String,
		initial_device_os_version: Option<String>,
		initial_device_hardware_model: Option<String>,
		initial_device_cpu_model: Option<String>,
		initial_device_cpu_architecture: Option<String>,
		initial_device_cpu_cores_physical: Option<u32>,
		initial_device_cpu_cores_logical: Option<u32>,
		initial_device_cpu_frequency_mhz: Option<i64>,
		initial_device_memory_total_bytes: Option<i64>,
		initial_device_form_factor: Option<String>,
		initial_device_manufacturer: Option<String>,
		initial_device_gpu_models: Option<Vec<String>>,
		initial_device_boot_disk_type: Option<String>,
		initial_device_boot_disk_capacity_bytes: Option<i64>,
		initial_device_swap_total_bytes: Option<i64>,
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

		let _creating = self.mark_creating(library_path.clone()).await;

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
			os: Set(initial_device_os),
			os_version: Set(initial_device_os_version),
			hardware_model: Set(initial_device_hardware_model),
			cpu_model: Set(initial_device_cpu_model),
			cpu_architecture: Set(initial_device_cpu_architecture),
			cpu_cores_physical: Set(initial_device_cpu_cores_physical),
			cpu_cores_logical: Set(initial_device_cpu_cores_logical),
			cpu_frequency_mhz: Set(initial_device_cpu_frequency_mhz),
			memory_total_bytes: Set(initial_device_memory_total_bytes),
			form_factor: Set(initial_device_form_factor),
			manufacturer: Set(initial_device_manufacturer),
			gpu_models: Set(initial_device_gpu_models.map(|g| serde_json::json!(g))),
			boot_disk_type: Set(initial_device_boot_disk_type),
			boot_disk_capacity_bytes: Set(initial_device_boot_disk_capacity_bytes),
			swap_total_bytes: Set(initial_device_swap_total_bytes),
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

		// Emit event - this is a synced library from another device
		self.event_bus.emit(Event::LibraryCreated {
			id: library.id(),
			name: library.name().await,
			path: library_path.clone(),
			source: LibraryCreationSource::Sync,
		});

		Ok(library)
	}

	/// Keep the watcher off `path` until the guard drops: the caller is
	/// assembling or replacing that library and opens it itself.
	pub(crate) async fn mark_creating(&self, path: PathBuf) -> CreationGuard {
		let path = creation_key(&path).await;
		self.creating
			.lock()
			.expect("creating set poisoned")
			.insert(path.clone());
		CreationGuard {
			creating: self.creating.clone(),
			path,
		}
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

		let _creating = self.mark_creating(library_path.clone()).await;

		// Create library directory
		tokio::fs::create_dir_all(&library_path).await?;

		// Initialize library
		self.initialize_library(&library_path, name.to_string(), context.clone())
			.await?;

		// Open the newly created library
		let library = self.open_library(&library_path, context.clone()).await?;

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
			sync_service: OnceCell::new(), // Initialized later // Initialized later
			source_manager: OnceCell::new(), // Initialized lazily
			device_cache: Arc::new(std::sync::RwLock::new(device_cache)),
			_lock: std::sync::Mutex::new(Some(lock)),
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

		// This is both the creation path and the migration path. Running it on
		// every open keeps older libraries on the current sidebar contract.
		self.create_default_space(&library).await?;

		// Adopt this library's source registrations. Absolute roots resolve
		// against wherever each anchoring volume is mounted now, so a drive
		// that came back at a different mount point needs no repair, and a
		// drive detection cannot see comes up detached whatever its row says.
		let live = self.volume_manager.live_volumes().await;
		let live = match &live {
			Some(volumes) => LiveVolumes::Detected(volumes),
			None => LiveVolumes::Unavailable,
		};
		match context
			.volume_index()
			.attach_library_with(library.db().clone(), Some(device_id), live)
			.await
		{
			Ok(adopted) => debug!("Adopted {adopted} sources for library {}", config.id),
			Err(e) => error!("Failed to adopt sources for library {}: {e}", config.id),
		}

		// Register library
		{
			let mut libraries = self.libraries.write().await;
			libraries.insert(config.id, library.clone());
		}

		// Now that the library is registered, reconcile the job rows the
		// previous process left behind: resumable jobs resume, everything
		// else is marked failed with its reason. Without this, interrupted
		// rows sit at Running forever and nothing a job list says can be
		// trusted.
		if let Err(e) = library.jobs.resume_interrupted_jobs_after_load().await {
			warn!(
				"Failed to reconcile interrupted jobs for library {}: {}",
				config.id, e
			);
		}

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

	/// The directory new libraries are written to.
	pub fn libraries_dir(&self) -> Option<&Path> {
		self.search_paths.first().map(PathBuf::as_path)
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

		// This copy was created from another device's library, so it holds
		// nothing yet and must backfill from a member before it may serve as
		// a source. The marker lives in sync.db where the sync service reads
		// it on open; written here so the first open already sees it.
		let sync_db = sea_orm::Database::connect(format!(
			"sqlite://{}?mode=rwc",
			path.join("sync.db").display()
		))
		.await
		.map_err(LibraryError::DatabaseError)?;
		crate::infra::sync::SyncStateStore::mark_initial_backfill_pending(&sync_db)
			.await
			.map_err(|e| LibraryError::Other(format!("Failed to mark backfill pending: {}", e)))?;
		drop(sync_db);

		info!(
			"Shared library '{}' initialized at {:?} with ID {}, waiting for first backfill",
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
			// Update existing device to pick up any changes (e.g., renamed device, hardware upgrades)
			let mut device_model: entities::device::ActiveModel = existing_device.into();

			// Update all fields including hardware specs (self-healing for NULL stubs)
			device_model.name = Set(device.name.clone());
			device_model.os_version = Set(device.os_version);
			device_model.hardware_model = Set(device.hardware_model);
			device_model.cpu_model = Set(device.cpu_model);
			device_model.cpu_architecture = Set(device.cpu_architecture);
			device_model.cpu_cores_physical = Set(device.cpu_cores_physical);
			device_model.cpu_cores_logical = Set(device.cpu_cores_logical);
			device_model.cpu_frequency_mhz = Set(device.cpu_frequency_mhz);
			device_model.memory_total_bytes = Set(device.memory_total_bytes);
			device_model.form_factor = Set(device.form_factor.map(|f| f.to_string()));
			device_model.manufacturer = Set(device.manufacturer);
			device_model.gpu_models = Set(device.gpu_models.map(|g| serde_json::json!(g)));
			device_model.boot_disk_type = Set(device.boot_disk_type);
			device_model.boot_disk_capacity_bytes = Set(device.boot_disk_capacity_bytes);
			device_model.swap_total_bytes = Set(device.swap_total_bytes);
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

	/// Create the compatible backing space for the default sidebar.
	///
	/// Uses deterministic UUIDs so all devices create the same default space,
	/// preventing duplicates during sync.
	async fn create_default_space(&self, library: &Arc<Library>) -> Result<()> {
		use crate::domain::ItemType;
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
			name: Set("Default".to_string()),
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

		// Keep stable item keys while the UI gives these destinations plain
		// product names such as Home, Storage, and Protection.
		let space_items = vec![
			(ItemType::Overview, "Overview", 0),
			(ItemType::Recents, "Recents", 1),
			(
				ItemType::Collection {
					slug: "screenshots".to_string(),
				},
				"Screenshots",
				2,
			),
			(ItemType::Sources, "Sources", 3),
			(ItemType::Redundancy, "Redundancy", 4),
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
				group_id: Set(None),   // Space-level items have no group
				entry_uuid: Set(None), // Default items don't have entries
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

		// Retired defaults are removed so existing libraries converge on the
		// current seed set; deterministic uuids make the deletion precise.
		for retired_name in ["File Kinds", "Favorites", "Analyzer"] {
			let retired_uuid =
				deterministic_library_default_uuid(library_id, "space_item", retired_name);
			ItemEntity::delete_many()
				.filter(ItemColumn::Uuid.eq(retired_uuid))
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

		// System topology is data on the Storage surface, not persisted
		// navigation. Delete only the deterministic groups. Pins a person placed
		// inside one are lifted to the workspace root before its container goes.
		for retired_name in ["Devices", "Locations", "Volumes", "Tags", "Sources"] {
			let retired_uuid =
				deterministic_library_default_uuid(library_id, "space_group", retired_name);
			if let Some(group) = GroupEntity::find()
				.filter(GroupColumn::Uuid.eq(retired_uuid))
				.one(db)
				.await
				.map_err(LibraryError::DatabaseError)?
			{
				let items = ItemEntity::find()
					.filter(ItemColumn::GroupId.eq(group.id))
					.all(db)
					.await
					.map_err(LibraryError::DatabaseError)?;
				for item in items {
					let mut item: crate::infra::db::entities::space_item::ActiveModel = item.into();
					item.group_id = Set(None);
					item.update(db).await.map_err(LibraryError::DatabaseError)?;
				}
				GroupEntity::delete_many()
					.filter(GroupColumn::Uuid.eq(retired_uuid))
					.exec(db)
					.await
					.map_err(LibraryError::DatabaseError)?;
			}
		}

		Ok(())
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
		let creating = self.creating.clone();

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
							let key = creation_key(&path).await;
							if creating.lock().is_ok_and(|creating| creating.contains(&key)) {
								debug!("Library {:?} is being created here, leaving it to its creator", path);
								continue;
							}

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
											creating: creating.clone(),
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
										creating: creating.clone(),
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
pub(crate) fn sanitize_filename(name: &str) -> String {
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

/// The watcher reports a library directory as the filesystem spells it, which
/// on macOS differs from the path a create call built (`/var` versus
/// `/private/var`), so both sides key on the canonical parent. The directory
/// itself may not exist yet when its creator registers it.
async fn creation_key(path: &Path) -> PathBuf {
	match (path.parent(), path.file_name()) {
		(Some(parent), Some(name)) => tokio::fs::canonicalize(parent)
			.await
			.map(|parent| parent.join(name))
			.unwrap_or_else(|_| path.to_path_buf()),
		_ => path.to_path_buf(),
	}
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
