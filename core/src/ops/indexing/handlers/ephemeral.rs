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

/// How long changes under summarised directories collect before those
/// directories are recounted.
const RECOUNT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

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
	pub async fn connect(&self, fs_watcher: Arc<FsWatcherService>) {
		*self.fs_watcher.write().await = Some(fs_watcher);
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

		// Summarised directories keep no tree to update, so a change under one
		// marks it for recounting instead. Doing that on a timer is what keeps
		// a file being written in a loop from recounting its whole subtree once
		// per write.
		{
			let context = context.clone();
			let is_running = is_running.clone();
			tokio::spawn(async move {
				while is_running.load(Ordering::SeqCst) {
					tokio::time::sleep(RECOUNT_INTERVAL).await;
					let recounted = responder::recount_summaries(&context).await;
					if recounted > 0 {
						debug!("Recounted {recounted} summarised directories");
					}
				}
			});
		}

		// Roots the watcher has changed files under since the last hashing
		// nudge. Drained on a timer: the content job claims its work from the
		// store and dispatch dedupes on the root, so nudging is idempotent
		// and the cost of a nudge with nothing to do is one empty query.
		let dirty_roots: Arc<std::sync::Mutex<std::collections::HashSet<std::path::PathBuf>>> =
			Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
		{
			let dirty_roots = dirty_roots.clone();
			let context = context.clone();
			let is_running = is_running.clone();
			tokio::spawn(async move {
				const HASH_NUDGE_INTERVAL: std::time::Duration =
					std::time::Duration::from_secs(30);
				while is_running.load(Ordering::SeqCst) {
					tokio::time::sleep(HASH_NUDGE_INTERVAL).await;
					let roots: Vec<std::path::PathBuf> =
						dirty_roots.lock().unwrap().drain().collect();
					if roots.is_empty() {
						continue;
					}
					let libraries = context.libraries().await.get_open_libraries().await;
					let Some(library) = libraries.first() else {
						continue;
					};
					for root in roots {
						let job = crate::ops::indexing::content_identity::ContentIdentityJob::new(
							root.clone(),
						);
						if let Err(e) = library
							.jobs()
							.dispatch_with_priority(
								job,
								crate::infra::job::types::JobPriority::LOW,
								None,
							)
							.await
						{
							warn!(root = %root.display(), "could not nudge hashing: {e}");
						}
					}
				}
			});
		}

		let dirty_for_events = dirty_roots.clone();
		tokio::spawn(async move {
			debug!("EphemeralEventHandler task started");

			while is_running.load(Ordering::SeqCst) {
				match rx.recv().await {
					Ok(event) => {
						if let Err(e) = Self::handle_event(&context, &event, rule_toggles).await {
							error!("Error handling ephemeral event: {}", e);
						} else if let Some(root) =
							context.ephemeral_cache().source_root_for(&event.path)
						{
							dirty_for_events.lock().unwrap().insert(root);
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
	async fn handle_event(
		context: &Arc<CoreContext>,
		event: &FsEvent,
		rule_toggles: RuleToggles,
	) -> Result<()> {
		let Some(root_path) = context
			.ephemeral_cache()
			.watched_root_for_change(&event.path)
			.await
		else {
			trace!("Event not under ephemeral watch: {}", event.path.display());
			return Ok(());
		};
		let root_path = &root_path;

		debug!(
			"Ephemeral event matched: {} (root: {})",
			event.path.display(),
			root_path.display()
		);

		// A vanished watched root means the volume unmounted; events from the
		// flood that follows must not mutate the in-memory index.
		if !super::root_is_present(root_path) {
			warn!(
				"Watched root {} missing, dropping {:?} event for {} — volume likely unmounted",
				root_path.display(),
				event.kind,
				event.path.display()
			);
			return Ok(());
		}

		// The source's own capture policy, not the handler's default: an
		// archival source keeps everything its walk keeps, live changes
		// included.
		let rule_toggles = context.ephemeral_cache().rule_toggles_for(root_path);
		responder::apply(context, root_path, event.clone(), rule_toggles).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Integration tests would require full context setup
	// The handler logic is straightforward - subscribe, filter, route
}
