//! Persistent event handler
//!
//! Subscribes to filesystem events and routes them to location workers
//! for batched database persistence. Used for indexed locations.
//!
//! ## Characteristics
//!
//! - **Recursive watching**: Processes events for entire directory trees
//! - **Batching**: Events are collected and processed in batches for efficiency
//! - **Location-scoped**: Events are routed to the appropriate location's worker

use crate::context::CoreContext;
use crate::ops::indexing::responder;
use crate::ops::indexing::rules::RuleToggles;
use crate::service::watcher::FsWatcherService;
use anyhow::Result;
use sd_fs_watcher::FsEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Metadata for a watched location
#[derive(Debug, Clone)]
pub struct LocationMeta {
	/// Location UUID
	pub id: Uuid,
	/// Library UUID this location belongs to
	pub library_id: Uuid,
	/// Root path of the location
	pub root_path: PathBuf,
	/// Indexing rule toggles
	pub rule_toggles: RuleToggles,
}

/// Configuration for the persistent event handler
#[derive(Debug, Clone)]
pub struct PersistentHandlerConfig {
	/// Debounce window for batching events (ms)
	pub debounce_window_ms: u64,
	/// Maximum batch size
	pub max_batch_size: usize,
	/// Worker channel buffer size
	pub worker_buffer_size: usize,
}

impl Default for PersistentHandlerConfig {
	fn default() -> Self {
		Self {
			debounce_window_ms: 150,
			max_batch_size: 10000,
			worker_buffer_size: 100000,
		}
	}
}

/// Handler for persistent (database-backed) filesystem events
///
/// Subscribes to `FsWatcher` events, filters by location scope,
/// and routes to per-location workers for batched processing.
pub struct PersistentEventHandler {
	/// Core context for database access
	context: Arc<CoreContext>,
	/// Reference to the filesystem watcher service (set via connect())
	fs_watcher: RwLock<Option<Arc<FsWatcherService>>>,
	/// Registered locations (root_path -> meta)
	locations: Arc<RwLock<HashMap<PathBuf, LocationMeta>>>,
	/// Per-location worker channels
	workers: Arc<RwLock<HashMap<Uuid, mpsc::Sender<FsEvent>>>>,
	/// Whether the handler is running
	is_running: Arc<AtomicBool>,
	/// Configuration
	config: PersistentHandlerConfig,
}

impl PersistentEventHandler {
	/// Create a new persistent event handler (unconnected)
	///
	/// Call `connect()` to attach to a FsWatcherService before starting.
	pub fn new_unconnected(context: Arc<CoreContext>) -> Self {
		Self {
			context,
			fs_watcher: RwLock::new(None),
			locations: Arc::new(RwLock::new(HashMap::new())),
			workers: Arc::new(RwLock::new(HashMap::new())),
			is_running: Arc::new(AtomicBool::new(false)),
			config: PersistentHandlerConfig::default(),
		}
	}

	/// Create a new persistent event handler (connected)
	pub fn new(context: Arc<CoreContext>, fs_watcher: Arc<FsWatcherService>) -> Self {
		Self {
			context,
			fs_watcher: RwLock::new(Some(fs_watcher)),
			locations: Arc::new(RwLock::new(HashMap::new())),
			workers: Arc::new(RwLock::new(HashMap::new())),
			is_running: Arc::new(AtomicBool::new(false)),
			config: PersistentHandlerConfig::default(),
		}
	}

	/// Connect to a FsWatcherService
	pub async fn connect(&self, fs_watcher: Arc<FsWatcherService>) {
		*self.fs_watcher.write().await = Some(fs_watcher);
	}

	/// Register a location for persistent indexing
	pub async fn add_location(&self, meta: LocationMeta) -> Result<()> {
		let location_id = meta.id;
		let root_path = meta.root_path.clone();

		info!(
			"Registering location {} at {}",
			location_id,
			root_path.display()
		);

		// Add to locations map
		{
			let mut locations = self.locations.write().await;
			locations.insert(root_path.clone(), meta.clone());
			debug!(
				"Added location to map. Total locations: {}",
				locations.len()
			);
		}

		// Create worker if handler is running
		if self.is_running.load(Ordering::SeqCst) {
			debug!(
				"Handler is running, creating worker for location {}",
				location_id
			);
			self.ensure_worker(meta).await?;
		} else {
			debug!(
				"Handler not running yet, worker will be created on start for location {}",
				location_id
			);
		}

		// Register path with FsWatcher if connected
		if let Some(fs_watcher) = self.fs_watcher.read().await.as_ref() {
			debug!(
				"Registering path {} with FsWatcher (recursive)",
				root_path.display()
			);
			fs_watcher
				.watch_path(&root_path, sd_fs_watcher::WatchConfig::recursive())
				.await?;
			info!(
				"Successfully registered {} with FsWatcher for location {}",
				root_path.display(),
				location_id
			);
		} else {
			warn!(
				"FsWatcher not connected, cannot watch path {} for location {}",
				root_path.display(),
				location_id
			);
		}

		Ok(())
	}

	/// Unregister a location
	pub async fn remove_location(&self, location_id: Uuid) -> Result<()> {
		info!("Unregistering location {}", location_id);

		// Find and remove the location
		let root_path = {
			let mut locations = self.locations.write().await;
			let path = locations
				.iter()
				.find(|(_, meta)| meta.id == location_id)
				.map(|(path, _)| path.clone());

			if let Some(path) = &path {
				locations.remove(path);
			}
			path
		};

		// Remove worker
		{
			let mut workers = self.workers.write().await;
			workers.remove(&location_id);
		}

		// Unwatch path if connected
		if let Some(path) = root_path {
			if let Some(fs_watcher) = self.fs_watcher.read().await.as_ref() {
				if let Err(e) = fs_watcher.unwatch_path(&path).await {
					warn!("Failed to unwatch path {}: {}", path.display(), e);
				}
			}
		}

		Ok(())
	}

	/// Get all registered locations
	pub async fn locations(&self) -> Vec<LocationMeta> {
		self.locations.read().await.values().cloned().collect()
	}

	/// Start the event handler
	pub async fn start(&self) -> Result<()> {
		if self.is_running.swap(true, Ordering::SeqCst) {
			warn!("PersistentEventHandler is already running");
			return Ok(());
		}

		let fs_watcher = self.fs_watcher.read().await.clone();
		let Some(fs_watcher) = fs_watcher else {
			return Err(anyhow::anyhow!(
				"PersistentEventHandler not connected to FsWatcherService"
			));
		};

		debug!("Starting PersistentEventHandler");

		// Create workers for all registered locations AND register paths with FsWatcher
		// This is critical: locations may have been added before start() was called,
		// when the FsWatcher wasn't connected yet, so we need to register them now.
		let locations: Vec<LocationMeta> = self.locations.read().await.values().cloned().collect();
		for meta in &locations {
			self.ensure_worker(meta.clone()).await?;

			// Register the path with the OS-level watcher (may have been skipped during add_location)
			debug!(
				"Registering path {} with FsWatcher for location {}",
				meta.root_path.display(),
				meta.id
			);
			if let Err(e) = fs_watcher
				.watch_path(&meta.root_path, sd_fs_watcher::WatchConfig::recursive())
				.await
			{
				error!(
					"Failed to register path {} with FsWatcher: {}",
					meta.root_path.display(),
					e
				);
			} else {
				info!(
					"Successfully registered {} with FsWatcher for location {}",
					meta.root_path.display(),
					meta.id
				);
			}
		}

		// Start the event routing task
		let mut rx = fs_watcher.subscribe();
		let locations = self.locations.clone();
		let workers = self.workers.clone();
		let is_running = self.is_running.clone();

		tokio::spawn(async move {
			debug!("PersistentEventHandler routing task started");

			while is_running.load(Ordering::SeqCst) {
				match rx.recv().await {
					Ok(event) => {
						if let Err(e) = Self::route_event(&event, &locations, &workers).await {
							error!("Error routing persistent event: {}", e);
						}
					}
					Err(broadcast::error::RecvError::Lagged(n)) => {
						warn!("PersistentEventHandler lagged by {} events", n);
					}
					Err(broadcast::error::RecvError::Closed) => {
						debug!("FsWatcher channel closed, stopping PersistentEventHandler");
						break;
					}
				}
			}

			debug!("PersistentEventHandler routing task stopped");
		});

		Ok(())
	}

	/// Stop the event handler
	pub async fn stop(&self) {
		debug!("Stopping PersistentEventHandler");
		self.is_running.store(false, Ordering::SeqCst);

		// Clear workers (dropping senders will stop worker tasks)
		let mut workers = self.workers.write().await;
		workers.clear();
	}

	/// Check if the handler is running
	pub fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	/// Ensure a worker exists for a location
	async fn ensure_worker(&self, meta: LocationMeta) -> Result<()> {
		let mut workers = self.workers.write().await;
		if workers.contains_key(&meta.id) {
			return Ok(());
		}

		debug!("Creating worker for location {}", meta.id);

		let (tx, rx) = mpsc::channel(self.config.worker_buffer_size);
		workers.insert(meta.id, tx);

		// Spawn worker task
		let context = self.context.clone();
		let config = self.config.clone();

		tokio::spawn(async move {
			if let Err(e) = Self::run_worker(rx, meta, context, config).await {
				error!("Location worker failed: {}", e);
			}
		});

		Ok(())
	}

	/// Route an event to the appropriate location worker
	async fn route_event(
		event: &FsEvent,
		locations: &Arc<RwLock<HashMap<PathBuf, LocationMeta>>>,
		workers: &Arc<RwLock<HashMap<Uuid, mpsc::Sender<FsEvent>>>>,
	) -> Result<()> {
		let locs = locations.read().await;

		trace!(
			"Routing event {:?} for path: {} (checking {} locations)",
			event.kind,
			event.path.display(),
			locs.len()
		);

		// Find the best matching location (longest prefix match)
		let mut best_match: Option<&LocationMeta> = None;
		let mut longest_len = 0;

		for (root_path, meta) in locs.iter() {
			trace!(
				"  Checking location {} at {} for path {}",
				meta.id,
				root_path.display(),
				event.path.display()
			);
			if event.path.starts_with(root_path) {
				let len = root_path.as_os_str().len();
				if len > longest_len {
					longest_len = len;
					best_match = Some(meta);
				}
			}
		}

		let Some(location) = best_match else {
			debug!(
				"Event not under any location: {} (registered locations: {:?})",
				event.path.display(),
				locs.keys().collect::<Vec<_>>()
			);
			return Ok(());
		};

		debug!(
			"Routing {:?} event for {} to location {}",
			event.kind,
			event.path.display(),
			location.id
		);

		// Send to worker
		let workers_map = workers.read().await;
		if let Some(tx) = workers_map.get(&location.id) {
			if let Err(e) = tx.send(event.clone()).await {
				warn!(
					"Failed to send event to worker for location {}: {}",
					location.id, e
				);
			}
		} else {
			warn!(
				"No worker found for location {} (workers: {:?})",
				location.id,
				workers_map.keys().collect::<Vec<_>>()
			);
		}

		Ok(())
	}

	/// Run the location worker (batching + responder calls)
	async fn run_worker(
		mut rx: mpsc::Receiver<FsEvent>,
		meta: LocationMeta,
		context: Arc<CoreContext>,
		config: PersistentHandlerConfig,
	) -> Result<()> {
		info!("Location worker started for {}", meta.id);

		while let Some(first_event) = rx.recv().await {
			// Start batching window
			let mut batch = vec![first_event];
			let deadline = Instant::now() + Duration::from_millis(config.debounce_window_ms);

			// Collect events within the debounce window
			while Instant::now() < deadline && batch.len() < config.max_batch_size {
				match rx.try_recv() {
					Ok(event) => batch.push(event),
					Err(mpsc::error::TryRecvError::Empty) => {
						// Brief sleep to avoid busy waiting
						tokio::time::sleep(Duration::from_millis(10)).await;
					}
					Err(mpsc::error::TryRecvError::Disconnected) => break,
				}
			}

			debug!(
				"Processing batch of {} events for location {}",
				batch.len(),
				meta.id
			);

			// Pass FsEvent batch directly to responder
			if let Err(e) = responder::apply_batch(
				&context,
				meta.library_id,
				meta.id,
				batch,
				meta.rule_toggles,
				&meta.root_path,
				None, // volume_backend - TODO: resolve from context
			)
			.await
			{
				error!("Failed to apply batch for location {}: {}", meta.id, e);
			}
		}

		info!("Location worker stopped for {}", meta.id);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_default() {
		let config = PersistentHandlerConfig::default();
		assert_eq!(config.debounce_window_ms, 150);
		assert_eq!(config.max_batch_size, 10000);
	}
}
