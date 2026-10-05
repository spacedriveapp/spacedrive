//! Main filesystem watcher implementation
//!
//! `FsWatcher` is the primary interface for watching filesystem changes.
//! It's storage-agnostic - it only knows about paths and events, not
//! about sources, libraries, or databases.

use crate::config::{WatchConfig, WatcherConfig};
use crate::error::{Result, WatcherError};
use crate::event::{FsEvent, RawNotifyEvent};
use crate::platform::PlatformHandler;
use crate::spelling::{self, Subscription};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
/// The mode to hand notify for a watch.
///
/// macOS is the exception and it fails silently. FSEvents has no non-recursive
/// subscription, and notify's backend does not emulate one: a watch registered
/// `NonRecursive` there is accepted, reports no error, and then delivers
/// nothing at all. Measured, not inferred — the same directory watched
/// `Recursive` delivers immediately.
///
/// So macOS always subscribes recursively and the shallow contract is kept
/// above, by callers that already compare an event's parent against the root
/// they asked for. Watching wider than asked costs events that get filtered;
/// watching non-recursively costs every event.
fn notify_mode(recursive: bool) -> RecursiveMode {
	if recursive || cfg!(target_os = "macos") {
		RecursiveMode::Recursive
	} else {
		RecursiveMode::NonRecursive
	}
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, trace, warn};

/// Handle returned when watching a path
///
/// When dropped, the path is automatically unwatched (if no other handles exist).
pub struct WatchHandle {
	path: PathBuf,
	watcher: Arc<FsWatcherInner>,
}

impl std::fmt::Debug for WatchHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WatchHandle")
			.field("path", &self.path)
			.finish()
	}
}

impl Drop for WatchHandle {
	fn drop(&mut self) {
		// Decrement reference count and unwatch if zero
		let path = self.path.clone();
		let inner = self.watcher.clone();

		// Spawn a task to handle the async unwatch
		tokio::spawn(async move {
			if let Err(e) = inner.release_watch(&path).await {
				warn!("Failed to release watch for {}: {}", path.display(), e);
			}
		});
	}
}

/// Watch state for a path
struct WatchState {
	config: WatchConfig,
	ref_count: usize,
	/// The platform subscriptions backing this watch. More than one when the
	/// root spans several firmlinks; see [`crate::spelling`].
	subscriptions: Vec<Subscription>,
}

/// Internal watcher state
struct FsWatcherInner {
	/// Configuration
	config: WatcherConfig,
	/// Watched paths with reference counting
	watched_paths: RwLock<HashMap<PathBuf, WatchState>>,
	/// The notify watcher instance
	notify_watcher: RwLock<Option<RecommendedWatcher>>,
	/// Platform-specific event handler
	platform_handler: PlatformHandler,
	/// Whether the watcher is running
	is_running: AtomicBool,
	/// Event sender for broadcasts
	event_tx: broadcast::Sender<FsEvent>,
	/// Metrics
	events_received: AtomicU64,
	events_emitted: AtomicU64,
}

impl FsWatcherInner {
	/// Add a watch with reference counting
	async fn add_watch(&self, path: PathBuf, config: WatchConfig) -> Result<()> {
		let mut watched = self.watched_paths.write().await;

		if let Some(state) = watched.get_mut(&path) {
			// Path already watched - increment ref count
			state.ref_count += 1;
			debug!(
				"Incremented ref count for {}: {}",
				path.display(),
				state.ref_count
			);
			return Ok(());
		}

		// Validate path exists
		if !path.exists() {
			return Err(WatcherError::PathNotFound(path));
		}

		let subscriptions = spelling::subscriptions(&path);

		// Register with notify if we're running
		if self.is_running.load(Ordering::SeqCst) {
			if let Some(watcher) = self.notify_watcher.write().await.as_mut() {
				let mode = notify_mode(config.recursive);

				for subscription in &subscriptions {
					watcher.watch(&subscription.subscribe, mode).map_err(|e| {
						WatcherError::WatchFailed {
							path: path.clone(),
							reason: e.to_string(),
						}
					})?;
				}
			}
		}

		watched.insert(
			path.clone(),
			WatchState {
				config,
				ref_count: 1,
				subscriptions,
			},
		);

		debug!("Started watching: {}", path.display());
		Ok(())
	}

	/// Rewrite an event's paths from the spelling the platform delivered into
	/// the spelling its watch is stored under.
	///
	/// Subscriptions and events are two halves of one translation: a watch
	/// registered under a firmlink's short spelling reports events under it
	/// too, and everything above the watcher works in the long one. Without
	/// this the paths never match a watched root, and an index keyed one way
	/// grows a second tree keyed the other.
	async fn restore_spelling(&self, event: &mut RawNotifyEvent) {
		let watched = self.watched_paths.read().await;

		let translations: Vec<&Subscription> = watched
			.values()
			.flat_map(|state| state.subscriptions.iter())
			.filter(|subscription| subscription.subscribe != subscription.restore_to)
			.collect();

		if translations.is_empty() {
			return;
		}

		for path in &mut event.paths {
			if let Some(restored) = translations
				.iter()
				.find_map(|subscription| spelling::restore(subscription, path))
			{
				*path = restored;
			}
		}
	}

	/// Release a watch (decrement ref count, unwatch if zero)
	async fn release_watch(&self, path: &Path) -> Result<()> {
		let mut watched = self.watched_paths.write().await;

		let subscriptions;
		let should_unwatch = if let Some(state) = watched.get_mut(path) {
			state.ref_count -= 1;
			subscriptions = state.subscriptions.clone();
			debug!(
				"Decremented ref count for {}: {}",
				path.display(),
				state.ref_count
			);
			state.ref_count == 0
		} else {
			return Ok(()); // Not watched
		};

		if should_unwatch {
			watched.remove(path);

			// Unregister from notify if we're running
			if self.is_running.load(Ordering::SeqCst) {
				if let Some(watcher) = self.notify_watcher.write().await.as_mut() {
					for subscription in &subscriptions {
						if let Err(e) = watcher.unwatch(&subscription.subscribe) {
							warn!(
								"Failed to unwatch {}: {}",
								subscription.subscribe.display(),
								e
							);
						}
					}
				}
			}

			debug!("Stopped watching: {}", path.display());
		}

		Ok(())
	}
}

/// Platform-agnostic filesystem watcher
///
/// Watches filesystem paths and emits normalized events. Handles platform-specific
/// quirks like macOS rename detection internally.
///
/// # Example
///
/// ```ignore
/// use sd_fs_watcher::{FsWatcher, WatchConfig};
///
/// let watcher = FsWatcher::new(Default::default());
/// watcher.start().await?;
///
/// // Subscribe to events
/// let mut rx = watcher.subscribe();
///
/// // Watch a path
/// let handle = watcher.watch("/path/to/watch", WatchConfig::recursive()).await?;
///
/// // Receive events
/// while let Ok(event) = rx.recv().await {
///     println!("Event: {:?}", event);
/// }
/// ```
pub struct FsWatcher {
	inner: Arc<FsWatcherInner>,
}

impl FsWatcher {
	/// Create a new filesystem watcher
	pub fn new(config: WatcherConfig) -> Self {
		let (event_tx, _) = broadcast::channel(config.event_buffer_size);

		Self {
			inner: Arc::new(FsWatcherInner {
				config,
				watched_paths: RwLock::new(HashMap::new()),
				notify_watcher: RwLock::new(None),
				platform_handler: PlatformHandler::new(),
				is_running: AtomicBool::new(false),
				event_tx,
				events_received: AtomicU64::new(0),
				events_emitted: AtomicU64::new(0),
			}),
		}
	}

	/// Start the watcher
	pub async fn start(&self) -> Result<()> {
		if self.inner.is_running.swap(true, Ordering::SeqCst) {
			return Err(WatcherError::AlreadyRunning);
		}

		info!("Starting filesystem watcher");

		// Create channel for raw events from notify
		let (raw_tx, raw_rx) = mpsc::channel(self.inner.config.event_buffer_size);

		// Create the notify watcher
		let raw_tx_clone = raw_tx.clone();
		let inner_clone = self.inner.clone();

		let watcher = notify::recommended_watcher(
			move |res: std::result::Result<notify::Event, notify::Error>| match res {
				Ok(event) => {
					inner_clone.events_received.fetch_add(1, Ordering::Relaxed);
					let raw_event = RawNotifyEvent::from_notify(event);

					if let Err(e) = raw_tx_clone.try_send(raw_event) {
						error!("Failed to send raw event: {}", e);
					}
				}
				Err(e) => {
					error!("Notify watcher error: {}", e);
				}
			},
		)
		.map_err(|e| WatcherError::StartFailed(e.to_string()))?;

		*self.inner.notify_watcher.write().await = Some(watcher);

		// Register all existing watched paths
		self.register_existing_watches().await?;

		// Start the event processing loop
		self.start_event_loop(raw_rx).await;

		info!("Filesystem watcher started");
		Ok(())
	}

	/// Stop the watcher
	pub async fn stop(&self) -> Result<()> {
		if !self.inner.is_running.swap(false, Ordering::SeqCst) {
			return Ok(()); // Already stopped
		}

		info!("Stopping filesystem watcher");

		// Clear the notify watcher
		*self.inner.notify_watcher.write().await = None;

		// Reset platform handler state
		self.inner.platform_handler.reset().await;

		info!("Filesystem watcher stopped");
		Ok(())
	}

	/// Check if the watcher is running
	pub fn is_running(&self) -> bool {
		self.inner.is_running.load(Ordering::SeqCst)
	}

	/// Watch a path
	///
	/// Returns a handle that automatically unwatches when dropped.
	pub async fn watch(&self, path: impl AsRef<Path>, config: WatchConfig) -> Result<WatchHandle> {
		let path = path.as_ref().to_path_buf();
		self.inner.add_watch(path.clone(), config).await?;

		Ok(WatchHandle {
			path,
			watcher: self.inner.clone(),
		})
	}

	/// Watch a path without returning a handle
	///
	/// Use this when you want to manually manage watch lifecycle via `unwatch()`.
	pub async fn watch_path(&self, path: impl AsRef<Path>, config: WatchConfig) -> Result<()> {
		let path = path.as_ref().to_path_buf();
		self.inner.add_watch(path, config).await
	}

	/// Unwatch a path
	pub async fn unwatch(&self, path: impl AsRef<Path>) -> Result<()> {
		self.inner.release_watch(path.as_ref()).await
	}

	/// Get all watched paths
	pub async fn watched_paths(&self) -> Vec<PathBuf> {
		self.inner
			.watched_paths
			.read()
			.await
			.keys()
			.cloned()
			.collect()
	}

	/// Subscribe to filesystem events
	pub fn subscribe(&self) -> broadcast::Receiver<FsEvent> {
		self.inner.event_tx.subscribe()
	}

	/// Get the number of events received
	pub fn events_received(&self) -> u64 {
		self.inner.events_received.load(Ordering::Relaxed)
	}

	/// Get the number of events emitted
	pub fn events_emitted(&self) -> u64 {
		self.inner.events_emitted.load(Ordering::Relaxed)
	}

	/// Register existing watches with notify
	async fn register_existing_watches(&self) -> Result<()> {
		let watched = self.inner.watched_paths.read().await;
		let mut watcher_guard = self.inner.notify_watcher.write().await;

		if let Some(watcher) = watcher_guard.as_mut() {
			for (path, state) in watched.iter() {
				let mode = notify_mode(state.config.recursive);

				for subscription in &state.subscriptions {
					if let Err(e) = watcher.watch(&subscription.subscribe, mode) {
						warn!(
							"Failed to register watch for {} as {}: {}",
							path.display(),
							subscription.subscribe.display(),
							e
						);
					} else {
						debug!(
							"Registered watch for {} as {}",
							path.display(),
							subscription.subscribe.display()
						);
					}
				}
			}
		}

		Ok(())
	}

	/// Start the event processing loop
	async fn start_event_loop(&self, mut raw_rx: mpsc::Receiver<RawNotifyEvent>) {
		let inner = self.inner.clone();
		let tick_interval = self.inner.config.tick_interval;

		tokio::spawn(async move {
			info!("Event processing loop started");

			// Use interval instead of sleep to ensure periodic ticking even when events are flowing
			let mut tick_timer = tokio::time::interval(tick_interval);
			tick_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				if !inner.is_running.load(Ordering::SeqCst) {
					break;
				}

				tokio::select! {
					// Process incoming raw events
					Some(mut raw_event) = raw_rx.recv() => {
						inner.restore_spelling(&mut raw_event).await;

						// Check if path should be filtered
						let should_process = if let Some(path) = raw_event.primary_path() {
							let watched = inner.watched_paths.read().await;
							// Find the watch config for this path
							let config = watched.iter().find(|(watched_path, _)| {
								path.starts_with(watched_path)
							}).map(|(_, state)| &state.config);

							if let Some(config) = config {
								!config.filters.should_skip(path)
							} else {
								true // No filter config, process anyway
							}
						} else {
							false
						};

						if should_process {
							// Process through platform handler
							match inner.platform_handler.process(raw_event).await {
								Ok(events) => {
									for event in events {
										inner.events_emitted.fetch_add(1, Ordering::Relaxed);
										if let Err(e) = inner.event_tx.send(event) {
											trace!("No event subscribers: {}", e);
										}
									}
								}
								Err(e) => {
									error!("Error processing event: {}", e);
								}
							}
						}
					}

					// Periodic tick for buffered event eviction
					_ = tick_timer.tick() => {
						match inner.platform_handler.tick().await {
							Ok(events) => {
								for event in events {
									inner.events_emitted.fetch_add(1, Ordering::Relaxed);
									if let Err(e) = inner.event_tx.send(event) {
										trace!("No event subscribers: {}", e);
									}
								}
							}
							Err(e) => {
								error!("Error during tick: {}", e);
							}
						}
					}
				}
			}

			info!("Event processing loop stopped");
		});
	}
}

impl Default for FsWatcher {
	fn default() -> Self {
		Self::new(WatcherConfig::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_watcher_creation() {
		let watcher = FsWatcher::new(WatcherConfig::default());
		assert!(!watcher.is_running());
	}

	#[tokio::test]
	async fn test_watcher_start_stop() {
		let watcher = FsWatcher::new(WatcherConfig::default());

		watcher.start().await.unwrap();
		assert!(watcher.is_running());

		watcher.stop().await.unwrap();
		assert!(!watcher.is_running());
	}

	#[tokio::test]
	async fn test_watch_path() {
		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();

		let temp_dir = TempDir::new().unwrap();

		watcher
			.watch_path(temp_dir.path(), WatchConfig::recursive())
			.await
			.unwrap();

		let paths = watcher.watched_paths().await;
		assert_eq!(paths.len(), 1);
		assert_eq!(paths[0], temp_dir.path());

		watcher.stop().await.unwrap();
	}

	/// A shallow watch has to actually deliver.
	///
	/// This is the regression that mattered: on macOS a `NonRecursive`
	/// subscription is accepted, reports no error, and silently delivers
	/// nothing, so every browse watch was registered and dead. The
	/// assertion is deliberately about arrival rather than about the mode we
	/// hand notify, since the mode is the workaround and arrival is the
	/// contract.
	#[tokio::test]
	async fn a_shallow_watch_delivers_events_for_immediate_children() {
		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();

		let temp_dir = TempDir::new().unwrap();
		let mut events = watcher.subscribe();

		let _handle = watcher
			.watch(temp_dir.path(), WatchConfig::shallow())
			.await
			.unwrap();

		// The backend needs a moment before it is actually subscribed.
		tokio::time::sleep(Duration::from_millis(300)).await;
		let target = temp_dir.path().join("appeared.txt");
		std::fs::write(&target, b"hello").unwrap();
		// The reported path is canonical, and a macOS temp dir reaches its
		// contents through a symlink, so the two forms have to be reconciled
		// before they can be compared.
		let target = target.canonicalize().unwrap();

		let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
		loop {
			let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
			assert!(
				!remaining.is_zero(),
				"a shallow watch delivered no event for a file created directly under it"
			);
			match tokio::time::timeout(remaining, events.recv()).await {
				Ok(Ok(event)) if event.path == target => break,
				Ok(Ok(_)) => continue,
				Ok(Err(_)) | Err(_) => panic!(
					"a shallow watch delivered no event for a file created directly under it"
				),
			}
		}

		watcher.stop().await.unwrap();
	}

	/// A watch on the long spelling has to deliver under the long spelling.
	///
	/// This is what `spelling` is for. FSEvents subscribes to the short side of
	/// a firmlink and reports events there, while Spacedrive resolves roots
	/// through the Data volume's mount point and keys its index on the long
	/// side. Registered without translation the watch delivers nothing;
	/// translated only outbound it delivers paths that match no watched root,
	/// and the index grows a second tree beside the one it already has.
	///
	/// The subject has to be a real firmlinked directory, and the home
	/// directory is the one every install has. A temp directory will not do:
	/// `/var/folders` is reached through a symlink rather than a firmlink and
	/// FSEvents refuses its resolved spelling outright, which is a separate
	/// quirk this translation does not claim to cover.
	#[cfg(target_os = "macos")]
	#[tokio::test]
	async fn a_watch_on_the_long_spelling_delivers_under_it() {
		let Ok(home) = std::env::var("HOME") else {
			return;
		};
		// A hidden directory is filtered out before it ever reaches a
		// subscriber, and a temp directory is hidden by default.
		let dir = tempfile::Builder::new()
			.prefix("sd-spelling-")
			.tempdir_in(&home)
			.unwrap();
		let short = dir.path().to_path_buf();
		let long = Path::new("/System/Volumes/Data").join(short.strip_prefix("/").unwrap());
		assert!(
			long.exists(),
			"{} should be reachable the long way round",
			short.display()
		);

		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();
		let mut events = watcher.subscribe();

		let _handle = watcher.watch(&long, WatchConfig::shallow()).await.unwrap();

		// The backend needs a moment before it is actually subscribed.
		tokio::time::sleep(Duration::from_millis(500)).await;
		std::fs::write(short.join("appeared.txt"), b"hello").unwrap();
		let expected = long.join("appeared.txt");

		let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
		loop {
			let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
			assert!(
				!remaining.is_zero(),
				"no event arrived as {}",
				expected.display()
			);
			match tokio::time::timeout(remaining, events.recv()).await {
				Ok(Ok(event)) if event.path == expected => break,
				Ok(Ok(event)) => {
					assert!(
						!event.path.starts_with(&short),
						"event arrived as {} but the watch is stored as {}",
						event.path.display(),
						long.display()
					);
					continue;
				}
				Ok(Err(_)) | Err(_) => panic!("no event arrived as {}", expected.display()),
			}
		}

		watcher.stop().await.unwrap();
	}

	#[tokio::test]
	async fn test_watch_handle_drops() {
		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();

		let temp_dir = TempDir::new().unwrap();

		{
			let _handle = watcher
				.watch(temp_dir.path(), WatchConfig::recursive())
				.await
				.unwrap();

			let paths = watcher.watched_paths().await;
			assert_eq!(paths.len(), 1);
		}

		// Give time for the async drop to complete
		tokio::time::sleep(Duration::from_millis(100)).await;

		let paths = watcher.watched_paths().await;
		assert_eq!(paths.len(), 0);

		watcher.stop().await.unwrap();
	}

	#[tokio::test]
	async fn test_reference_counting() {
		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();

		let temp_dir = TempDir::new().unwrap();

		// Watch the same path twice
		let _handle1 = watcher
			.watch(temp_dir.path(), WatchConfig::recursive())
			.await
			.unwrap();

		let _handle2 = watcher
			.watch(temp_dir.path(), WatchConfig::recursive())
			.await
			.unwrap();

		let paths = watcher.watched_paths().await;
		assert_eq!(paths.len(), 1); // Only one path in the map

		drop(_handle1);
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Should still be watched (handle2 exists)
		let paths = watcher.watched_paths().await;
		assert_eq!(paths.len(), 1);

		drop(_handle2);
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Now should be unwatched
		let paths = watcher.watched_paths().await;
		assert_eq!(paths.len(), 0);

		watcher.stop().await.unwrap();
	}

	#[tokio::test]
	async fn test_file_deletion_events() {
		let _ = tracing_subscriber::fmt()
			.with_env_filter("sd_fs_watcher=debug")
			.try_init();

		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();

		// Use home directory instead of temp - macOS FSEvents doesn't watch temp dirs
		let home = std::env::var("HOME").unwrap();
		let test_dir = PathBuf::from(home).join("SD_FS_WATCHER_TEST");
		if test_dir.exists() {
			std::fs::remove_dir_all(&test_dir).unwrap();
		}
		std::fs::create_dir_all(&test_dir).unwrap();
		let test_file = test_dir.join("test.txt");

		// Subscribe BEFORE watching to ensure we get all events
		let mut rx = watcher.subscribe();

		// Watch the directory
		watcher
			.watch_path(&test_dir, WatchConfig::recursive())
			.await
			.unwrap();

		// Give the watcher time to settle
		tokio::time::sleep(Duration::from_millis(200)).await;

		// Drain any startup events
		while let Ok(_) = rx.try_recv() {}

		// Create a file
		std::fs::write(&test_file, "initial content").unwrap();
		println!("Created file: {}", test_file.display());

		// macOS buffers creates for 500ms for rename detection, plus some processing time
		// Wait for create event with generous timeout
		let create_event = tokio::time::timeout(Duration::from_secs(3), async {
			loop {
				match rx.recv().await {
					Ok(event) => {
						println!(
							"Received event: {:?} for path: {}",
							event.kind,
							event.path.display()
						);
						// Match by filename to handle /var vs /private/var differences
						if event.path.file_name() == test_file.file_name() {
							return event;
						}
					}
					Err(e) => {
						println!("Event recv error: {}", e);
						panic!("Broadcast channel closed: {}", e);
					}
				}
			}
		})
		.await
		.expect("Timeout waiting for create event");

		println!("Got create event: {:?}", create_event.kind);
		assert!(
			matches!(create_event.kind, crate::event::FsEventKind::Create),
			"Expected Create event, got {:?}",
			create_event.kind
		);

		// Give a moment for any additional events to settle
		tokio::time::sleep(Duration::from_millis(200)).await;

		// Delete the file
		std::fs::remove_file(&test_file).unwrap();
		println!("Deleted file: {}", test_file.display());

		// The write behind the create leaves a buffered Modify that the handler
		// may flush before or after the delete, so drain the file's events until
		// the Remove arrives and check that no Create was reported in between.
		let kinds_after_delete = tokio::time::timeout(Duration::from_secs(5), async {
			let mut kinds = Vec::new();
			loop {
				match rx.recv().await {
					Ok(event) if event.path == test_file => {
						println!("Received event after delete: {:?}", event.kind);
						let is_remove = matches!(event.kind, crate::event::FsEventKind::Remove);
						kinds.push(event.kind);
						if is_remove {
							return kinds;
						}
					}
					Ok(event) => {
						println!(
							"Ignoring unrelated event after delete: {:?} for {}",
							event.kind,
							event.path.display()
						);
					}
					Err(e) => panic!("Event recv error after delete: {}", e),
				}
			}
		})
		.await
		.expect("Timeout waiting for delete event");

		// This is the critical assertion - a deletion must never surface as a Create
		assert!(
			!kinds_after_delete
				.iter()
				.any(|kind| matches!(kind, crate::event::FsEventKind::Create)),
			"BUG: Expected Remove event after file deletion, but got {:?}. \
			This indicates the watcher is misreporting deletions as creates.",
			kinds_after_delete
		);

		watcher.stop().await.unwrap();

		// Cleanup
		let _ = std::fs::remove_dir_all(&test_dir);
	}

	#[tokio::test]
	async fn test_file_modify_then_delete() {
		let _ = tracing_subscriber::fmt()
			.with_env_filter("sd_fs_watcher=debug")
			.try_init();

		let watcher = FsWatcher::new(WatcherConfig::default());
		watcher.start().await.unwrap();

		// Use home directory instead of temp - macOS FSEvents doesn't watch temp dirs
		let home = std::env::var("HOME").unwrap();
		let test_dir = PathBuf::from(home).join("SD_FS_WATCHER_TEST_2");
		if test_dir.exists() {
			std::fs::remove_dir_all(&test_dir).unwrap();
		}
		std::fs::create_dir_all(&test_dir).unwrap();
		let test_file = test_dir.join("document.txt");

		let mut rx = watcher.subscribe();

		watcher
			.watch_path(&test_dir, WatchConfig::recursive())
			.await
			.unwrap();

		tokio::time::sleep(Duration::from_millis(200)).await;

		// Drain any startup events
		while let Ok(_) = rx.try_recv() {}

		// Create file
		std::fs::write(&test_file, "Hello World").unwrap();
		println!("Created file: {}", test_file.display());

		// Wait for create event
		let _create = tokio::time::timeout(Duration::from_secs(2), async {
			loop {
				if let Ok(event) = rx.recv().await {
					if event.path == test_file
						&& matches!(event.kind, crate::event::FsEventKind::Create)
					{
						println!("Got create event");
						return;
					}
				}
			}
		})
		.await
		.expect("Timeout waiting for create");

		tokio::time::sleep(Duration::from_millis(200)).await;

		// Modify file using tokio::fs::write (like the failing test does)
		tokio::fs::write(&test_file, "Hello World - Updated!")
			.await
			.unwrap();
		println!("Modified file: {}", test_file.display());

		// Collect modify events (could be Create or Modify depending on platform)
		tokio::time::sleep(Duration::from_millis(500)).await;
		while let Ok(event) = rx.try_recv() {
			if event.path == test_file {
				println!("Got modify-related event: {:?}", event.kind);
			}
		}

		// Delete file
		tokio::fs::remove_file(&test_file).await.unwrap();
		println!("Deleted file: {}", test_file.display());

		// Wait for delete event
		let delete_event = tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				match rx.recv().await {
					Ok(event) if event.path == test_file => {
						println!(
							"Received event after delete: {:?} for {}",
							event.kind,
							event.path.display()
						);
						return event;
					}
					Ok(event) => {
						println!(
							"Ignoring event: {:?} for {}",
							event.kind,
							event.path.display()
						);
					}
					Err(_) => break,
				}
			}
			panic!("No delete event received");
		})
		.await
		.expect("Timeout waiting for delete event");

		// Critical assertion - this mimics what the integration test does
		assert!(
			matches!(delete_event.kind, crate::event::FsEventKind::Remove),
			"BUG: After create->modify->delete sequence, expected Remove event but got {:?}. \
			This reproduces the integration test failure where deletions are reported as Creates.",
			delete_event.kind
		);

		watcher.stop().await.unwrap();

		// Cleanup
		let _ = std::fs::remove_dir_all(&test_dir);
	}
}
