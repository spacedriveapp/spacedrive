//! Library manager - handles creation, opening, and discovery of libraries

use super::{
	config::{LibraryConfig, LibrarySettings, LibraryStatistics, ThumbnailMetadata},
	error::{LibraryError, Result},
	lock::LibraryLock,
	Library, LIBRARY_CONFIG_VERSION, LIBRARY_EXTENSION,
};
use crate::{
	context::CoreContext,
	device::DeviceManager,
	infra::{
		db::{entities, Database},
		event::{Event, EventBus},
		job::manager::JobManager,
	},
	service::session::SessionStateService,
	volume::VolumeManager,
};
use chrono::Utc;
use once_cell::sync::OnceCell;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
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
		self.initialize_library(&library_path, name).await?;

		// Open the newly created library
		let library = self.open_library(&library_path, context.clone()).await?;

		// Emit event
		self.event_bus.emit(Event::LibraryCreated {
			id: library.id(),
			name: library.name().await,
			path: library_path,
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

		// Open database
		let db_path = path.join("database.db");
		let db = Arc::new(Database::open(&db_path).await?);

		// Get this device's ID for sync coordination
		let device_id = context
			.device_manager
			.device_id()
			.map_err(|e| LibraryError::Other(format!("Failed to get device ID: {}", e)))?;

		// Create transaction manager
		let transaction_manager = Arc::new(crate::infra::sync::TransactionManager::new(
			self.event_bus.clone(),
		));

		// Create job manager with context
		let job_manager =
			Arc::new(JobManager::new(path.to_path_buf(), context.clone(), config.id).await?);
		job_manager.initialize().await?;

		// Create library instance
		let library = Arc::new(Library {
			path: path.to_path_buf(),
			config: RwLock::new(config.clone()),
			db,
			jobs: job_manager,
			event_bus: self.event_bus.clone(),
			transaction_manager,
			sync_service: OnceCell::new(), // Initialized later
			_lock: lock,
		});

		// Ensure device is registered in this library
		if let Err(e) = self.ensure_device_registered(&library).await {
			warn!("Failed to register device in library {}: {}", config.id, e);
		}

		// Register library
		{
			let mut libraries = self.libraries.write().await;
			libraries.insert(config.id, library.clone());
		}

		// Now that the library is registered in the context, resume interrupted jobs
		if let Err(e) = library.jobs.resume_interrupted_jobs_after_load().await {
			warn!(
				"Failed to resume interrupted jobs for library {}: {}",
				config.id, e
			);
		}

		// Note: Sidecar manager initialization should be done by the Core when libraries are loaded
		// This allows Core to pass its services reference

		// Initialize sync service
		if let Err(e) = library.init_sync_service(device_id).await {
			warn!(
				"Failed to initialize sync service for library {}: {}",
				config.id, e
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
									warn!("Failed to auto-load library from {:?}: {}", path, e);
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

	/// Initialize a new library directory
	async fn initialize_library(&self, path: &Path, name: String) -> Result<()> {
		// Create subdirectories
		tokio::fs::create_dir_all(path.join("thumbnails")).await?;
		tokio::fs::create_dir_all(path.join("previews")).await?;
		tokio::fs::create_dir_all(path.join("indexes")).await?;
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

		// Save configuration
		let config_path = path.join("library.json");
		let json = serde_json::to_string_pretty(&config)?;
		tokio::fs::write(config_path, json).await?;

		// Initialize database
		let db_path = path.join("database.db");
		let db = Database::create(&db_path).await?;

		// Run initial migrations
		db.migrate().await?;

		// Create thumbnail metadata
		let thumb_meta = ThumbnailMetadata::default();
		let thumb_meta_path = path.join("thumbnails").join("metadata.json");
		let json = serde_json::to_string_pretty(&thumb_meta)?;
		tokio::fs::write(thumb_meta_path, json).await?;

		info!("Initialized new library '{}' at {:?}", config.name, path);

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
			device_model.hardware_model = Set(device.hardware_model);
			device_model.is_online = Set(true);
			device_model.last_seen_at = Set(Utc::now());
			device_model.updated_at = Set(Utc::now());

			device_model
				.update(db.conn())
				.await
				.map_err(LibraryError::DatabaseError)?;

			debug!("Updated device {} in library {}", device.id, library.id());
		} else {
			// Register the device for the first time
			let device_model = entities::device::ActiveModel {
				id: sea_orm::ActiveValue::NotSet,
				uuid: Set(device.id),
				name: Set(device.name.clone()),
				os: Set(device.os.to_string()),
				os_version: Set(None),
				hardware_model: Set(device.hardware_model),
				network_addresses: Set(serde_json::json!(device.network_addresses)),
				is_online: Set(true),
				last_seen_at: Set(Utc::now()),
			capabilities: Set(serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			})),
			created_at: Set(device.created_at),
			updated_at: Set(Utc::now()),
		};

			device_model
				.insert(db.conn())
				.await
				.map_err(LibraryError::DatabaseError)?;

			info!(
				"Registered device {} in library {}",
				device.id,
				library.id()
			);
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
