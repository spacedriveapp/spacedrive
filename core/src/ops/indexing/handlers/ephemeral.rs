//! Ephemeral event handler
//!
//! Subscribes to filesystem events and routes them to the ephemeral responder
//! for in-memory index updates. Used for browsing external drives, network shares,
//! and other non-persistent locations.
//!
//! ## Characteristics
//!
//! - **Shallow watching**: Only processes events for immediate children of watched directories
//! - **No batching**: Memory writes are fast, events processed immediately
//! - **Session-based**: Events only processed for active browsing sessions

use crate::context::CoreContext;
use crate::ops::indexing::ephemeral::responder;
use crate::ops::indexing::rules::RuleToggles;
use crate::service::watcher::FsWatcherService;
use anyhow::Result;
use sd_fs_watcher::FsEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, trace, warn};

/// Handler for ephemeral (in-memory) filesystem events
///
/// Subscribes to `FsWatcher` events and routes matching events to the
/// ephemeral responder for immediate in-memory updates.
pub struct EphemeralEventHandler {
	/// Core context (contains ephemeral_cache)
	context: Arc<CoreContext>,
	/// Reference to the filesystem watcher service (set via connect())
	fs_watcher: RwLock<Option<Arc<FsWatcherService>>>,
	/// Whether the handler is running
	is_running: Arc<AtomicBool>,
	/// Default rule toggles for filtering
	rule_toggles: RuleToggles,
}

impl EphemeralEventHandler {
	/// Create a new ephemeral event handler (unconnected)
	///
	/// Call `connect()` to attach to a FsWatcherService before starting.
	pub fn new_unconnected(context: Arc<CoreContext>) -> Self {
		Self {
			context,
			fs_watcher: RwLock::new(None),
			is_running: Arc::new(AtomicBool::new(false)),
			rule_toggles: RuleToggles::default(),
		}
	}

	/// Create a new ephemeral event handler (connected)
	pub fn new(context: Arc<CoreContext>, fs_watcher: Arc<FsWatcherService>) -> Self {
		Self {
			context,
			fs_watcher: RwLock::new(Some(fs_watcher)),
			is_running: Arc::new(AtomicBool::new(false)),
			rule_toggles: RuleToggles::default(),
		}
	}

	/// Connect to a FsWatcherService
	pub fn connect(&self, fs_watcher: Arc<FsWatcherService>) {
		// Use blocking_write since this is called during init, not async context
		*self.fs_watcher.blocking_write() = Some(fs_watcher);
	}

	/// Start the event handler
	///
	/// Spawns a task that subscribes to filesystem events and routes
	/// matching events to the ephemeral responder.
	pub async fn start(&self) -> Result<()> {
		if self.is_running.swap(true, Ordering::SeqCst) {
			warn!("EphemeralEventHandler is already running");
			return Ok(());
		}

		let fs_watcher = self.fs_watcher.read().await.clone();
		let Some(fs_watcher) = fs_watcher else {
			return Err(anyhow::anyhow!(
				"EphemeralEventHandler not connected to FsWatcherService"
			));
		};

		debug!("Starting EphemeralEventHandler");

		let mut rx = fs_watcher.subscribe();
		let context = self.context.clone();
		let rule_toggles = self.rule_toggles;
		let is_running = self.is_running.clone();

		tokio::spawn(async move {
			debug!("EphemeralEventHandler task started");

			while is_running.load(Ordering::SeqCst) {
				match rx.recv().await {
					Ok(event) => {
						if let Err(e) = Self::handle_event(&context, &event, rule_toggles).await {
							error!("Error handling ephemeral event: {}", e);
						}
					}
					Err(broadcast::error::RecvError::Lagged(n)) => {
						warn!("EphemeralEventHandler lagged by {} events", n);
						// Continue processing - we'll catch up
					}
					Err(broadcast::error::RecvError::Closed) => {
						debug!("FsWatcher channel closed, stopping EphemeralEventHandler");
						break;
					}
				}
			}

			debug!("EphemeralEventHandler task stopped");
		});

		Ok(())
	}

	/// Stop the event handler
	pub fn stop(&self) {
		debug!("Stopping EphemeralEventHandler");
		self.is_running.store(false, Ordering::SeqCst);
	}

	/// Check if the handler is running
	pub fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	/// Handle a single filesystem event
	///
	/// Checks if the event's path is under an ephemeral watched directory.
	/// For shallow watches, only processes events for immediate children.
	async fn handle_event(
		context: &Arc<CoreContext>,
		event: &FsEvent,
		rule_toggles: RuleToggles,
	) -> Result<()> {
		// Get the parent directory of the event path
		let Some(parent) = event.path.parent() else {
			trace!("Event path has no parent: {}", event.path.display());
			return Ok(());
		};

		// Check if the parent is being watched (shallow watch = immediate children only)
		let watched_paths = context.ephemeral_cache().watched_paths();

		// Find if any watched path matches the parent
		let matching_root = watched_paths.iter().find(|watched| {
			// For shallow watches, parent must exactly match the watched path
			parent == watched.as_path()
		});

		let Some(root_path) = matching_root else {
			// Not under any ephemeral watch
			trace!("Event not under ephemeral watch: {}", event.path.display());
			return Ok(());
		};

		debug!(
			"Ephemeral event matched: {} (root: {})",
			event.path.display(),
			root_path.display()
		);

		// Pass FsEvent directly to responder
		responder::apply(context, root_path, event.clone(), rule_toggles).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Integration tests would require full context setup
	// The handler logic is straightforward - subscribe, filter, route
}
