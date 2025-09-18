//! Location Watcher Service - Monitors file system changes in indexed locations

use crate::context::CoreContext;
use crate::infra::event::{Event, EventBus, FsRawEventKind};
use crate::service::Service;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

mod event_handler;
mod metrics;
mod platform;
mod worker;
pub mod utils;

#[cfg(test)]
mod tests;

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
			debounce_window_ms: 150, // 150ms default debounce window
			max_batch_size: 10000, // Large batches for efficiency
			metrics_log_interval_ms: 30000, // 30 seconds
			enable_metrics: true,
			max_queue_depth_before_reindex: 50000, // 50% of buffer size
			enable_focused_reindex: true,
		}
	}
}

impl LocationWatcherConfig {
	/// Create a new configuration with custom values
	pub fn new(
		debounce_window_ms: u64,
		event_buffer_size: usize,
		max_batch_size: usize,
	) -> Self {
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
	pub fn resource_optimized(
		memory_quota: usize,
		cpu_quota: usize,
	) -> Self {
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
			return Err(anyhow::anyhow!("Max batch size cannot exceed event buffer size"));
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
}

impl LocationWatcher {
	/// Create a new location watcher
	pub fn new(config: LocationWatcherConfig, events: Arc<EventBus>, context: Arc<CoreContext>) -> Self {
		let platform_handler = Arc::new(PlatformHandler::new());

		Self {
			config,
			events,
			context,
			watched_locations: Arc::new(RwLock::new(HashMap::new())),
			watcher: Arc::new(RwLock::new(None)),
			is_running: Arc::new(RwLock::new(false)),
			platform_handler,
			workers: Arc::new(RwLock::new(HashMap::new())),
			metrics: Arc::new(WatcherMetrics::new()),
			worker_metrics: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Ensure a worker exists for the given location
	async fn ensure_worker_for_location(&self, location_id: Uuid, library_id: Uuid) -> Result<mpsc::Sender<WatcherEvent>> {
		// Check if worker already exists
		{
			let workers = self.workers.read().await;
			if let Some(sender) = workers.get(&location_id) {
				return Ok(sender.clone());
			}
		}

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
		);

		// Record worker creation
		self.metrics.record_worker_created();

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
		
		// Record worker destruction
		self.metrics.record_worker_destroyed();
	}

	/// Get metrics for a specific location
	pub async fn get_location_metrics(&self, location_id: Uuid) -> Option<Arc<LocationWorkerMetrics>> {
		let metrics_map = self.worker_metrics.read().await;
		metrics_map.get(&location_id).cloned()
	}

	/// Get global watcher metrics
	pub fn get_global_metrics(&self) -> Arc<WatcherMetrics> {
		self.metrics.clone()
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

		let mut locations = self.watched_locations.write().await;

		if locations.contains_key(&location.id) {
			warn!("Location {} is already being watched", location.id);
			return Ok(());
		}

		// Create worker for this location
		if *self.is_running.read().await {
			self.ensure_worker_for_location(location.id, location.library_id).await?;
		}

		// Add to file system watcher if running
		if *self.is_running.read().await {
			if let Some(watcher) = self.watcher.write().await.as_mut() {
				watcher.watch(&location.path, RecursiveMode::Recursive)?;
				info!("Started watching location: {}", location.path.display());
			}
		}

		locations.insert(location.id, location);
		Ok(())
	}

	/// Remove a location from watching
	pub async fn remove_location(&self, location_id: Uuid) -> Result<()> {
		let mut locations = self.watched_locations.write().await;

		if let Some(location) = locations.remove(&location_id) {
			// Remove worker for this location
			self.remove_worker_for_location(location_id).await;

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

	/// Start the event processing loop
	async fn start_event_loop(&self) -> Result<()> {
		let platform_handler = self.platform_handler.clone();
		let watched_locations = self.watched_locations.clone();
		let workers = self.workers.clone();
		let is_running = self.is_running.clone();
		let debug_mode = self.config.debug_mode;
		let metrics = self.metrics.clone();

		let (tx, mut rx) = mpsc::channel(self.config.event_buffer_size);

		// Create file system watcher
		let mut watcher = notify::recommended_watcher(move |res| {
			match res {
				Ok(event) => {
					if debug_mode {
						debug!("Raw file system event: {:?}", event);
					}

					// Record event received
					metrics.record_event_received();

					// Convert notify event to our WatcherEvent
					let watcher_event = WatcherEvent::from_notify_event(event);

					// Use spawn_blocking to avoid blocking the notify callback
					// This ensures we never drop events - we wait for buffer space
					let tx_clone = tx.clone();
					tokio::spawn(async move {
						if let Err(e) = tx_clone.send(watcher_event).await {
							error!("Failed to send watcher event (receiver dropped): {}", e);
							// This should only happen if the receiver is dropped
						}
					});
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

		// Store watcher
		*self.watcher.write().await = Some(watcher);

		// Start event processing loop
		tokio::spawn(async move {
			while *is_running.read().await {
				tokio::select! {
					Some(event) = rx.recv() => {
						// Process the event through platform handler
						match platform_handler.process_event(event, &watched_locations).await {
							Ok(processed_events) => {
								for processed_event in processed_events {
									match processed_event {
										Event::FsRawChange { library_id, kind } => {
											// Find the location for this event and route to worker
											let locations = watched_locations.read().await;
											for location in locations.values() {
												if location.library_id == library_id {
													if let Some(worker_tx) = workers.read().await.get(&location.id) {
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
														
														if let Err(e) = worker_tx.send(watcher_event).await {
															warn!("Failed to send event to worker for location {}: {}", location.id, e);
														}
														break;
													}
												}
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
					}
					_ = tokio::time::sleep(Duration::from_millis(100)) => {
						// Periodic tick for debouncing and cleanup
						if let Err(e) = platform_handler.tick().await {
							error!("Error during platform handler tick: {}", e);
						}

						// Handle platform-specific tick events that might generate additional events
						#[cfg(target_os = "macos")]
						{
							if let Ok(tick_events) = platform_handler.inner.tick_with_locations(&watched_locations).await {
								for tick_event in tick_events {
									// Note: We need access to events bus here, but it's not available in this scope
									// This will be handled by the workers when they emit final events
								}
							}
						}

						#[cfg(target_os = "windows")]
						{
							if let Ok(tick_events) = platform_handler.inner.tick_with_locations(&watched_locations).await {
								for tick_event in tick_events {
									// Note: We need access to events bus here, but it's not available in this scope
									// This will be handled by the workers when they emit final events
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
	use super::*;
	use tempfile::TempDir;

	fn create_test_events() -> Arc<EventBus> {
		Arc::new(EventBus::default())
	}

	#[tokio::test]
	async fn test_location_watcher_creation() {
		let config = LocationWatcherConfig::default();
		let events = create_test_events();
		let watcher = LocationWatcher::new(config, events);

		assert!(!watcher.is_running());
		assert_eq!(watcher.name(), "location_watcher");
	}

	#[tokio::test]
	async fn test_add_remove_location() {
		let config = LocationWatcherConfig::default();
		let events = create_test_events();
		let watcher = LocationWatcher::new(config, events);

		let temp_dir = TempDir::new().unwrap();
		let location = WatchedLocation {
			id: Uuid::new_v4(),
			library_id: Uuid::new_v4(),
			path: temp_dir.path().to_path_buf(),
			enabled: true,
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
