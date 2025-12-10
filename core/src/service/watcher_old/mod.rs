//! Location Watcher Service - Monitors file system changes in indexed locations

use crate::context::CoreContext;
use crate::infra::event::{Event, EventBus, FsRawEventKind};
use crate::service::Service;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

mod event_handler;
mod metrics;
mod platform;
pub mod utils;
mod worker;

#[cfg(feature = "examples")]
pub mod example;

pub use metrics::{LocationWorkerMetrics, MetricsCollector, WatcherMetrics};
pub use worker::LocationWorker;

pub use event_handler::WatcherEvent;
pub use platform::PlatformHandler;

/// Configuration for the location watcher
#[derive(Debug, Clone)]
pub struct LocationWatcherConfig {
	/// Debounce duration for file system events
	pub debounce_duration: Duration,
	/// Maximum number of events to buffer per location (never drops events)
	pub event_buffer_size: usize,
	/// Whether to enable detailed debug logging
	pub debug_mode: bool,
	/// Debounce window for batching events (100-250ms)
	pub debounce_window_ms: u64,
	/// Maximum batch size for processing efficiency
	pub max_batch_size: usize,
	/// Metrics logging interval
	pub metrics_log_interval_ms: u64,
	/// Whether to enable metrics collection
	pub enable_metrics: bool,
	/// Maximum queue depth before triggering re-index
	pub max_queue_depth_before_reindex: usize,
	/// Whether to enable focused re-indexing on overflow
	pub enable_focused_reindex: bool,
}

impl Default for LocationWatcherConfig {
	fn default() -> Self {
		Self {
			debounce_duration: Duration::from_millis(100),
			event_buffer_size: 100000, // Large buffer to never drop events
			debug_mode: false,
			debounce_window_ms: 150,        // 150ms default debounce window
			max_batch_size: 10000,          // Large batches for efficiency
			metrics_log_interval_ms: 30000, // 30 seconds
			enable_metrics: true,
			max_queue_depth_before_reindex: 50000, // 50% of buffer size
			enable_focused_reindex: true,
		}
	}
}

impl LocationWatcherConfig {
	/// Create a new configuration with custom values
	pub fn new(debounce_window_ms: u64, event_buffer_size: usize, max_batch_size: usize) -> Self {
		Self {
			debounce_duration: Duration::from_millis(100),
			event_buffer_size,
			debug_mode: false,
			debounce_window_ms,
			max_batch_size,
			metrics_log_interval_ms: 30000,
			enable_metrics: true,
			max_queue_depth_before_reindex: event_buffer_size / 2,
			enable_focused_reindex: true,
		}
	}

	/// Create a configuration optimized for resource-constrained environments
	/// This is for future resource manager integration
	pub fn resource_optimized(memory_quota: usize, cpu_quota: usize) -> Self {
		// Calculate buffer size based on available memory (1KB per event estimate)
		let event_buffer_size = std::cmp::max(10000, memory_quota / 1000);

		// Calculate batch size based on CPU quota (100 events per CPU unit)
		let max_batch_size = std::cmp::max(1000, cpu_quota / 100);

		Self {
			debounce_duration: Duration::from_millis(100),
			event_buffer_size,
			debug_mode: false,
			debounce_window_ms: 150,
			max_batch_size,
			metrics_log_interval_ms: 30000,
			enable_metrics: true,
			max_queue_depth_before_reindex: event_buffer_size / 2,
			enable_focused_reindex: true,
		}
	}

	/// Validate the configuration
	pub fn validate(&self) -> Result<()> {
		if self.debounce_window_ms < 50 {
			return Err(anyhow::anyhow!("Debounce window must be at least 50ms"));
		}
		if self.debounce_window_ms > 1000 {
			return Err(anyhow::anyhow!("Debounce window must be at most 1000ms"));
		}
		if self.event_buffer_size < 100 {
			return Err(anyhow::anyhow!("Event buffer size must be at least 100"));
		}
		if self.max_batch_size < 1 {
			return Err(anyhow::anyhow!("Max batch size must be at least 1"));
		}
		if self.max_batch_size > self.event_buffer_size {
			return Err(anyhow::anyhow!(
				"Max batch size cannot exceed event buffer size"
			));
		}
		Ok(())
	}
}

/// Location watcher service that monitors file system changes
pub struct LocationWatcher {
	/// Watcher configuration
	config: LocationWatcherConfig,
	/// Event bus for emitting events
	events: Arc<EventBus>,
	/// Core context for DB and library access
	context: Arc<CoreContext>,
	/// Currently watched locations
	watched_locations: Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
	/// Ephemeral watches (shallow, non-recursive) keyed by path
	ephemeral_watches: Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	/// File system watcher
	watcher: Arc<RwLock<Option<RecommendedWatcher>>>,
	/// Whether the service is running
	is_running: Arc<RwLock<bool>>,
	/// Platform-specific event handler
	platform_handler: Arc<PlatformHandler>,
	/// Per-location workers
	workers: Arc<RwLock<HashMap<Uuid, mpsc::Sender<WatcherEvent>>>>,
	/// Global watcher metrics
	metrics: Arc<WatcherMetrics>,
	/// Worker metrics by location
	worker_metrics: Arc<RwLock<HashMap<Uuid, Arc<LocationWorkerMetrics>>>>,
	/// Metrics collector for periodic logging
	metrics_collector: Arc<RwLock<Option<Arc<MetricsCollector>>>>,
}

/// Information about a watched location
#[derive(Debug, Clone)]
pub struct WatchedLocation {
	/// Location UUID
	pub id: Uuid,
	/// Library UUID this location belongs to
	pub library_id: Uuid,
	/// Path being watched
	pub path: PathBuf,
	/// Whether watching is enabled for this location
	pub enabled: bool,
	/// Indexing rule toggles for filtering events
	pub rule_toggles: crate::ops::indexing::rules::RuleToggles,
}

/// Information about an ephemeral watch (shallow, non-recursive)
#[derive(Debug, Clone)]
pub struct EphemeralWatch {
	/// Path being watched
	pub path: PathBuf,
	/// Indexing rule toggles for filtering events
	pub rule_toggles: crate::ops::indexing::rules::RuleToggles,
}

impl LocationWatcher {
	/// Create a new location watcher
	pub fn new(
		config: LocationWatcherConfig,
		events: Arc<EventBus>,
		context: Arc<CoreContext>,
	) -> Self {
		let platform_handler = Arc::new(PlatformHandler::new());

		Self {
			config,
			events,
			context,
			watched_locations: Arc::new(RwLock::new(HashMap::new())),
			ephemeral_watches: Arc::new(RwLock::new(HashMap::new())),
			watcher: Arc::new(RwLock::new(None)),
			is_running: Arc::new(RwLock::new(false)),
			platform_handler,
			workers: Arc::new(RwLock::new(HashMap::new())),
			metrics: Arc::new(WatcherMetrics::new()),
			worker_metrics: Arc::new(RwLock::new(HashMap::new())),
			metrics_collector: Arc::new(RwLock::new(None)),
		}
	}

	/// Ensure a worker exists for the given location
	async fn ensure_worker_for_location(
		&self,
		location_id: Uuid,
		library_id: Uuid,
	) -> Result<mpsc::Sender<WatcherEvent>> {
		// Check if worker already exists
		{
			let workers = self.workers.read().await;
			if let Some(sender) = workers.get(&location_id) {
				debug!(
					"Worker already exists for location {}, reusing",
					location_id
				);
				return Ok(sender.clone());
			}
		}

		info!("Creating new worker for location {}", location_id);

		// Get rule toggles and location root from watched locations
		let (rule_toggles, location_root) = {
			let locations = self.watched_locations.read().await;
			locations
				.get(&location_id)
				.map(|loc| (loc.rule_toggles, loc.path.clone()))
				.ok_or_else(|| {
					anyhow::anyhow!("Location {} not found in watched locations", location_id)
				})?
		};

		// Create metrics for this worker
		let worker_metrics = Arc::new(LocationWorkerMetrics::new());
		{
			let mut metrics_map = self.worker_metrics.write().await;
			metrics_map.insert(location_id, worker_metrics.clone());
		}

		// Create new worker
		let (tx, rx) = mpsc::channel(self.config.event_buffer_size);
		let worker = LocationWorker::new(
			location_id,
			library_id,
			rx,
			self.context.clone(),
			self.events.clone(),
			self.config.clone(),
			worker_metrics.clone(),
			rule_toggles,
			location_root,
		);

		// Record worker creation
		self.metrics.record_worker_created();

		// Register worker metrics with collector
		if let Some(collector) = self.metrics_collector.read().await.as_ref() {
			collector.add_worker_metrics(location_id, worker_metrics.clone());
		}

		// Spawn the worker task
		tokio::spawn(async move {
			if let Err(e) = worker.run().await {
				error!("Location worker {} failed: {}", location_id, e);
			}
		});

		// Store the sender
		{
			let mut workers = self.workers.write().await;
			workers.insert(location_id, tx.clone());
		}

		Ok(tx)
	}

	/// Remove a worker for a location
	async fn remove_worker_for_location(&self, location_id: Uuid) {
		let mut workers = self.workers.write().await;
		workers.remove(&location_id);

		// Remove metrics
		let mut metrics_map = self.worker_metrics.write().await;
		metrics_map.remove(&location_id);

		// Unregister from metrics collector
		if let Some(collector) = self.metrics_collector.read().await.as_ref() {
			collector.remove_worker_metrics(&location_id);
		}

		// Record worker destruction
		self.metrics.record_worker_destroyed();
	}

	/// Get metrics for a specific location
	pub async fn get_location_metrics(
		&self,
		location_id: Uuid,
	) -> Option<Arc<LocationWorkerMetrics>> {
		let metrics_map = self.worker_metrics.read().await;
		metrics_map.get(&location_id).cloned()
	}

	/// Get global watcher metrics
	pub fn get_global_metrics(&self) -> Arc<WatcherMetrics> {
		self.metrics.clone()
	}

	/// Manually trigger metrics logging (useful for testing)
	pub async fn log_metrics_now(&self) {
		// Log global metrics
		self.metrics.log_metrics();

		// Log worker metrics
		let worker_metrics = self.worker_metrics.read().await;
		for (location_id, metrics) in worker_metrics.iter() {
			metrics.log_metrics(*location_id);
		}
	}

	/// Start the metrics collector for periodic logging
	async fn start_metrics_collector(&self) -> Result<()> {
		if !self.config.enable_metrics {
			return Ok(());
		}

		let log_interval = Duration::from_millis(self.config.metrics_log_interval_ms);
		let metrics_collector = Arc::new(MetricsCollector::new(self.metrics.clone(), log_interval));

		// Store reference for worker registration first
		*self.metrics_collector.write().await = Some(metrics_collector.clone());

		// Start the metrics collection task
		tokio::spawn(async move {
			metrics_collector.start_collection().await;
		});

		info!(
			"Metrics collector started with {}ms interval",
			self.config.metrics_log_interval_ms
		);
		Ok(())
	}

	/// Add a location to watch
	pub async fn add_location(&self, location: WatchedLocation) -> Result<()> {
		if !location.enabled {
			debug!(
				"Location {} is disabled, not adding to watcher",
				location.id
			);
			return Ok(());
		}

		// Skip cloud locations - they don't have filesystem paths to watch
		// Cloud paths use service-native URIs like s3://, gdrive://, etc.
		let path_str = location.path.to_string_lossy();
		if path_str.contains("://") && !path_str.starts_with("local://") {
			debug!(
				"Skipping cloud location {} from filesystem watcher: {}",
				location.id, path_str
			);
			return Ok(());
		}

		// Verify this device owns the location (defense in depth)
		// This prevents watching locations owned by other devices
		let libraries = self.context.libraries().await;
		if let Some(library) = libraries.get_library(location.library_id).await {
			let db = library.db().conn();
			let current_device_uuid = crate::device::get_current_device_id();

			// Query the location to check ownership
			if let Ok(Some(location_record)) = crate::infra::db::entities::location::Entity::find()
				.filter(crate::infra::db::entities::location::Column::Uuid.eq(location.id))
				.one(db)
				.await
			{
				// Get the owning device
				if let Ok(Some(owning_device)) =
					crate::infra::db::entities::device::Entity::find_by_id(
						location_record.device_id,
					)
					.one(db)
					.await
				{
					if owning_device.uuid != current_device_uuid {
						warn!(
							"Refusing to watch location {} owned by device {} (current device: {})",
							location.id, owning_device.uuid, current_device_uuid
						);
						return Err(anyhow::anyhow!(
							"Cannot watch location {} - owned by different device",
							location.id
						));
					}
				}
			}
		}

		// First, add to watched_locations map
		{
			let mut locations = self.watched_locations.write().await;

			if locations.contains_key(&location.id) {
				warn!("Location {} is already being watched", location.id);
				return Ok(());
			}

			locations.insert(location.id, location.clone());
		} // Drop write lock here to avoid deadlock when ensure_worker_for_location reads it

		// Create worker for this location (after dropping write lock to avoid deadlock)
		if *self.is_running.read().await {
			self.ensure_worker_for_location(location.id, location.library_id)
				.await?;
		}

		// Register database connection for this location (needed for rename detection)
		let libraries = self.context.libraries().await;
		if let Some(library) = libraries.get_library(location.library_id).await {
			let db = library.db().conn().clone();
			self.platform_handler
				.register_location_db(location.id, db)
				.await;
			debug!(
				"Registered database connection for location {} (rename detection)",
				location.id
			);
		}

		// Add to file system watcher if running
		if *self.is_running.read().await {
			if let Some(watcher) = self.watcher.write().await.as_mut() {
				watcher.watch(&location.path, RecursiveMode::Recursive)?;
				info!("Started watching location: {}", location.path.display());
			}
		}

		Ok(())
	}

	/// Remove a location from watching
	pub async fn remove_location(&self, location_id: Uuid) -> Result<()> {
		let mut locations = self.watched_locations.write().await;

		if let Some(location) = locations.remove(&location_id) {
			// Remove worker for this location
			self.remove_worker_for_location(location_id).await;

			// Unregister database connection for this location
			self.platform_handler
				.unregister_location_db(location_id)
				.await;
			debug!(
				"Unregistered database connection for location {}",
				location_id
			);

			// Remove from file system watcher if running
			if *self.is_running.read().await {
				if let Some(watcher) = self.watcher.write().await.as_mut() {
					watcher.unwatch(&location.path)?;
					info!("Stopped watching location: {}", location.path.display());
				}
			}
		}

		Ok(())
	}

	/// Update a location's settings
	pub async fn update_location(&self, location_id: Uuid, enabled: bool) -> Result<()> {
		let mut locations = self.watched_locations.write().await;

		if let Some(location) = locations.get_mut(&location_id) {
			let was_enabled = location.enabled;
			location.enabled = enabled;

			if *self.is_running.read().await {
				if let Some(watcher) = self.watcher.write().await.as_mut() {
					match (was_enabled, enabled) {
						(false, true) => {
							// Enable watching
							watcher.watch(&location.path, RecursiveMode::Recursive)?;
							info!("Enabled watching for location: {}", location.path.display());
						}
						(true, false) => {
							// Disable watching
							watcher.unwatch(&location.path)?;
							info!(
								"Disabled watching for location: {}",
								location.path.display()
							);
						}
						_ => {} // No change needed
					}
				}
			}
		}

		Ok(())
	}

	/// Get all watched locations
	pub async fn get_watched_locations(&self) -> Vec<WatchedLocation> {
		self.watched_locations
			.read()
			.await
			.values()
			.cloned()
			.collect()
	}

	// ========================================================================
	// Ephemeral Watch Support (shallow, non-recursive)
	// ========================================================================

	/// Add an ephemeral watch for a directory (shallow, immediate children only).
	///
	/// Unlike location watches which are recursive, ephemeral watches only monitor
	/// immediate children of the watched directory. This is appropriate for ephemeral
	/// browsing where only the current directory's contents are indexed.
	///
	/// The path should already be indexed in the ephemeral cache before calling this.
	pub async fn add_ephemeral_watch(
		&self,
		path: PathBuf,
		rule_toggles: crate::ops::indexing::rules::RuleToggles,
	) -> Result<()> {
		// Check if path is valid
		if !path.exists() {
			return Err(anyhow::anyhow!(
				"Cannot watch non-existent path: {}",
				path.display()
			));
		}

		if !path.is_dir() {
			return Err(anyhow::anyhow!(
				"Cannot watch non-directory path: {}",
				path.display()
			));
		}

		// Check if already watching
		{
			let watches = self.ephemeral_watches.read().await;
			if watches.contains_key(&path) {
				debug!("Already watching ephemeral path: {}", path.display());
				return Ok(());
			}
		}

		// Register in ephemeral cache
		self.context
			.ephemeral_cache()
			.register_for_watching(path.clone());

		// Add to our tracking
		{
			let mut watches = self.ephemeral_watches.write().await;
			watches.insert(
				path.clone(),
				EphemeralWatch {
					path: path.clone(),
					rule_toggles,
				},
			);
			// Update metrics
			self.metrics.update_ephemeral_watches(watches.len());
		}

		// Add to file system watcher with NonRecursive mode
		if *self.is_running.read().await {
			if let Some(watcher) = self.watcher.write().await.as_mut() {
				watcher.watch(&path, RecursiveMode::NonRecursive)?;
				info!("Started shallow ephemeral watch for: {}", path.display());
			}
		}

		Ok(())
	}

	/// Remove an ephemeral watch
	pub async fn remove_ephemeral_watch(&self, path: &Path) -> Result<()> {
		let watch = {
			let mut watches = self.ephemeral_watches.write().await;
			let watch = watches.remove(path);
			// Update metrics
			self.metrics.update_ephemeral_watches(watches.len());
			watch
		};

		if let Some(watch) = watch {
			// Unregister from ephemeral cache
			self.context
				.ephemeral_cache()
				.unregister_from_watching(&watch.path);

			// Remove from file system watcher
			if *self.is_running.read().await {
				if let Some(watcher) = self.watcher.write().await.as_mut() {
					if let Err(e) = watcher.unwatch(&watch.path) {
						warn!(
							"Failed to unwatch ephemeral path {}: {}",
							watch.path.display(),
							e
						);
					} else {
						info!("Stopped ephemeral watch for: {}", watch.path.display());
					}
				}
			}
		}

		Ok(())
	}

	/// Get all ephemeral watches
	pub async fn get_ephemeral_watches(&self) -> Vec<EphemeralWatch> {
		self.ephemeral_watches
			.read()
			.await
			.values()
			.cloned()
			.collect()
	}

	/// Check if a path has an ephemeral watch
	pub async fn has_ephemeral_watch(&self, path: &Path) -> bool {
		self.ephemeral_watches.read().await.contains_key(path)
	}

	/// Find the ephemeral watch that covers a given path (if any).
	///
	/// For shallow watches, only returns a match if the path is an immediate
	/// child of a watched directory.
	pub async fn find_ephemeral_watch_for_path(&self, path: &Path) -> Option<EphemeralWatch> {
		let watches = self.ephemeral_watches.read().await;

		// Get the parent directory of the event path
		let parent = path.parent()?;

		// Check if the parent is being watched
		watches.get(parent).cloned()
	}

	/// Load existing locations from the database and add them to the watcher
	async fn load_existing_locations(&self) -> Result<()> {
		info!("Loading existing locations from database...");

		// Get all libraries from the context
		let libraries = self.context.libraries().await;
		let library_list = libraries.list().await;

		let mut total_locations = 0;

		for library in library_list {
			// Query locations for this library
			let db = library.db().conn();

			// Get current device UUID (this device)
			let current_device_uuid = crate::device::get_current_device_id();

			// First, get the current device's database ID by UUID
			let current_device = match crate::infra::db::entities::device::Entity::find()
				.filter(crate::infra::db::entities::device::Column::Uuid.eq(current_device_uuid))
				.one(db)
				.await
			{
				Ok(Some(device)) => device,
				Ok(None) => {
					warn!(
						"Current device {} not found in library {} database, skipping location loading",
						current_device_uuid,
						library.id()
					);
					continue;
				}
				Err(e) => {
					warn!(
						"Failed to query device {} in library {}: {}, skipping",
						current_device_uuid,
						library.id(),
						e
					);
					continue;
				}
			};

			// Add timeout to the database query
			// Only watch locations owned by THIS device
			let locations_result = tokio::time::timeout(
				std::time::Duration::from_secs(10),
				crate::infra::db::entities::location::Entity::find()
					.filter(
						crate::infra::db::entities::location::Column::DeviceId
							.eq(current_device.id),
					)
					.all(db),
			)
			.await;

			match locations_result {
				Ok(Ok(locations)) => {
					debug!(
						"Found {} locations in library {}",
						locations.len(),
						library.id()
					);

					for location in locations {
						// Skip locations without entry_id (not yet synced)
						let Some(entry_id) = location.entry_id else {
							debug!("Skipping location {} without entry_id", location.uuid);
							continue;
						};

						// Skip locations with IndexMode::None (not persistently indexed)
						if location.index_mode == "none" {
							debug!(
								"Skipping location {} with IndexMode::None (ephemeral browsing only)",
								location.uuid
							);
							continue;
						}

						// Get the full path using PathResolver with timeout
						let path_result = tokio::time::timeout(
							std::time::Duration::from_secs(5),
							crate::ops::indexing::path_resolver::PathResolver::get_full_path(
								db, entry_id,
							),
						)
						.await;

						match path_result {
							Ok(Ok(path)) => {
								// Skip cloud locations - they don't have filesystem paths to watch
								// Cloud paths use service-native URIs like s3://, gdrive://, etc.
								let path_str = path.to_string_lossy();
								if path_str.contains("://") && !path_str.starts_with("local://") {
									debug!(
										"Skipping cloud location {} from filesystem watcher: {}",
										location.uuid, path_str
									);
									continue;
								}

								// Register database connection for this location first
								let db = library.db().conn().clone();
								self.platform_handler
									.register_location_db(location.uuid, db)
									.await;

								// Convert database location to WatchedLocation
								let watched_location = WatchedLocation {
									id: location.uuid,
									library_id: library.id(),
									path: path.clone(),
									enabled: true,                    // TODO: Add enabled field to database schema
									rule_toggles: Default::default(), // Use default rules for existing locations
								};

								// Add to watched locations
								if let Err(e) = self.add_location(watched_location).await {
									warn!(
										"Failed to add location {} to watcher: {}",
										location.uuid, e
									);
								} else {
									total_locations += 1;
									debug!(
										"Added location {} to watcher: {} (with DB connection)",
										location.uuid,
										path.display()
									);
								}
							}
							Ok(Err(e)) => {
								warn!(
									"Failed to get path for location {}: {}, skipping",
									location.uuid, e
								);
							}
							Err(_) => {
								warn!(
									"Timeout getting path for location {}, skipping",
									location.uuid
								);
							}
						}
					}
				}
				Ok(Err(e)) => {
					warn!(
						"Database error loading locations for library {}: {}, continuing with other libraries",
						library.id(),
						e
					);
				}
				Err(_) => {
					warn!(
						"Timeout loading locations for library {}, continuing with other libraries",
						library.id()
					);
				}
			}
		}

		info!("Loaded {} locations from database", total_locations);

		// Update metrics with the total count
		self.metrics.update_total_locations(total_locations);

		Ok(())
	}

	/// Start the event processing loop
	async fn start_event_loop(&self) -> Result<()> {
		let platform_handler = self.platform_handler.clone();
		let watched_locations = self.watched_locations.clone();
		let ephemeral_watches = self.ephemeral_watches.clone();
		let workers = self.workers.clone();
		let is_running = self.is_running.clone();
		let debug_mode = self.config.debug_mode;
		let metrics = self.metrics.clone();
		let events = self.events.clone();
		let context = self.context.clone();

		let (tx, mut rx) = mpsc::channel(self.config.event_buffer_size);
		let tx_clone = tx.clone();

		// Create file system watcher
		let mut watcher =
			notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
				match res {
					Ok(event) => {
						// Always log raw events for now to debug rename issues
						debug!(
							"Raw notify event: kind={:?}, paths={:?}",
							event.kind, event.paths
						);

						// Record event received
						metrics.record_event_received();

						// Convert notify event to our WatcherEvent
						let watcher_event = WatcherEvent::from_notify_event(event);

						// Send event directly to avoid runtime context issues
						// Use try_send since we're in a sync context
						match tx_clone.try_send(watcher_event) {
							Ok(_) => {
								debug!("Successfully sent event to channel");
							}
							Err(e) => {
								error!("Failed to send watcher event: {}", e);
								// This could happen if the channel is full or receiver is dropped
							}
						}
					}
					Err(e) => {
						error!("File system watcher error: {}", e);
					}
				}
			})?;

		// Configure watcher
		watcher.configure(Config::default().with_poll_interval(Duration::from_millis(500)))?;

		// Watch all enabled locations and create workers
		let locations = watched_locations.read().await;
		for location in locations.values() {
			if location.enabled {
				watcher.watch(&location.path, RecursiveMode::Recursive)?;
				info!("Started watching location: {}", location.path.display());
			}
		}
		drop(locations);

		// Watch all ephemeral paths (non-recursive/shallow)
		let ephemeral = ephemeral_watches.read().await;
		for watch in ephemeral.values() {
			watcher.watch(&watch.path, RecursiveMode::NonRecursive)?;
			info!(
				"Started shallow ephemeral watch for: {}",
				watch.path.display()
			);
		}
		drop(ephemeral);

		// Store watcher
		*self.watcher.write().await = Some(watcher);

		// Start event processing loop
		tokio::spawn(async move {
			info!("Location watcher event loop task spawned");

			while *is_running.read().await {
				tokio::select! {
					Some(event) = rx.recv() => {
						debug!("Received event from channel: {:?}", event.kind);
						// Process the event through platform handler
						match platform_handler.process_event(event, &watched_locations, &ephemeral_watches).await {
							Ok(processed_events) => {
								for processed_event in processed_events {
									match processed_event {
										Event::FsRawChange { library_id, kind } => {
											// Emit the event to the event bus for subscribers
											events.emit(Event::FsRawChange {
												library_id,
												kind: kind.clone(),
											});

											// Extract path from event for location matching
											let event_path = match &kind {
												FsRawEventKind::Create { path } => Some(path.as_path()),
												FsRawEventKind::Modify { path } => Some(path.as_path()),
												FsRawEventKind::Remove { path } => Some(path.as_path()),
												FsRawEventKind::Rename { from, .. } => Some(from.as_path()),
											};

											// First, check if this is an ephemeral watch event
											// For shallow watches, only process if path is immediate child
											let mut handled_by_ephemeral = false;
											if let Some(event_path) = event_path {
												let parent = event_path.parent();
												if let Some(parent_path) = parent {
													let ephemeral = ephemeral_watches.read().await;
													if let Some(watch) = ephemeral.get(parent_path) {
														debug!(
															"Ephemeral watch match for {}: parent {} is watched",
															event_path.display(),
															parent_path.display()
														);
														handled_by_ephemeral = true;

														// Process via ephemeral handler
														let ctx = context.clone();
														let root = watch.path.clone();
														let toggles = watch.rule_toggles;
														let event_kind = kind.clone();

														debug!("Spawning ephemeral responder task for: {}", event_path.display());
														tokio::spawn(async move {
															debug!("Ephemeral responder task started");
															if let Err(e) = crate::ops::indexing::ephemeral::responder::apply(
																&ctx,
																&root,
																event_kind,
																toggles,
															).await {
																warn!("Failed to process ephemeral event: {}", e);
															} else {
																debug!("Ephemeral responder task completed successfully");
															}
														});
													} else {
														trace!("No ephemeral watch for parent: {}", parent_path.display());
													}
												}
											}

											// Skip location matching if handled by ephemeral
											if handled_by_ephemeral {
												continue;
											}

											// Find the location for this event by matching path prefix
											// CRITICAL: Must match by path, not just library_id, to avoid routing
											// events to the wrong location when multiple locations exist in one library
											let locations = watched_locations.read().await;
											let mut matched_location = None;
											let mut longest_match_len = 0;

											if let Some(event_path) = event_path {
												for location in locations.values() {
													if location.library_id == library_id && location.enabled {
														// Check if event path is under this location's root
														if event_path.starts_with(&location.path) {
															let match_len = location.path.as_os_str().len();
															// Use longest matching path to handle nested locations
															if match_len > longest_match_len {
																longest_match_len = match_len;
																matched_location = Some(location.id);
															}
														}
													}
												}
											}

											if let Some(location_id) = matched_location {
												if let Some(worker_tx) = workers.read().await.get(&location_id) {
													// Convert FsRawEventKind back to WatcherEvent for worker
													let watcher_event = match kind {
														FsRawEventKind::Create { path } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Create,
															paths: vec![path],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
														FsRawEventKind::Modify { path } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Modify,
															paths: vec![path],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
														FsRawEventKind::Remove { path } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Remove,
															paths: vec![path],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
														FsRawEventKind::Rename { from, to } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Rename { from, to },
															paths: vec![],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
													};

													debug!("Routing event to location {}: {:?}", location_id, watcher_event.kind);
													if let Err(e) = worker_tx.send(watcher_event).await {
														warn!("Failed to send event to worker for location {}: {}", location_id, e);
													} else {
														debug!("✓ Successfully sent event to worker for location {}", location_id);
													}
												} else {
													warn!("No worker found for matched location {}", location_id);
												}
											} else {
												warn!("No matching location found for event path: {:?}", event_path);
											}
										}
										other => {
											// Preserve emission of any other events
											// Note: We need access to events bus here, but it's not available in this scope
											// This will be handled by the workers when they emit final events
										}
									}
								}
							}
							Err(e) => {
								error!("Error processing watcher event: {}", e);
							}
						}
						trace!("Finished processing event, continuing loop");
					}
					_ = tokio::time::sleep(Duration::from_millis(100)) => {
						// Periodic tick for debouncing and cleanup
						if let Err(e) = platform_handler.tick().await {
							error!("Error during platform handler tick: {}", e);
						}

						// Handle platform-specific tick events that might generate additional events (e.g., rename matching)
						#[cfg(target_os = "macos")]
						{
							if let Ok(tick_events) = platform_handler.inner.tick_with_locations(&watched_locations, &ephemeral_watches).await {
								for tick_event in tick_events {
									match tick_event {
										Event::FsRawChange { library_id, kind } => {
											// Emit the event to the event bus for subscribers
											events.emit(Event::FsRawChange {
												library_id,
												kind: kind.clone(),
											});

											// Extract path from event for location matching
											let event_path = match &kind {
												FsRawEventKind::Create { path } => Some(path.as_path()),
												FsRawEventKind::Modify { path } => Some(path.as_path()),
												FsRawEventKind::Remove { path } => Some(path.as_path()),
												FsRawEventKind::Rename { from, .. } => Some(from.as_path()),
											};

											// Check if this is an ephemeral event first
											let mut handled_by_ephemeral = false;
											if let Some(event_path) = event_path {
												let parent = event_path.parent();
												if let Some(parent_path) = parent {
													let ephemeral = ephemeral_watches.read().await;
													if let Some(watch) = ephemeral.get(parent_path) {
														debug!(
															"Tick event: Ephemeral watch match for {}: parent {} is watched",
															event_path.display(),
															parent_path.display()
														);
														handled_by_ephemeral = true;

														// Process via ephemeral handler
														let ctx = context.clone();
														let root = watch.path.clone();
														let toggles = watch.rule_toggles;
														let event_kind = kind.clone();

														debug!("Tick event: Spawning ephemeral responder task for: {}", event_path.display());
														tokio::spawn(async move {
															debug!("Tick event: Ephemeral responder task started");
															if let Err(e) = crate::ops::indexing::ephemeral::responder::apply(
																&ctx,
																&root,
																event_kind,
																toggles,
															).await {
																warn!("Tick event: Failed to process ephemeral event: {}", e);
															} else {
																debug!("Tick event: Ephemeral responder task completed successfully");
															}
														});
													}
												}
											}

											// Skip location routing if handled by ephemeral
											if handled_by_ephemeral {
												continue;
											}

											// Find the location for this event by matching path prefix
											let locations = watched_locations.read().await;
											let mut matched_location = None;
											let mut longest_match_len = 0;

											if let Some(event_path) = event_path {
												for location in locations.values() {
													if location.library_id == library_id && location.enabled {
														if event_path.starts_with(&location.path) {
															let match_len = location.path.as_os_str().len();
															if match_len > longest_match_len {
																longest_match_len = match_len;
																matched_location = Some(location.id);
															}
														}
													}
												}
											}

											if let Some(location_id) = matched_location {
												if let Some(worker_tx) = workers.read().await.get(&location_id) {
													let watcher_event = match kind {
														FsRawEventKind::Create { path } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Create,
															paths: vec![path],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
														FsRawEventKind::Modify { path } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Modify,
															paths: vec![path],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
														FsRawEventKind::Remove { path } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Remove,
															paths: vec![path],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
														FsRawEventKind::Rename { from, to } => WatcherEvent {
															kind: event_handler::WatcherEventKind::Rename { from, to },
															paths: vec![],
															timestamp: std::time::SystemTime::now(),
															attrs: vec![],
														},
													};

													if let Err(e) = worker_tx.send(watcher_event).await {
														warn!("Failed to send tick event to worker for location {}: {}", location_id, e);
													}
												}
											}
										}
										_ => {
											// Other event types, if any
										}
									}
								}
							}
						}

						#[cfg(target_os = "windows")]
						{
							if let Ok(tick_events) = platform_handler.inner.tick_with_locations(&watched_locations).await {
								for tick_event in tick_events {
									// Similar handling for Windows if needed
									match tick_event {
										Event::FsRawChange { library_id, kind } => {
											events.emit(Event::FsRawChange {
												library_id,
												kind: kind.clone(),
											});
										}
										_ => {}
									}
								}
							}
						}
					}
				}
			}

			info!("Location watcher event loop stopped");
		});

		Ok(())
	}

	/// Start listening for LocationAdded events to dynamically add new locations
	async fn start_location_event_listener(&self) {
		let mut event_subscriber = self.events.subscribe();
		let watched_locations = self.watched_locations.clone();
		let watcher_ref = self.watcher.clone();
		let workers = self.workers.clone();
		let is_running = self.is_running.clone();
		let context = self.context.clone();
		let events = self.events.clone();
		let config = self.config.clone();
		let worker_metrics = self.worker_metrics.clone();
		let metrics = self.metrics.clone();
		let metrics_collector = self.metrics_collector.clone();
		let platform_handler = self.platform_handler.clone();

		tokio::spawn(async move {
			info!("Location event listener started");

			while *is_running.read().await {
				match event_subscriber.recv().await {
					Ok(Event::LocationAdded {
						library_id,
						location_id,
						path,
					}) => {
						info!(
							"Location added event received: {} at {}",
							location_id,
							path.display()
						);

						// Query the location to check its index_mode
						let libraries = context.libraries().await;
						let should_watch = if let Some(library) = libraries.get_library(library_id).await {
							let db = library.db().conn();
							match crate::infra::db::entities::location::Entity::find()
								.filter(crate::infra::db::entities::location::Column::Uuid.eq(location_id))
								.one(db)
								.await
							{
								Ok(Some(location_record)) => {
									if location_record.index_mode == "none" {
										debug!(
											"Skipping newly added location {} with IndexMode::None",
											location_id
										);
										false
									} else {
										true
									}
								}
								Ok(None) => {
									warn!("Location {} not found in database", location_id);
									false
								}
								Err(e) => {
									error!("Failed to query location {}: {}", location_id, e);
									false
								}
							}
						} else {
							warn!("Library {} not found for location {}", library_id, location_id);
							false
						};

						if !should_watch {
							continue;
						}

						// Create a temporary LocationWatcher instance for this operation
						let temp_watcher = LocationWatcher {
							config: config.clone(),
							events: events.clone(),
							context: context.clone(),
							watched_locations: watched_locations.clone(),
							ephemeral_watches: Arc::new(RwLock::new(HashMap::new())),
							watcher: watcher_ref.clone(),
							is_running: is_running.clone(),
							platform_handler: platform_handler.clone(),
							workers: workers.clone(),
							metrics: metrics.clone(),
							worker_metrics: worker_metrics.clone(),
							metrics_collector: metrics_collector.clone(),
						};

						// Create WatchedLocation and add to watcher
						let watched_location = WatchedLocation {
							id: location_id,
							library_id,
							path: path.clone(),
							enabled: true,
							rule_toggles: Default::default(), // Use default rules for new locations
						};

						// Add location to watcher
						if let Err(e) = temp_watcher.add_location(watched_location).await {
							error!("Failed to add location {} to watcher: {}", location_id, e);
						} else {
							info!(
								"Successfully added location {} to watcher: {}",
								location_id,
								path.display()
							);
						}
					}
					Ok(Event::LocationRemoved { location_id, .. }) => {
						info!("Location removed event received: {}", location_id);

						// Create a temporary LocationWatcher instance for this operation
						let temp_watcher = LocationWatcher {
							config: config.clone(),
							events: events.clone(),
							context: context.clone(),
							watched_locations: watched_locations.clone(),
							ephemeral_watches: Arc::new(RwLock::new(HashMap::new())),
							watcher: watcher_ref.clone(),
							is_running: is_running.clone(),
							platform_handler: platform_handler.clone(),
							workers: workers.clone(),
							metrics: metrics.clone(),
							worker_metrics: worker_metrics.clone(),
							metrics_collector: metrics_collector.clone(),
						};

						// Remove location from watcher
						if let Err(e) = temp_watcher.remove_location(location_id).await {
							error!(
								"Failed to remove location {} from watcher: {}",
								location_id, e
							);
						} else {
							info!("Successfully removed location {} from watcher", location_id);
						}
					}
					Ok(_) => {
						// Ignore other events
					}
					Err(e) => {
						// error!("Location event listener error: {}", e);
						// Continue listening despite errors
					}
				}
			}

			info!("Location event listener stopped");
		});
	}
}

#[async_trait::async_trait]
impl Service for LocationWatcher {
	async fn start(&self) -> Result<()> {
		if *self.is_running.read().await {
			warn!("Location watcher is already running");
			return Ok(());
		}

		info!("Starting location watcher service");

		*self.is_running.write().await = true;

		// Load existing locations from database
		if let Err(e) = self.load_existing_locations().await {
			error!("Failed to load existing locations: {}", e);
			// Continue starting the service even if loading locations fails
		}

		// Start listening for LocationAdded events
		self.start_location_event_listener().await;

		// Start metrics collector
		self.start_metrics_collector().await?;

		self.start_event_loop().await?;

		info!("Location watcher service started");
		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !*self.is_running.read().await {
			return Ok(());
		}

		info!("Stopping location watcher service");

		*self.is_running.write().await = false;

		// Clean up watcher
		*self.watcher.write().await = None;

		// Clean up all workers (dropping the senders will close the channels and stop the workers)
		let worker_count = {
			let mut workers = self.workers.write().await;
			let count = workers.len();
			workers.clear();
			count
		};

		info!("Stopped {} location workers", worker_count);

		// Clean up worker metrics
		{
			let mut metrics_map = self.worker_metrics.write().await;
			metrics_map.clear();
		}

		// Clean up metrics collector
		{
			*self.metrics_collector.write().await = None;
		}

		info!("Location watcher service stopped");
		Ok(())
	}

	fn is_running(&self) -> bool {
		// Use try_read to avoid blocking
		self.is_running.try_read().map_or(false, |guard| *guard)
	}

	fn name(&self) -> &'static str {
		"location_watcher"
	}
}

#[cfg(test)]
mod tests {
	use crate::ops::indexing::RuleToggles;

	use super::*;
	use tempfile::TempDir;

	fn create_test_events() -> Arc<EventBus> {
		Arc::new(EventBus::default())
	}

	fn create_mock_context() -> Arc<CoreContext> {
		// This would need to be implemented based on your CoreContext structure
		// For now, we'll use a placeholder
		todo!("Implement mock CoreContext for tests")
	}

	#[tokio::test]
	async fn test_location_watcher_creation() {
		let config = LocationWatcherConfig::default();
		let events = create_test_events();
		let context = create_mock_context();
		let watcher = LocationWatcher::new(config, events, context);

		assert!(!watcher.is_running());
		assert_eq!(watcher.name(), "location_watcher");
	}

	#[tokio::test]
	async fn test_add_remove_location() {
		let config = LocationWatcherConfig::default();
		let events = create_test_events();
		let context = create_mock_context();
		let watcher = LocationWatcher::new(config, events, context);

		let temp_dir = TempDir::new().unwrap();
		let location = WatchedLocation {
			id: Uuid::new_v4(),
			library_id: Uuid::new_v4(),
			path: temp_dir.path().to_path_buf(),
			enabled: true,
			rule_toggles: RuleToggles::default(),
		};

		let location_id = location.id;

		// Add location
		watcher.add_location(location).await.unwrap();

		let locations = watcher.get_watched_locations().await;
		assert_eq!(locations.len(), 1);
		assert_eq!(locations[0].id, location_id);

		// Remove location
		watcher.remove_location(location_id).await.unwrap();

		let locations = watcher.get_watched_locations().await;
		assert_eq!(locations.len(), 0);
	}
}
