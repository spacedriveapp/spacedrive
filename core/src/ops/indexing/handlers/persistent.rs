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
			self.ensure_worker(meta.clone()).await?;
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
		use sd_fs_watcher::FsEventKind;
		use std::collections::HashMap;

		info!("Location worker started for {}", meta.id);

		// Buffer for pending removes - maps inode to (path, timestamp, is_directory)
		// These are Remove events that might be part of a rename operation.
		let mut pending_removes: HashMap<u64, (PathBuf, Instant, Option<bool>)> = HashMap::new();
		const RENAME_TIMEOUT: Duration = Duration::from_millis(1000);

		// Create a periodic tick for evicting expired pending removes
		// This ensures removes are processed even if no new events arrive
		let mut eviction_tick = tokio::time::interval(Duration::from_millis(500));
		eviction_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

		loop {
			// Wait for either an event or an eviction tick
			let batch = tokio::select! {
				// Process incoming events
				Some(first_event) = rx.recv() => {
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

					batch
				}
				// Periodic eviction tick
				_ = eviction_tick.tick() => {
					Vec::new() // Empty batch - just check for evictions
				}
			};

			// Check if worker should stop (channel disconnected and no pending removes)
			if batch.is_empty() && pending_removes.is_empty() {
				if rx.is_closed() {
					break;
				}
				continue;
			}

			// Evict expired pending removes
			let now = Instant::now();
			let expired: Vec<u64> = pending_removes
				.iter()
				.filter(|(_, (_, ts, _))| now.duration_since(*ts) > RENAME_TIMEOUT)
				.map(|(inode, _)| *inode)
				.collect();

			// Process expired removes as actual deletions
			let mut expired_events = Vec::new();
			for inode in expired {
				if let Some((path, _, is_dir)) = pending_removes.remove(&inode) {
					debug!(
						"Evicting expired pending remove: {} (inode {})",
						path.display(),
						inode
					);
					expired_events.push(FsEvent::remove(path));
				}
			}

			// Detect renames by matching Remove+Create pairs using database inodes.
			// On macOS, renames arrive as separate Remove and Create events across batches.
			let batch = Self::detect_renames_from_db(
				&context,
				meta.library_id,
				batch,
				&mut pending_removes,
			)
			.await;

			// Combine expired removes with processed batch
			let mut final_batch = expired_events;
			final_batch.extend(batch);

			if final_batch.is_empty() {
				continue;
			}

			// Pass FsEvent batch directly to responder
			if let Err(e) = responder::apply_batch(
				&context,
				meta.library_id,
				meta.id,
				final_batch,
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

	/// Detect renames by matching Remove+Create pairs using database inodes.
	///
	/// Remove events with inodes are buffered in `pending_removes`. Create events
	/// check against this buffer to detect renames. This handles the case where
	/// Remove and Create arrive in separate batches (common on macOS FSEvents).
	async fn detect_renames_from_db(
		context: &Arc<CoreContext>,
		library_id: Uuid,
		events: Vec<FsEvent>,
		pending_removes: &mut std::collections::HashMap<u64, (PathBuf, Instant, Option<bool>)>,
	) -> Vec<FsEvent> {
		use sd_fs_watcher::FsEventKind;

		let Some(library) = context.get_library(library_id).await else {
			return events;
		};
		let db = library.db().conn();

		let mut result: Vec<FsEvent> = Vec::new();

		for event in events {
			match &event.kind {
				FsEventKind::Remove => {
					// Query database for inode
					let inode = Self::get_inode_from_db(db, &event.path).await;
					if let Some(inode) = inode {
						debug!(
							"Buffering Remove event: {} with inode {} for potential rename",
							event.path.display(),
							inode
						);
						// Buffer for potential rename detection
						pending_removes
							.insert(inode, (event.path, Instant::now(), event.is_directory));
					} else {
						// No inode in DB, emit as regular Remove
						result.push(event);
					}
				}
				FsEventKind::Create => {
					// Get inode from filesystem
					let inode = Self::get_inode_from_fs(&event.path).await;

					if let Some(inode) = inode {
						// Check if this matches a pending remove
						if let Some((old_path, _, is_dir)) = pending_removes.remove(&inode) {
							// Found a match - emit Rename
							info!(
								"Detected rename via database inode {}: {} -> {}",
								inode,
								old_path.display(),
								event.path.display()
							);
							let rename_event = if let Some(is_dir) = is_dir.or(event.is_directory) {
								FsEvent::rename_with_dir_flag(old_path, event.path, is_dir)
							} else {
								FsEvent::rename(old_path, event.path)
							};
							result.push(rename_event);
							continue;
						}
					}
					// No matching pending remove, emit as regular Create
					result.push(event);
				}
				_ => {
					result.push(event);
				}
			}
		}

		result
	}

	/// Get the inode for a path from the database.
	async fn get_inode_from_db(
		db: &sea_orm::DatabaseConnection,
		path: &std::path::Path,
	) -> Option<u64> {
		use crate::infra::db::entities::{directory_paths, entry};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// First try as directory (lookup via directory_paths)
		let path_str = path.to_string_lossy().to_string();
		if let Ok(Some(dir_record)) = directory_paths::Entity::find()
			.filter(directory_paths::Column::Path.eq(&path_str))
			.one(db)
			.await
		{
			if let Ok(Some(entry_record)) =
				entry::Entity::find_by_id(dir_record.entry_id).one(db).await
			{
				if let Some(inode) = entry_record.inode {
					return Some(inode as u64);
				}
			}
		}

		// Try as file (lookup via parent directory + name)
		let parent = path.parent()?;
		let name = path.file_stem()?.to_str()?;
		let ext = path.extension().and_then(|e| e.to_str());

		let parent_str = parent.to_string_lossy().to_string();
		let parent_dir = directory_paths::Entity::find()
			.filter(directory_paths::Column::Path.eq(&parent_str))
			.one(db)
			.await
			.ok()??;

		let mut query = entry::Entity::find()
			.filter(entry::Column::ParentId.eq(parent_dir.entry_id))
			.filter(entry::Column::Name.eq(name));

		if let Some(e) = ext {
			query = query.filter(entry::Column::Extension.eq(e.to_lowercase()));
		} else {
			query = query.filter(entry::Column::Extension.is_null());
		}

		let entry_record = query.one(db).await.ok()??;
		entry_record.inode.map(|i| i as u64)
	}

	/// Get the inode for a path from the filesystem.
	async fn get_inode_from_fs(path: &std::path::Path) -> Option<u64> {
		#[cfg(unix)]
		{
			use std::os::unix::fs::MetadataExt;
			tokio::fs::metadata(path).await.ok().map(|m| m.ino())
		}
		#[cfg(not(unix))]
		{
			None
		}
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
