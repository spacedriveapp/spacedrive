//! Library management system
//!
//! This module provides the core library functionality for Spacedrive.
//! Each library is a self-contained directory with its own database,
//! thumbnails, and other data.

pub(crate) mod config;
mod error;
mod lock;
mod manager;
mod sync_helpers;

pub use config::{LibraryConfig, LibrarySettings, LibraryStatistics};
pub use error::{LibraryError, Result};
pub use lock::LibraryLock;
pub use manager::{DiscoveredLibrary, LibraryManager};

/// Filename for the library database
pub(crate) const LIBRARY_DB_FILENAME: &str = "library.db";

use crate::infra::{
	db::Database,
	event::EventBus,
	job::manager::JobManager,
	sync::{SyncEventBus, TransactionManager},
};
use once_cell::sync::OnceCell;
use sea_orm::ConnectionTrait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Represents an open Spacedrive library
pub struct Library {
	/// Root directory of the library (the .sdlibrary folder)
	path: PathBuf,

	/// Library configuration
	config: Arc<RwLock<LibraryConfig>>,

	/// Core context for accessing system services
	core_context: Arc<crate::context::CoreContext>,

	/// Database connection
	db: Arc<Database>,

	/// Job manager for this library
	jobs: Arc<JobManager>,

	/// General event bus for UI, jobs, volume events, etc
	event_bus: Arc<EventBus>,

	/// Dedicated sync event bus (prevents starvation from high-volume events)
	sync_events: Arc<SyncEventBus>,

	/// Transaction manager for atomic writes + sync logging
	transaction_manager: Arc<TransactionManager>,

	/// Sync service for real-time synchronization (initialized after library creation)
	sync_service: OnceCell<Arc<crate::service::sync::SyncService>>,

	/// File sync service for cross-location file synchronization (initialized after library creation)
	file_sync_service: OnceCell<Arc<crate::service::file_sync::FileSyncService>>,

	/// Library-specific device cache (slug → UUID)
	/// Loaded from this library's devices table for per-library device resolution
	device_cache: Arc<StdRwLock<HashMap<String, Uuid>>>,

	/// Lock preventing concurrent access
	_lock: LibraryLock,
}

impl Library {
	/// Get the library ID
	pub fn id(&self) -> Uuid {
		// Config is immutable for ID, so we can use try_read
		self.config.try_read().map(|c| c.id).unwrap_or_else(|_| {
			// This should never happen in practice
			panic!("Failed to read library config for ID")
		})
	}

	/// Get the library name
	pub async fn name(&self) -> String {
		self.config.read().await.name.clone()
	}

	/// Get the library path
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Get the database
	pub fn db(&self) -> &Arc<Database> {
		&self.db
	}

	/// Get the general event bus (for UI, jobs, volumes, etc)
	pub fn event_bus(&self) -> &Arc<EventBus> {
		&self.event_bus
	}

	/// Get the dedicated sync event bus
	pub fn sync_events(&self) -> &Arc<SyncEventBus> {
		&self.sync_events
	}

	/// Get the job manager
	pub fn jobs(&self) -> &Arc<JobManager> {
		&self.jobs
	}

	/// Get the transaction manager
	pub fn transaction_manager(&self) -> &Arc<TransactionManager> {
		&self.transaction_manager
	}

	/// Get the sync service
	pub fn sync_service(&self) -> Option<&Arc<crate::service::sync::SyncService>> {
		self.sync_service.get()
	}

	/// Get the file sync service
	pub fn file_sync_service(&self) -> Option<&Arc<crate::service::file_sync::FileSyncService>> {
		self.file_sync_service.get()
	}

	/// Initialize the file sync service (called during library setup)
	pub fn init_file_sync_service(self: &Arc<Self>) -> Result<()> {
		if self.file_sync_service.get().is_some() {
			warn!(
				"File sync service already initialized for library {}",
				self.id()
			);
			return Ok(());
		}

		let file_sync_service = crate::service::file_sync::FileSyncService::new(self.clone());

		self.file_sync_service
			.set(Arc::new(file_sync_service))
			.map_err(|_| {
				LibraryError::Other("File sync service already initialized".to_string())
			})?;

		debug!("File sync service initialized for library {}", self.id());

		Ok(())
	}

	/// Get core context
	pub fn core_context(&self) -> &Arc<crate::context::CoreContext> {
		&self.core_context
	}

	/// Initialize the sync service (called during library setup)
	#[cfg_attr(test, allow(dead_code))] // Exposed for integration tests
	pub async fn init_sync_service(
		&self,
		device_id: Uuid,
		network: Arc<dyn crate::infra::sync::NetworkTransport>,
	) -> Result<()> {
		if self.sync_service.get().is_some() {
			warn!(
				"Sync service already initialized for library {}, cannot replace transport. Transport: {}",
				self.id(),
				self.sync_service.get().unwrap().peer_sync().transport_name()
			);
			return Ok(());
		}

		let sync_service =
			crate::service::sync::SyncService::new_from_library(self, device_id, network)
				.await
				.map_err(|e| {
					LibraryError::Other(format!("Failed to create sync service: {}", e))
				})?;

		self.sync_service
			.set(Arc::new(sync_service))
			.map_err(|_| LibraryError::Other("Sync service already initialized".to_string()))?;

		// Start the sync service
		if let Some(service) = self.sync_service.get() {
			use crate::service::Service;
			service
				.start()
				.await
				.map_err(|e| LibraryError::Other(format!("Failed to start sync service: {}", e)))?;
		}

		Ok(())
	}

	/// Get a copy of the current configuration
	pub async fn config(&self) -> LibraryConfig {
		self.config.read().await.clone()
	}

	/// Update library configuration
	pub async fn update_config<F>(&self, f: F) -> Result<()>
	where
		F: FnOnce(&mut LibraryConfig),
	{
		let mut config = self.config.write().await;
		f(&mut config);
		config.updated_at = chrono::Utc::now();

		// Save to disk
		let config_path = self.path.join("library.json");
		let json = serde_json::to_string_pretty(&*config)?;
		tokio::fs::write(config_path, json).await?;

		Ok(())
	}

	/// Reload library configuration from disk
	pub async fn reload_config(&self) -> Result<()> {
		let config_path = self.path.join("library.json");
		let json = tokio::fs::read_to_string(config_path).await?;
		let config: LibraryConfig = serde_json::from_str(&json)?;

		let mut current_config = self.config.write().await;
		*current_config = config;

		// Note: Cannot call self.id() here as we still hold the write lock
		// The caller should log this if needed

		Ok(())
	}

	/// Save library configuration to disk
	pub async fn save_config(&self, config: &LibraryConfig) -> Result<()> {
		let config_path = self.path.join("library.json");
		let json = serde_json::to_string_pretty(config)?;
		tokio::fs::write(config_path, json).await?;
		Ok(())
	}

	/// Load device cache from library database
	/// Returns HashMap of device_slug → device_uuid for all devices in this library
	pub(crate) async fn load_device_cache_from_db(
		db: &crate::infra::db::Database,
	) -> Result<HashMap<String, Uuid>> {
		use crate::infra::db::entities;
		use sea_orm::EntityTrait;

		let devices = entities::device::Entity::find()
			.all(db.conn())
			.await
			.map_err(|e| LibraryError::Other(format!("Failed to load devices: {}", e)))?;

		let cache: HashMap<String, Uuid> = devices.into_iter().map(|d| (d.slug, d.uuid)).collect();

		debug!("Loaded {} devices into library cache", cache.len());

		Ok(cache)
	}

	/// Resolve device slug to UUID within this library
	/// Checks current device first, then library's device cache
	pub fn resolve_device_slug(&self, slug: &str) -> Option<Uuid> {
		// Priority 1: Check if it's the current device
		let current_device_id = crate::device::get_current_device_id();
		let current_device_slug = crate::device::get_current_device_slug();

		if slug == current_device_slug {
			return Some(current_device_id);
		}

		// Priority 2: Check library's device cache
		if let Ok(cache) = self.device_cache.read() {
			if let Some(device_id) = cache.get(slug).copied() {
				return Some(device_id);
			}
		}

		// Priority 3: Fall back to paired devices from networking layer
		// This allows file transfers between paired devices even if they're not in the library DB
		if let Ok(networking_guard) = self.core_context.networking.try_read() {
			if let Some(networking) = networking_guard.as_ref() {
				if let Ok(registry) = networking.device_registry().try_read() {
					// Check all devices in the registry for a matching slug
					for (device_id, state) in registry.get_all_devices() {
						let device_info = match state {
							crate::service::network::device::DeviceState::Paired {
								info, ..
							}
							| crate::service::network::device::DeviceState::Connected {
								info,
								..
							}
							| crate::service::network::device::DeviceState::Disconnected {
								info,
								..
							} => Some(info),
							_ => None,
						};

						if let Some(info) = device_info {
							if info.device_slug == slug {
								return Some(device_id);
							}
						}
					}
				}
			}
		}

		None
	}

	/// Reload device cache from database
	/// Called after device changes (e.g., sync updates, device registration)
	pub async fn reload_device_cache(&self) -> Result<()> {
		let new_cache = Self::load_device_cache_from_db(&self.db).await?;

		let mut cache = self
			.device_cache
			.write()
			.map_err(|_| LibraryError::Other("Device cache lock poisoned".to_string()))?;

		debug!(
			"Reloading device cache for library {}: {} devices",
			self.id(),
			new_cache.len()
		);
		*cache = new_cache;

		Ok(())
	}

	/// Add or update device in cache
	/// Called when device joins/updates via sync
	pub fn cache_device(&self, slug: String, device_id: Uuid) -> Result<()> {
		let mut cache = self
			.device_cache
			.write()
			.map_err(|_| LibraryError::Other("Device cache lock poisoned".to_string()))?;
		cache.insert(slug.clone(), device_id);
		debug!(
			"Cached device in library {}: {} -> {}",
			self.id(),
			slug,
			device_id
		);
		Ok(())
	}

	/// Ensure slug is unique within existing slugs
	/// Appends -2, -3, etc. if collision detected (like VolumeManager does)
	pub fn ensure_unique_slug(base_slug: &str, existing_slugs: &[String]) -> String {
		let mut candidate = base_slug.to_string();
		let mut counter = 2;

		while existing_slugs.contains(&candidate) {
			candidate = format!("{}-{}", base_slug, counter);
			counter += 1;

			if counter > 1000 {
				// Fallback: use UUID suffix if too many collisions
				let uuid_suffix = Uuid::new_v4()
					.to_string()
					.split('-')
					.next()
					.unwrap()
					.to_string();
				return format!("{}-{}", base_slug, uuid_suffix);
			}
		}

		candidate
	}

	/// Get the thumbnail directory for this library
	pub fn thumbnails_dir(&self) -> PathBuf {
		self.path.join("thumbnails")
	}

	/// Get the job logs directory for this library
	pub fn job_logs_dir(&self) -> PathBuf {
		self.path.join("logs")
	}

	/// Get the path for a specific thumbnail with size
	pub fn thumbnail_path(&self, cas_id: &str, size: u32) -> PathBuf {
		if cas_id.len() < 4 {
			// Fallback for short IDs
			return self
				.thumbnails_dir()
				.join(format!("{}_{}.webp", cas_id, size));
		}

		// Two-level sharding based on first four characters
		let shard1 = &cas_id[0..2];
		let shard2 = &cas_id[2..4];

		self.thumbnails_dir()
			.join(shard1)
			.join(shard2)
			.join(format!("{}_{}.webp", cas_id, size))
	}

	/// Get the path for any thumbnail size (legacy compatibility)
	pub fn thumbnail_path_legacy(&self, cas_id: &str) -> PathBuf {
		self.thumbnail_path(cas_id, 256) // Default to 256px
	}

	/// Save a thumbnail with specific size
	pub async fn save_thumbnail(&self, cas_id: &str, size: u32, data: &[u8]) -> Result<()> {
		let path = self.thumbnail_path(cas_id, size);

		// Ensure parent directory exists
		if let Some(parent) = path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Write thumbnail
		tokio::fs::write(path, data).await?;

		Ok(())
	}

	/// Check if a thumbnail exists for a specific size
	pub async fn has_thumbnail(&self, cas_id: &str, size: u32) -> bool {
		tokio::fs::metadata(self.thumbnail_path(cas_id, size))
			.await
			.is_ok()
	}

	/// Shutdown the library, gracefully stopping all jobs
	pub async fn shutdown(&self) -> Result<()> {
		debug!("Shutting down library {}", self.id());

		// Stop sync service
		if let Some(sync_service) = self.sync_service() {
			use crate::service::Service;
			if let Err(e) = sync_service.stop().await {
				warn!("Error stopping sync service: {}", e);
			}
		}

		// Shutdown the job manager, which will pause all running jobs
		self.jobs.shutdown().await?;

		// Save config to ensure any updates are persisted
		let config = self.config.read().await;
		self.save_config(&*config).await?;

		// Close library database connection properly
		debug!("Closing library database connection");

		// First, checkpoint the WAL file to merge it back into the main database
		use sea_orm::{ConnectionTrait, Statement};
		if let Err(e) = self
			.db
			.as_ref()
			.conn()
			.execute(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				"PRAGMA wal_checkpoint(TRUNCATE)",
			))
			.await
		{
			warn!("Failed to checkpoint WAL file: {}", e);
		} else {
			debug!("WAL file checkpointed successfully");
		}

		if let Err(e) = self.db.as_ref().conn().clone().close().await {
			warn!("Failed to close library database connection: {}", e);
		} else {
			debug!("Library database connection closed successfully");
		}

		// Clear device cache from DeviceManager
		if let Err(e) = self.core_context.device_manager.clear_paired_device_cache() {
			warn!("Failed to clear paired device cache: {}", e);
		}

		Ok(())
	}

	/// Delete the library, including all data
	pub async fn delete(&self) -> Result<bool> {
		// Shutdown the library
		self.shutdown().await?;

		// Delete the library directory if it exists
		if tokio::fs::metadata(self.path()).await.is_err() {
			return Ok(false);
		}

		tokio::fs::remove_dir_all(self.path()).await?;
		Ok(true)
	}

	/// Check if thumbnails exist for all specified sizes
	pub async fn has_all_thumbnails(&self, cas_id: &str, sizes: &[u32]) -> bool {
		for &size in sizes {
			if !self.has_thumbnail(cas_id, size).await {
				return false;
			}
		}
		true
	}

	/// Get thumbnail data for specific size
	pub async fn get_thumbnail(&self, cas_id: &str, size: u32) -> Result<Vec<u8>> {
		let path = self.thumbnail_path(cas_id, size);
		Ok(tokio::fs::read(path).await?)
	}

	/// Get the best available thumbnail (largest size available)
	pub async fn get_best_thumbnail(
		&self,
		cas_id: &str,
		preferred_sizes: &[u32],
	) -> Result<Option<(u32, Vec<u8>)>> {
		// Try sizes in descending order
		let mut sizes = preferred_sizes.to_vec();
		sizes.sort_by(|a, b| b.cmp(a));

		for &size in &sizes {
			if self.has_thumbnail(cas_id, size).await {
				let data = self.get_thumbnail(cas_id, size).await?;
				return Ok(Some((size, data)));
			}
		}

		Ok(None)
	}

	/// Start thumbnail generation job
	#[cfg(feature = "ffmpeg")]
	pub async fn generate_thumbnails(
		&self,
		entry_ids: Option<Vec<Uuid>>,
	) -> Result<crate::infra::job::handle::JobHandle> {
		use crate::ops::media::thumbnail::{ThumbnailJob, ThumbnailJobConfig};

		let config =
			ThumbnailJobConfig::from_sizes(self.config().await.settings.thumbnail_sizes.clone());

		let job = if let Some(ids) = entry_ids {
			ThumbnailJob::for_entries(ids, config)
		} else {
			ThumbnailJob::new(config)
		};

		self.jobs()
			.dispatch(job)
			.await
			.map_err(|e| LibraryError::JobError(e))
	}

	/// Update library statistics
	pub async fn update_statistics<F>(&self, f: F) -> Result<()>
	where
		F: FnOnce(&mut LibraryStatistics),
	{
		self.update_config(|config| {
			f(&mut config.statistics);
			config.statistics.updated_at = chrono::Utc::now();
		})
		.await
	}

	/// Get cached statistics immediately (non-blocking)
	pub async fn get_statistics(&self) -> LibraryStatistics {
		// Get library info before any potential locking
		let library_id = self.id();
		let library_name = self.name().await;

		// Try to reload config from disk to get latest statistics
		if let Err(e) = self.reload_config().await {
			debug!(
				library_id = %library_id,
				library_name = %library_name,
				error = %e,
				"Failed to reload config from disk, using cached statistics"
			);
		} else {
			debug!(
				library_id = %library_id,
				"Reloaded library configuration from disk"
			);
		}

		let stats = self.config.read().await.statistics.clone();

		// Trigger non-blocking recalculation
		if let Err(e) = self.recalculate_statistics().await {
			debug!(
				library_id = %library_id,
				library_name = %library_name,
				error = %e,
				"Failed to trigger statistics recalculation"
			);
		}

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			total_files = stats.total_files,
			total_size = stats.total_size,
			database_size = stats.database_size,
			updated_at = %stats.updated_at,
			"Retrieved library statistics"
		);

		stats
	}

	/// Calculate statistics directly from database (for queries)
	/// This is synchronous and queries the database directly for accurate real-time stats
	pub async fn calculate_statistics_for_query(&self) -> Result<LibraryStatistics> {
		let library_id = self.id();
		let library_name = self.name().await;

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			"Calculating statistics from database for query"
		);

		// Calculate all statistics from database
		let stats = self.calculate_all_statistics().await?;

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			total_files = stats.total_files,
			total_size = stats.total_size,
			location_count = stats.location_count,
			tag_count = stats.tag_count,
			device_count = stats.device_count,
			total_capacity = stats.total_capacity,
			available_capacity = stats.available_capacity,
			"Completed database statistics calculation for query"
		);

		Ok(stats)
	}

	/// Trigger async statistics recalculation
	pub async fn recalculate_statistics(&self) -> Result<()> {
		let library_id = self.id();
		let library_name = self.name().await;
		let event_bus = self.event_bus.clone();
		let path = self.path().to_path_buf();
		let db = self.db().clone();
		let config = self.config.read().await.clone();
		let config_lock = Arc::clone(&self.config);

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			"Starting async statistics recalculation for library"
		);

		// Spawn background task to calculate statistics
		tokio::spawn(async move {
			debug!(
				library_id = %library_id,
				library_name = %library_name,
				"Background statistics calculation task started"
			);

			if let Err(e) = Self::calculate_statistics_async_static(
				library_id,
				event_bus.clone(),
				path,
				db,
				config,
				config_lock,
			)
			.await
			{
				tracing::error!(
					library_id = %library_id,
					library_name = %library_name,
					error = %e,
					"Failed to calculate library statistics"
				);
			} else {
				// debug!(
				// 	library_id = %library_id,
				// 	library_name = %library_name,
				// 	"Background statistics calculation completed successfully"
				// );
			}
		});
		Ok(())
	}

	/// Calculate all statistics asynchronously (static version for background task)
	async fn calculate_statistics_async_static(
		library_id: Uuid,
		event_bus: Arc<EventBus>,
		path: PathBuf,
		db: Arc<Database>,
		mut config: LibraryConfig,
		config_lock: Arc<RwLock<LibraryConfig>>,
	) -> Result<()> {
		debug!(
			library_id = %library_id,
			library_name = %config.name,
			"Starting statistics calculation from database"
		);

		let mut stats = Self::calculate_all_statistics_static(&db, &path).await?;
		stats.updated_at = chrono::Utc::now();

		debug!(
			library_id = %library_id,
			library_name = %config.name,
			total_files = stats.total_files,
			total_size = stats.total_size,
			location_count = stats.location_count,
			tag_count = stats.tag_count,
			device_count = stats.device_count,
			total_capacity = stats.total_capacity,
			available_capacity = stats.available_capacity,
			thumbnail_count = stats.thumbnail_count,
			database_size = stats.database_size,
			"Calculated library statistics"
		);

		// Update config with new statistics
		config.statistics = stats.clone();
		config.statistics.updated_at = chrono::Utc::now();

		// Save config to disk
		let config_path = path.join("library.json");
		let json = serde_json::to_string_pretty(&config)?;
		tokio::fs::write(&config_path, json).await?;

		debug!(
			library_id = %library_id,
			library_name = %config.name,
			config_path = %config_path.display(),
			"Saved updated statistics to library.json"
		);

		// Update the in-memory config cache
		{
			let mut cached_config = config_lock.write().await;
			cached_config.statistics = stats.clone();
			debug!(
				library_id = %library_id,
				library_name = %config.name,
				"Updated in-memory config cache with new statistics"
			);
		}

		// Emit ResourceChanged event for normalizedCache using EventEmitter trait
		let library = crate::domain::Library::from_config(&config, path.clone());
		use crate::domain::resource::EventEmitter;
		if let Err(e) = library.emit_changed(&event_bus) {
			warn!(
				library_id = %library_id,
				error = %e,
				"Failed to emit library ResourceChanged event"
			);
		}

		// Also emit the legacy event for backwards compatibility
		event_bus.emit(crate::infra::event::Event::LibraryStatisticsUpdated {
			library_id,
			statistics: stats,
		});

		debug!(
			library_id = %library_id,
			library_name = %config.name,
			"Statistics calculation and save completed successfully, events emitted"
		);

		Ok(())
	}

	/// Calculate all statistics asynchronously
	async fn calculate_statistics_async(&self) -> Result<()> {
		let library_id = self.id();
		let library_name = self.name().await;

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			"Starting instance-based statistics calculation"
		);

		let mut stats = self.calculate_all_statistics().await?;
		stats.updated_at = chrono::Utc::now();

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			total_files = stats.total_files,
			total_size = stats.total_size,
			location_count = stats.location_count,
			tag_count = stats.tag_count,
			device_count = stats.device_count,
			total_capacity = stats.total_capacity,
			available_capacity = stats.available_capacity,
			thumbnail_count = stats.thumbnail_count,
			database_size = stats.database_size,
			"Calculated library statistics (instance method)"
		);

		// Update config with new statistics
		self.update_statistics(|existing_stats| {
			*existing_stats = stats.clone();
		})
		.await?;

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			"Updated and saved statistics via update_statistics method"
		);

		// Emit ResourceChanged event for normalizedCache using EventEmitter trait
		let config = self.config.read().await;
		let library = crate::domain::Library::from_config(&config, self.path().to_path_buf());
		drop(config);

		use crate::domain::resource::EventEmitter;
		if let Err(e) = library.emit_changed(&self.event_bus) {
			warn!(
				library_id = %library_id,
				error = %e,
				"Failed to emit library ResourceChanged event"
			);
		}

		// Also emit the legacy event for backwards compatibility
		self.event_bus
			.emit(crate::infra::event::Event::LibraryStatisticsUpdated {
				library_id: self.id(),
				statistics: stats,
			});

		debug!(
			library_id = %library_id,
			library_name = %library_name,
			"Instance-based statistics calculation completed successfully, events emitted"
		);

		Ok(())
	}

	/// Calculate all statistics from database (static version)
	async fn calculate_all_statistics_static(
		db: &Arc<Database>,
		path: &PathBuf,
	) -> Result<LibraryStatistics> {
		let db_conn = db.conn();

		debug!("Starting file statistics calculation");
		// Calculate file count and total size
		let (total_files, total_size) = Self::calculate_file_statistics_static(&db_conn).await?;
		debug!(
			total_files = total_files,
			total_size = total_size,
			"Completed file statistics calculation"
		);

		debug!("Starting location count calculation");
		// Calculate location count
		let location_count = Self::calculate_location_count_static(&db_conn).await?;
		debug!(
			location_count = location_count,
			"Completed location count calculation"
		);

		debug!("Starting tag count calculation");
		// Calculate tag count
		let tag_count = Self::calculate_tag_count_static(&db_conn).await?;
		debug!(tag_count = tag_count, "Completed tag count calculation");

		debug!("Starting device count calculation");
		// Calculate device count
		let device_count = Self::calculate_device_count_static(&db_conn).await?;
		debug!(
			device_count = device_count,
			"Completed device count calculation"
		);

		debug!("Starting unique content count calculation");
		// Calculate unique content count
		let unique_content_count = Self::calculate_unique_content_count_static(&db_conn).await?;
		debug!(
			unique_content_count = unique_content_count,
			"Completed unique content count calculation"
		);

		debug!("Starting content kind counts update");
		// Update content kind counts
		if let Err(e) = Self::update_content_kind_counts_static(&db_conn).await {
			warn!(
				error = %e,
				"Failed to update content kind counts"
			);
		} else {
			debug!("Completed content kind counts update");
		}

		debug!("Starting volume capacity calculation");
		// Calculate volume capacity
		let (total_capacity, available_capacity) =
			Self::calculate_volume_capacity_static(&db_conn).await?;
		debug!(
			total_capacity = total_capacity,
			available_capacity = available_capacity,
			"Completed volume capacity calculation"
		);

		debug!("Starting thumbnail count calculation");
		// Calculate thumbnail count
		let thumbnail_count = Self::calculate_thumbnail_count_static(path).await?;
		debug!(
			thumbnail_count = thumbnail_count,
			"Completed thumbnail count calculation"
		);

		debug!("Starting database size calculation");
		// Calculate database size
		let database_size = Self::calculate_database_size_static(path).await?;
		debug!(
			database_size = database_size,
			"Completed database size calculation"
		);

		Ok(LibraryStatistics {
			total_files,
			total_size,
			location_count,
			tag_count,
			device_count,
			unique_content_count,
			total_capacity,
			available_capacity,
			thumbnail_count,
			database_size,
			last_indexed: None, // Will be preserved from existing config
			updated_at: chrono::Utc::now(),
		})
	}

	/// Calculate all statistics from database
	async fn calculate_all_statistics(&self) -> Result<LibraryStatistics> {
		let db = self.db().conn();

		// Calculate file count and total size
		let (total_files, total_size) = self.calculate_file_statistics(db).await?;

		// Calculate location count
		let location_count = self.calculate_location_count(db).await?;

		// Calculate tag count
		let tag_count = self.calculate_tag_count(db).await?;

		// Calculate device count
		let device_count = self.calculate_device_count(db).await?;

		// Calculate unique content count
		let unique_content_count = self.calculate_unique_content_count(db).await?;

		// Calculate volume capacity
		let (total_capacity, available_capacity) = self.calculate_volume_capacity(db).await?;

		// Calculate thumbnail count
		let thumbnail_count = self.calculate_thumbnail_count().await?;

		// Calculate database size
		let database_size = self.calculate_database_size().await?;

		Ok(LibraryStatistics {
			total_files,
			total_size,
			location_count,
			tag_count,
			device_count,
			unique_content_count,
			total_capacity,
			available_capacity,
			thumbnail_count,
			database_size,
			last_indexed: self.config.read().await.statistics.last_indexed,
			updated_at: chrono::Utc::now(),
		})
	}

	/// Calculate file statistics from database
	async fn calculate_file_statistics(
		&self,
		db: &sea_orm::DatabaseConnection,
	) -> Result<(u64, u64)> {
		use crate::infra::db::entities::{entry, entry_closure, location};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		debug!("Starting file statistics calculation");

		// Get all location root entry IDs for this library
		let locations = location::Entity::find().all(db).await?;
		let location_root_entry_ids: Vec<i32> =
			locations.iter().filter_map(|l| l.entry_id).collect();

		debug!(
			location_count = locations.len(),
			location_root_entry_ids_count = location_root_entry_ids.len(),
			"Found locations for file statistics calculation"
		);

		if location_root_entry_ids.is_empty() {
			debug!("No locations found, returning zero file statistics");
			return Ok((0, 0));
		}

		// Get all descendant entry IDs using closure table
		let mut all_entry_ids = location_root_entry_ids.clone();
		for root_id in location_root_entry_ids {
			let descendant_ids = entry_closure::Entity::find()
				.filter(entry_closure::Column::AncestorId.eq(root_id))
				.all(db)
				.await?
				.into_iter()
				.map(|ec| ec.descendant_id)
				.collect::<Vec<i32>>();
			all_entry_ids.extend(descendant_ids);
		}

		debug!(
			total_entry_ids = all_entry_ids.len(),
			"Collected all entry IDs from closure table"
		);

		if all_entry_ids.is_empty() {
			debug!("No entries found, returning zero file statistics");
			return Ok((0, 0));
		}

		// Count files and sum their sizes
		let file_stats = entry::Entity::find()
			.filter(entry::Column::Id.is_in(all_entry_ids))
			.filter(entry::Column::Kind.eq(0)) // Files only
			.select_only()
			.column_as(entry::Column::Id.count(), "file_count")
			.column_as(entry::Column::Size.sum(), "total_size")
			.into_tuple::<(Option<i64>, Option<i64>)>()
			.one(db)
			.await?;

		let (file_count, total_size) = file_stats.unwrap_or((Some(0), Some(0)));
		let result = (
			file_count.unwrap_or(0) as u64,
			total_size.unwrap_or(0) as u64,
		);

		debug!(
			file_count = result.0,
			total_size = result.1,
			"Completed file statistics calculation"
		);

		Ok(result)
	}

	/// Calculate location count
	async fn calculate_location_count(&self, db: &sea_orm::DatabaseConnection) -> Result<u32> {
		use crate::infra::db::entities::location;
		use sea_orm::{EntityTrait, QueryTrait};

		debug!("Starting location count calculation");
		let locations = location::Entity::find().all(db).await?;
		let count = locations.len() as u32;

		debug!(
			location_count = count,
			"Completed location count calculation"
		);

		Ok(count)
	}

	/// Calculate tag count
	async fn calculate_tag_count(&self, db: &sea_orm::DatabaseConnection) -> Result<u32> {
		use crate::infra::db::entities::tag;
		use sea_orm::{EntityTrait, QueryTrait};

		debug!("Starting tag count calculation");
		let tags = tag::Entity::find().all(db).await?;
		let count = tags.len() as u32;

		debug!(tag_count = count, "Completed tag count calculation");

		Ok(count)
	}

	/// Calculate device count
	async fn calculate_device_count(&self, db: &sea_orm::DatabaseConnection) -> Result<u32> {
		use crate::infra::db::entities::device;
		use sea_orm::{EntityTrait, QueryTrait};

		debug!("Starting device count calculation");
		let devices = device::Entity::find().all(db).await?;
		let count = devices.len() as u32;

		debug!(device_count = count, "Completed device count calculation");

		Ok(count)
	}

	/// Calculate unique content count
	async fn calculate_unique_content_count(
		&self,
		db: &sea_orm::DatabaseConnection,
	) -> Result<u64> {
		use crate::infra::db::entities::content_identity;
		use sea_orm::{EntityTrait, PaginatorTrait};

		debug!("Starting unique content count calculation");
		let count = content_identity::Entity::find().count(db).await?;

		debug!(
			unique_content_count = count,
			"Completed unique content count calculation"
		);

		Ok(count)
	}

	/// Calculate volume capacity (total and available) across all volumes
	/// Only counts user-relevant volumes (Primary, UserData, External, Secondary)
	/// Excludes system volumes (VM, Recovery, Preboot, etc.)
	/// Excludes volumes that are subpaths of other volumes (e.g., /System/Volumes/Data/home inside /System/Volumes/Data)
	async fn calculate_volume_capacity(
		&self,
		db: &sea_orm::DatabaseConnection,
	) -> Result<(u64, u64)> {
		use crate::infra::db::entities::volume;
		use sea_orm::{EntityTrait, QueryTrait};

		debug!("Starting volume capacity calculation");
		let volumes = volume::Entity::find().all(db).await?;

		// First pass: filter to user-visible volumes
		let mut user_volumes: Vec<_> = volumes
			.into_iter()
			.filter(|vol| {
				let volume_type = vol.volume_type.as_deref().unwrap_or("Unknown");
				matches!(
					volume_type,
					"Primary" | "UserData" | "External" | "Secondary"
				)
			})
			.collect();

		// Deduplicate by fingerprint first (same physical volume tracked multiple times)
		let mut seen_fingerprints = std::collections::HashSet::new();
		user_volumes.retain(|v| seen_fingerprints.insert(v.fingerprint.clone()));

		// Sort by mount point length (shorter first) to detect parent volumes first
		user_volumes.sort_by_key(|v| v.mount_point.as_ref().map(|m| m.len()).unwrap_or(0));

		let mut total_capacity = 0u64;
		let mut available_capacity = 0u64;
		let mut counted_volumes = 0;
		let mut excluded_by_subpath = 0;

		let mut counted_mount_points: Vec<String> = Vec::new();

		for vol in user_volumes {
			let mount_point = match &vol.mount_point {
				Some(mp) => mp,
				None => continue,
			};

			// Check if this volume's mount point is a subpath of any already-counted volume
			let is_subpath = counted_mount_points
				.iter()
				.any(|parent| mount_point.starts_with(parent) && mount_point != parent);

			if is_subpath {
				debug!(
					volume_type = vol.volume_type.as_deref().unwrap_or("Unknown"),
					mount_point = ?mount_point,
					"Excluding volume: subpath of already-counted volume"
				);
				excluded_by_subpath += 1;
				continue;
			}

			// Count this volume
			if let Some(capacity) = vol.total_capacity {
				total_capacity = total_capacity.saturating_add(capacity as u64);
				counted_volumes += 1;
				counted_mount_points.push(mount_point.clone());
				debug!(
					volume_type = vol.volume_type.as_deref().unwrap_or("Unknown"),
					mount_point = ?mount_point,
					capacity = capacity,
					fingerprint = vol.fingerprint,
					"Counted volume"
				);
			}
			if let Some(available) = vol.available_capacity {
				available_capacity = available_capacity.saturating_add(available as u64);
			}
		}

		debug!(
			total_capacity = total_capacity,
			available_capacity = available_capacity,
			counted_volumes = counted_volumes,
			excluded_by_subpath = excluded_by_subpath,
			"Completed volume capacity calculation"
		);

		Ok((total_capacity, available_capacity))
	}

	/// Calculate thumbnail count by scanning thumbnail directory
	async fn calculate_thumbnail_count(&self) -> Result<u64> {
		let thumbnails_dir = self.thumbnails_dir();

		debug!(
			thumbnails_dir = %thumbnails_dir.display(),
			"Starting thumbnail count calculation"
		);

		if !thumbnails_dir.exists() {
			debug!("Thumbnails directory does not exist, returning zero count");
			return Ok(0);
		}

		let mut count = 0u64;
		let mut entries = tokio::fs::read_dir(&thumbnails_dir).await?;

		while let Some(entry) = entries.next_entry().await? {
			if entry.file_type().await?.is_dir() {
				// Recursively count files in subdirectories
				count += self.count_files_recursive(entry.path()).await?;
			} else if entry.file_name().to_string_lossy().ends_with(".webp") {
				count += 1;
			}
		}

		debug!(
			thumbnail_count = count,
			thumbnails_dir = %thumbnails_dir.display(),
			"Completed thumbnail count calculation"
		);

		Ok(count)
	}

	/// Count files recursively in a directory
	async fn count_files_recursive(&self, path: std::path::PathBuf) -> Result<u64> {
		Box::pin(self.count_files_recursive_impl(path)).await
	}

	async fn count_files_recursive_impl(&self, path: std::path::PathBuf) -> Result<u64> {
		let mut count = 0u64;
		let mut entries = tokio::fs::read_dir(&path).await?;

		while let Some(entry) = entries.next_entry().await? {
			if entry.file_type().await?.is_dir() {
				count += Box::pin(self.count_files_recursive_impl(entry.path())).await?;
			} else if entry.file_name().to_string_lossy().ends_with(".webp") {
				count += 1;
			}
		}

		Ok(count)
	}

	/// Calculate database file size
	async fn calculate_database_size(&self) -> Result<u64> {
		let db_path = self.path().join(LIBRARY_DB_FILENAME);

		debug!(
			db_path = %db_path.display(),
			"Starting database size calculation"
		);

		if db_path.exists() {
			let metadata = tokio::fs::metadata(&db_path).await?;
			let size = metadata.len();

			debug!(
				database_size = size,
				db_path = %db_path.display(),
				"Completed database size calculation"
			);

			Ok(size)
		} else {
			debug!(
				db_path = %db_path.display(),
				"Database file does not exist, returning zero size"
			);
			Ok(0)
		}
	}

	// Static versions of calculation methods for background tasks

	/// Calculate file statistics from database (static version)
	async fn calculate_file_statistics_static(
		db: &sea_orm::DatabaseConnection,
	) -> Result<(u64, u64)> {
		use crate::infra::db::entities::{entry, entry_closure, location};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		debug!("Starting file statistics calculation");

		// Get all location root entry IDs for this library
		debug!("Fetching location root entry IDs");
		let locations = location::Entity::find().all(db).await?;
		let location_root_entry_ids: Vec<i32> =
			locations.iter().filter_map(|l| l.entry_id).collect();
		debug!(
			location_count = locations.len(),
			"Found {} locations",
			locations.len()
		);

		if location_root_entry_ids.is_empty() {
			debug!("No locations found, returning zero file statistics");
			return Ok((0, 0));
		}

		// Calculate total size by summing aggregate_size of location root entries
		debug!("Calculating total size from location root aggregate sizes");
		let total_size_result = entry::Entity::find()
			.filter(entry::Column::Id.is_in(location_root_entry_ids.clone()))
			.select_only()
			.column_as(entry::Column::AggregateSize.sum(), "total_size")
			.into_tuple::<Option<i64>>()
			.one(db)
			.await?;

		// Calculate file count by counting ALL files in the library
		debug!("Calculating file count from all file entries");
		let file_count_result = entry::Entity::find()
			.filter(entry::Column::Kind.eq(0)) // Files only
			.select_only()
			.column_as(entry::Column::Id.count(), "file_count")
			.into_tuple::<Option<i64>>()
			.one(db)
			.await?;

		let total_size = total_size_result.unwrap_or(Some(0)).unwrap_or(0) as u64;
		let file_count = file_count_result.unwrap_or(Some(0)).unwrap_or(0) as u64;
		let result = (file_count, total_size);

		debug!(
			file_count = result.0,
			total_size = result.1,
			"Completed file statistics calculation"
		);
		Ok(result)
	}

	/// Calculate location count (static version)
	async fn calculate_location_count_static(db: &sea_orm::DatabaseConnection) -> Result<u32> {
		use crate::infra::db::entities::location;
		use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect, QueryTrait, Select};

		debug!("Executing location count query");
		let count = location::Entity::find().count(db).await?;
		debug!(
			location_count = count,
			"Location count query completed successfully"
		);
		Ok(count as u32)
	}

	/// Calculate tag count (static version)
	async fn calculate_tag_count_static(db: &sea_orm::DatabaseConnection) -> Result<u32> {
		use crate::infra::db::entities::tag;
		use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect, QueryTrait, Select};

		debug!("Executing tag count query");
		let count = tag::Entity::find().count(db).await?;
		debug!(tag_count = count, "Tag count query completed successfully");
		Ok(count as u32)
	}

	/// Calculate device count (static version)
	async fn calculate_device_count_static(db: &sea_orm::DatabaseConnection) -> Result<u32> {
		use crate::infra::db::entities::device;
		use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect, QueryTrait, Select};

		debug!("Executing device count query");
		let count = device::Entity::find().count(db).await?;
		debug!(
			device_count = count,
			"Device count query completed successfully"
		);
		Ok(count as u32)
	}

	/// Calculate unique content count (static version)
	async fn calculate_unique_content_count_static(
		db: &sea_orm::DatabaseConnection,
	) -> Result<u64> {
		use crate::infra::db::entities::content_identity;
		use sea_orm::{EntityTrait, PaginatorTrait};

		debug!("Executing unique content count query");
		let count = content_identity::Entity::find().count(db).await?;
		debug!(
			unique_content_count = count,
			"Unique content count query completed successfully"
		);
		Ok(count)
	}

	/// Update file counts for each content kind in the content_kinds table (static version)
	async fn update_content_kind_counts_static(db: &sea_orm::DatabaseConnection) -> Result<()> {
		use sea_orm::Statement;

		debug!("Starting content kind counts update");

		// Reset all counts to 0 first, then update with actual counts in a single query.
		// This handles both updates and resets efficiently.
		db.execute(Statement::from_string(
			sea_orm::DbBackend::Sqlite,
			"UPDATE content_kinds SET file_count = 0".to_owned(),
		))
		.await?;

		// Use raw SQL with GROUP BY to count efficiently in the database.
		// This avoids loading all content_identity records into memory.
		let rows_affected = db
			.execute(Statement::from_string(
				sea_orm::DbBackend::Sqlite,
				r#"
					UPDATE content_kinds
					SET file_count = (
						SELECT COUNT(*)
						FROM content_identities
						WHERE content_identities.kind_id = content_kinds.id
					)
				"#
				.to_owned(),
			))
			.await?
			.rows_affected();

		debug!(
			rows_affected = rows_affected,
			"Updated content kind file counts"
		);

		debug!("Content kind counts update completed");
		Ok(())
	}

	/// Calculate volume capacity (total and available) across all volumes (static version)
	/// Only counts user-relevant volumes (Primary, UserData, External, Secondary)
	/// Excludes system volumes (VM, Recovery, Preboot, etc.)
	/// Excludes volumes that are subpaths of other volumes (e.g., /System/Volumes/Data/home inside /System/Volumes/Data)
	async fn calculate_volume_capacity_static(
		db: &sea_orm::DatabaseConnection,
	) -> Result<(u64, u64)> {
		use crate::infra::db::entities::volume;
		use sea_orm::{EntityTrait, QueryTrait};

		debug!("Executing volume capacity query");
		let volumes = volume::Entity::find().all(db).await?;

		// First pass: filter to user-visible volumes
		let mut user_volumes: Vec<_> = volumes
			.into_iter()
			.filter(|vol| {
				let volume_type = vol.volume_type.as_deref().unwrap_or("Unknown");
				matches!(
					volume_type,
					"Primary" | "UserData" | "External" | "Secondary"
				)
			})
			.collect();

		// Deduplicate by fingerprint first (same physical volume tracked multiple times)
		let mut seen_fingerprints = std::collections::HashSet::new();
		user_volumes.retain(|v| seen_fingerprints.insert(v.fingerprint.clone()));

		// Sort by mount point length (shorter first) to detect parent volumes first
		user_volumes.sort_by_key(|v| v.mount_point.as_ref().map(|m| m.len()).unwrap_or(0));

		let mut total_capacity = 0u64;
		let mut available_capacity = 0u64;
		let mut counted_volumes = 0;
		let mut excluded_by_subpath = 0;

		let mut counted_mount_points: Vec<String> = Vec::new();

		for vol in user_volumes {
			let mount_point = match &vol.mount_point {
				Some(mp) => mp,
				None => continue,
			};

			// Check if this volume's mount point is a subpath of any already-counted volume
			let is_subpath = counted_mount_points
				.iter()
				.any(|parent| mount_point.starts_with(parent) && mount_point != parent);

			if is_subpath {
				debug!(
					volume_type = vol.volume_type.as_deref().unwrap_or("Unknown"),
					mount_point = ?mount_point,
					"Excluding volume: subpath of already-counted volume"
				);
				excluded_by_subpath += 1;
				continue;
			}

			// Count this volume
			if let Some(capacity) = vol.total_capacity {
				total_capacity = total_capacity.saturating_add(capacity as u64);
				counted_volumes += 1;
				counted_mount_points.push(mount_point.clone());
				debug!(
					volume_type = vol.volume_type.as_deref().unwrap_or("Unknown"),
					mount_point = ?mount_point,
					capacity = capacity,
					fingerprint = vol.fingerprint,
					"Counted volume"
				);
			}
			if let Some(available) = vol.available_capacity {
				available_capacity = available_capacity.saturating_add(available as u64);
			}
		}

		debug!(
			total_capacity = total_capacity,
			available_capacity = available_capacity,
			counted_volumes = counted_volumes,
			excluded_by_subpath = excluded_by_subpath,
			"Volume capacity query completed successfully"
		);

		Ok((total_capacity, available_capacity))
	}

	/// Calculate thumbnail count by scanning thumbnail directory (static version)
	async fn calculate_thumbnail_count_static(path: &PathBuf) -> Result<u64> {
		let thumbnails_dir = path.join("thumbnails");
		if !thumbnails_dir.exists() {
			return Ok(0);
		}

		let mut count = 0u64;
		let mut entries = tokio::fs::read_dir(&thumbnails_dir).await?;

		while let Some(entry) = entries.next_entry().await? {
			if entry.file_type().await?.is_dir() {
				// Recursively count files in subdirectories
				count += Self::count_files_recursive_static(entry.path()).await?;
			} else if entry.file_name().to_string_lossy().ends_with(".webp") {
				count += 1;
			}
		}

		Ok(count)
	}

	/// Count files recursively in a directory (static version)
	async fn count_files_recursive_static(path: std::path::PathBuf) -> Result<u64> {
		Box::pin(Self::count_files_recursive_static_impl(path)).await
	}

	async fn count_files_recursive_static_impl(path: std::path::PathBuf) -> Result<u64> {
		let mut count = 0u64;
		let mut entries = tokio::fs::read_dir(&path).await?;

		while let Some(entry) = entries.next_entry().await? {
			if entry.file_type().await?.is_dir() {
				count += Box::pin(Self::count_files_recursive_static_impl(entry.path())).await?;
			} else if entry.file_name().to_string_lossy().ends_with(".webp") {
				count += 1;
			}
		}

		Ok(count)
	}

	/// Calculate database file size (static version)
	async fn calculate_database_size_static(path: &PathBuf) -> Result<u64> {
		let db_path = path.join(LIBRARY_DB_FILENAME);
		if db_path.exists() {
			let metadata = tokio::fs::metadata(&db_path).await?;
			Ok(metadata.len())
		} else {
			Ok(0)
		}
	}
}

// Note: Library does not implement Clone due to the exclusive lock
// Use Arc<Library> when you need shared access

/// Current library configuration version
pub const LIBRARY_CONFIG_VERSION: u32 = 2;

/// Library directory extension
pub const LIBRARY_EXTENSION: &str = "sdlibrary";
