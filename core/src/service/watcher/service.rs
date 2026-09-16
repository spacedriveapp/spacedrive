//! FsWatcher Service - wraps the sd-fs-watcher crate for use in Spacedrive
//!
//! This service manages the lifecycle of the filesystem watcher and provides
//! the event stream that handlers subscribe to. It owns and starts the
//! `EphemeralEventHandler`.

use crate::context::CoreContext;
use crate::library::Library;
use crate::ops::indexing::handlers::EphemeralEventHandler;
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
/// - Owns and starts the event handler (EphemeralEventHandler)
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
			ephemeral_handler: EphemeralEventHandler::new_unconnected(context),
			is_running: AtomicBool::new(false),
			config,
		}
	}

	/// Initialize handlers with a reference to self (wrapped in Arc)
	///
	/// Must be called after the service is wrapped in Arc.
	pub async fn init_handlers(self: &Arc<Self>) {
		self.ephemeral_handler.connect(self.clone()).await;
		// Subscription precedes the restore pass: arm_restored_sources
		// installs the announcement channel synchronously, so no restore
		// announced below can be missed.
		self.clone().arm_restored_sources();
		self.clone().restore_registered_sources();
	}

	/// Watch every source whose index arrives from a snapshot.
	///
	/// An index becomes browsable two ways and only one of them armed a watch.
	/// A walk finishes and the indexing job watches what it walked; a restart
	/// rebuilds the same index from a snapshot and watched nothing, so a drive
	/// that browsed perfectly reported no changes until it was indexed again.
	fn arm_restored_sources(self: Arc<Self>) {
		let mut restored = self.context.ephemeral_cache().subscribe_restored_roots();

		tokio::spawn(async move {
			while let Some(root) = restored.recv().await {
				match self.watch_ephemeral(root.clone()).await {
					Ok(()) => info!("Watching restored source: {}", root.display()),
					Err(e) => {
						warn!("Failed to watch restored source {}: {}", root.display(), e);
						// A restore announces its volume root, and one
						// unreadable directory anywhere under it fails the
						// whole recursive watch. The registered sources on
						// that volume are narrower and still watchable; a
						// nested source must not lose its watch to a sibling
						// it does not contain.
						for source in self.context.ephemeral_cache().sources() {
							if !source.attached
								|| source.root == root || !source.root.starts_with(&root)
							{
								continue;
							}
							match self.watch_ephemeral(source.root.clone()).await {
								Ok(()) => {
									info!("Watching restored source: {}", source.root.display())
								}
								Err(e) => warn!(
									"Failed to watch restored source {}: {}",
									source.root.display(),
									e
								),
							}
						}
					}
				}
			}
		});
	}

	/// Restore every attached registered source's arena at startup.
	///
	/// A restore announces its root, and the announcement is what arms the
	/// source's watch. Nothing else drives a restore on a daemon nobody is
	/// reading from: a peer's snapshot fetch used to, incidentally, until
	/// generation convergence removed that churn — which left registered
	/// sources unwatched until the first local read, with live changes
	/// falling into unwatched trees in the meantime. Detached sources stay
	/// lazy; there is no filesystem to watch until they return.
	fn restore_registered_sources(self: Arc<Self>) {
		tokio::spawn(async move {
			let cache = self.context.ephemeral_cache();
			let mut restored = 0usize;
			for source in cache.sources() {
				if !source.attached {
					continue;
				}
				if cache.ensure_restored(&source.root).await {
					restored += 1;
				}
			}
			if restored > 0 {
				info!("Restored {restored} registered source arena(s) at startup");
			}
		});
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

	/// Watch an ephemeral path (shallow, in-memory only)
	///
	/// Used for browsing external drives, network shares, etc.
	/// Registers with ephemeral cache and starts OS-level watching.
	pub async fn watch_ephemeral(&self, path: impl Into<PathBuf>) -> Result<()> {
		let path = path.into();
		debug!("Watching ephemeral path: {}", path.display());

		if self.context.ephemeral_cache().is_watched(&path) {
			return Ok(());
		}

		// Register with ephemeral cache so handler knows to process events.
		// Without this the OS watch still fires and `EphemeralEventHandler`
		// drops every event as unmatched, which looks exactly like a watcher
		// that is running and a UI that never updates.
		if !self
			.context
			.ephemeral_cache()
			.register_for_watching(path.clone())
		{
			return Err(anyhow::anyhow!(
				"cannot watch {}: not indexed, or its source is detached",
				path.display()
			));
		}

		// Recursive, because the index under this root is. A source is walked
		// all the way down and the watch has to cover what the walk covered;
		// depth is then decided per event by whether the index holds the
		// parent. macOS has no non-recursive subscription anyway, so a shallow
		// config here was only ever honoured on other platforms, where it made
		// every change below the first level invisible.
		self.watcher
			.watch_path(&path, WatchConfig::recursive())
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

		self.ephemeral_handler.start().await?;

		info!("FsWatcher service started");

		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.is_running.swap(false, Ordering::SeqCst) {
			return Ok(());
		}

		info!("Stopping FsWatcher service");

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
