//! FsWatcher Service - wraps the sd-fs-watcher crate for use in Spacedrive
//!
//! This service manages the lifecycle of the filesystem watcher and provides
//! the event stream that handlers subscribe to. It owns and starts the
//! `EphemeralEventHandler` and `PersistentEventHandler`.

use crate::context::CoreContext;
use crate::library::Library;
use crate::ops::indexing::handlers::{EphemeralEventHandler, LocationMeta, PersistentEventHandler};
use crate::ops::indexing::rules::RuleToggles;
use crate::service::Service;
use anyhow::Result;
use sd_fs_watcher::{FsEvent, FsWatcher, WatchConfig, WatcherConfig};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Configuration for the FsWatcher service
#[derive(Debug, Clone)]
pub struct FsWatcherServiceConfig {
	/// Size of the internal event buffer
	pub event_buffer_size: usize,
	/// Tick interval for platform-specific event eviction
	pub tick_interval: Duration,
	/// Enable debug logging
	pub debug_mode: bool,
}

impl Default for FsWatcherServiceConfig {
	fn default() -> Self {
		Self {
			event_buffer_size: 100_000,
			tick_interval: Duration::from_millis(100),
			debug_mode: false,
		}
	}
}

impl From<FsWatcherServiceConfig> for WatcherConfig {
	fn from(config: FsWatcherServiceConfig) -> Self {
		WatcherConfig::default()
			.with_buffer_size(config.event_buffer_size)
			.with_tick_interval(config.tick_interval)
			.with_debug(config.debug_mode)
	}
}

/// Filesystem watcher service that wraps sd-fs-watcher
///
/// This service:
/// - Manages the lifecycle of the underlying FsWatcher
/// - Owns and starts the event handlers (PersistentEventHandler, EphemeralEventHandler)
/// - Handles watch registration for paths
///
/// ## Usage
///
/// ```ignore
/// let config = FsWatcherServiceConfig::default();
/// let service = FsWatcherService::new(context, config);
///
/// // Start the service (also starts handlers)
/// service.start().await?;
///
/// // Watch a location (persistent, recursive)
/// service.watch_location(LocationMeta { ... }).await?;
///
/// // Watch an ephemeral path (shallow, in-memory)
/// service.watch_ephemeral("/path/to/browse").await?;
/// ```
pub struct FsWatcherService {
	/// Core context for ephemeral cache access
	context: Arc<CoreContext>,
	/// The underlying filesystem watcher
	watcher: FsWatcher,
	/// Handler for persistent (database) events
	persistent_handler: PersistentEventHandler,
	/// Handler for ephemeral (in-memory) events
	ephemeral_handler: EphemeralEventHandler,
	/// Whether the service is running
	is_running: AtomicBool,
	/// Configuration
	config: FsWatcherServiceConfig,
}

impl FsWatcherService {
	/// Create a new FsWatcher service
	///
	/// Note: Handlers are created but not yet connected. Call `init_handlers()`
	/// after wrapping in Arc to connect them to the watcher.
	pub fn new(context: Arc<CoreContext>, config: FsWatcherServiceConfig) -> Self {
		let watcher_config: WatcherConfig = config.clone().into();
		let watcher = FsWatcher::new(watcher_config);

		Self {
			context: context.clone(),
			watcher,
			persistent_handler: PersistentEventHandler::new_unconnected(context.clone()),
			ephemeral_handler: EphemeralEventHandler::new_unconnected(context),
			is_running: AtomicBool::new(false),
			config,
		}
	}

	/// Initialize handlers with a reference to self (wrapped in Arc)
	///
	/// Must be called after the service is wrapped in Arc.
	pub async fn init_handlers(self: &Arc<Self>) {
		self.persistent_handler.connect(self.clone()).await;
		self.ephemeral_handler.connect(self.clone()).await;
	}

	/// Subscribe to filesystem events
	///
	/// Returns a broadcast receiver that will receive all filesystem events.
	/// Multiple subscribers can exist simultaneously.
	pub fn subscribe(&self) -> broadcast::Receiver<FsEvent> {
		self.watcher.subscribe()
	}

	/// Watch a path with the given configuration
	///
	/// For persistent locations, use `WatchConfig::recursive()`.
	/// For ephemeral browsing, use `WatchConfig::shallow()`.
	pub async fn watch_path(&self, path: impl Into<PathBuf>, config: WatchConfig) -> Result<()> {
		let path = path.into();
		debug!("Watching path: {}", path.display());
		self.watcher.watch_path(&path, config).await?;
		Ok(())
	}

	/// Stop watching a path
	pub async fn unwatch_path(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
		let path = path.as_ref();
		debug!("Unwatching path: {}", path.display());
		self.watcher.unwatch(path).await?;
		Ok(())
	}

	/// Get all currently watched paths
	pub async fn watched_paths(&self) -> Vec<PathBuf> {
		self.watcher.watched_paths().await
	}

	/// Get the number of events received from the OS
	pub fn events_received(&self) -> u64 {
		self.watcher.events_received()
	}

	/// Get the number of events emitted to subscribers
	pub fn events_emitted(&self) -> u64 {
		self.watcher.events_emitted()
	}

	/// Get a reference to the underlying watcher
	///
	/// Use this for advanced operations or when you need direct access
	/// to the watcher's capabilities.
	pub fn inner(&self) -> &FsWatcher {
		&self.watcher
	}

	/// Watch a location (persistent, recursive)
	///
	/// The location will be watched recursively and events will be
	/// batched and persisted to the database.
	pub async fn watch_location(&self, meta: LocationMeta) -> Result<()> {
		info!(
			"Watching location {} at {}",
			meta.id,
			meta.root_path.display()
		);
		self.persistent_handler.add_location(meta).await
	}

	/// Stop watching a location
	pub async fn unwatch_location(&self, location_id: uuid::Uuid) -> Result<()> {
		info!("Unwatching location {}", location_id);
		self.persistent_handler.remove_location(location_id).await
	}

	/// Get all watched locations
	pub async fn watched_locations(&self) -> Vec<LocationMeta> {
		self.persistent_handler.locations().await
	}

	/// Load and watch all eligible locations from a library
	///
	/// Only watches locations that:
	/// - Are on this device
	/// - Have IndexMode != None
	pub async fn load_library_locations(&self, library: &Library) -> Result<usize> {
		use crate::infra::db::entities::{device, location};
		use crate::ops::indexing::path_resolver::PathResolver;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let db = library.db().conn();
		let mut count = 0;

		// Get current device UUID and find in this library's database
		let current_device_uuid = crate::device::get_current_device_id();
		let current_device = device::Entity::find()
			.filter(device::Column::Uuid.eq(current_device_uuid))
			.one(db)
			.await?;

		let Some(current_device) = current_device else {
			warn!(
				"Current device {} not found in library {} database",
				current_device_uuid,
				library.id()
			);
			return Ok(0);
		};

		// Query locations owned by this device
		let locations = location::Entity::find()
			.filter(location::Column::DeviceId.eq(current_device.id))
			.all(db)
			.await?;

		debug!(
			"Found {} locations in library {} for this device",
			locations.len(),
			library.id()
		);

		for loc in locations {
			// Skip locations without entry_id (not yet indexed)
			let Some(entry_id) = loc.entry_id else {
				debug!("Skipping location {} - no entry_id", loc.uuid);
				continue;
			};

			// Skip IndexMode::None
			if loc.index_mode == "none" {
				debug!("Skipping location {} - IndexMode::None", loc.uuid);
				continue;
			}

			// Get the full filesystem path
			let path = match PathResolver::get_full_path(db, entry_id).await {
				Ok(path) => path,
				Err(e) => {
					warn!("Failed to resolve path for location {}: {}", loc.uuid, e);
					continue;
				}
			};

			// Skip cloud locations
			let path_str = path.to_string_lossy();
			if path_str.contains("://") && !path_str.starts_with("local://") {
				debug!("Skipping cloud location {}: {}", loc.uuid, path_str);
				continue;
			}

			// Check if path exists
			if !path.exists() {
				warn!(
					"Location {} path does not exist: {}",
					loc.uuid,
					path.display()
				);
				continue;
			}

			let meta = LocationMeta {
				id: loc.uuid,
				library_id: library.id(),
				root_path: path,
				rule_toggles: RuleToggles::default(),
			};

			if let Err(e) = self.watch_location(meta).await {
				warn!("Failed to watch location {}: {}", loc.uuid, e);
			} else {
				count += 1;
			}
		}

		info!("Loaded {} locations from library {}", count, library.id());
		Ok(count)
	}

	/// Watch an ephemeral path (shallow, in-memory only)
	///
	/// Used for browsing external drives, network shares, etc.
	/// Registers with ephemeral cache and starts OS-level watching.
	pub async fn watch_ephemeral(&self, path: impl Into<PathBuf>) -> Result<()> {
		let path = path.into();
		debug!("Watching ephemeral path: {}", path.display());

		// Register with ephemeral cache so handler knows to process events
		self.context
			.ephemeral_cache()
			.register_for_watching(path.clone());

		// Start OS-level watching (shallow = immediate children only)
		self.watcher
			.watch_path(&path, WatchConfig::shallow())
			.await?;

		Ok(())
	}

	/// Stop watching an ephemeral path
	pub async fn unwatch_ephemeral(&self, path: &Path) -> Result<()> {
		debug!("Unwatching ephemeral path: {}", path.display());

		// Unregister from ephemeral cache
		self.context
			.ephemeral_cache()
			.unregister_from_watching(path);

		// Stop OS-level watching
		self.watcher.unwatch(path).await?;

		Ok(())
	}

	// ==================== Handler Access ====================

	/// Get reference to persistent handler
	pub fn persistent_handler(&self) -> &PersistentEventHandler {
		&self.persistent_handler
	}

	/// Get reference to ephemeral handler
	pub fn ephemeral_handler(&self) -> &EphemeralEventHandler {
		&self.ephemeral_handler
	}
}

#[async_trait::async_trait]
impl Service for FsWatcherService {
	async fn start(&self) -> Result<()> {
		if self.is_running.swap(true, Ordering::SeqCst) {
			warn!("FsWatcher service is already running");
			return Ok(());
		}

		info!("Starting FsWatcher service");

		// Start the underlying watcher first
		self.watcher.start().await?;

		// Start the event handlers
		self.persistent_handler.start().await?;
		self.ephemeral_handler.start().await?;

		info!("FsWatcher service started (with handlers)");

		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.is_running.swap(false, Ordering::SeqCst) {
			return Ok(());
		}

		info!("Stopping FsWatcher service");

		// Stop handlers first
		self.persistent_handler.stop().await;
		self.ephemeral_handler.stop();

		// Then stop the watcher
		self.watcher.stop().await?;

		info!("FsWatcher service stopped");

		Ok(())
	}

	fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	fn name(&self) -> &'static str {
		"fs_watcher"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_default() {
		let config = FsWatcherServiceConfig::default();
		assert_eq!(config.event_buffer_size, 100_000);
		assert!(!config.debug_mode);
	}

	// Note: Full service tests require CoreContext which needs async runtime
	// See integration tests for complete service lifecycle testing
}

