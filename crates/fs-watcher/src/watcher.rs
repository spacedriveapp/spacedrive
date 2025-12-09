//! Main filesystem watcher implementation
//!
//! `FsWatcher` is the primary interface for watching filesystem changes.
//! It's storage-agnostic - it only knows about paths and events, not
//! about locations, libraries, or databases.

use crate::config::{WatchConfig, WatcherConfig};
use crate::error::{Result, WatcherError};
use crate::event::{FsEvent, RawNotifyEvent};
use crate::platform::PlatformHandler;
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
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

        // Register with notify if we're running
        if self.is_running.load(Ordering::SeqCst) {
            if let Some(watcher) = self.notify_watcher.write().await.as_mut() {
                let mode = if config.recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };

                watcher.watch(&path, mode).map_err(|e| WatcherError::WatchFailed {
                    path: path.clone(),
                    reason: e.to_string(),
                })?;
            }
        }

        watched.insert(
            path.clone(),
            WatchState {
                config,
                ref_count: 1,
            },
        );

        debug!("Started watching: {}", path.display());
        Ok(())
    }

    /// Release a watch (decrement ref count, unwatch if zero)
    async fn release_watch(&self, path: &Path) -> Result<()> {
        let mut watched = self.watched_paths.write().await;

        let should_unwatch = if let Some(state) = watched.get_mut(path) {
            state.ref_count -= 1;
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
                    if let Err(e) = watcher.unwatch(path) {
                        warn!("Failed to unwatch {}: {}", path.display(), e);
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

        let watcher = notify::recommended_watcher(move |res: std::result::Result<notify::Event, notify::Error>| {
            match res {
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
            }
        })
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
                let mode = if state.config.recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };

                if let Err(e) = watcher.watch(path, mode) {
                    warn!("Failed to register watch for {}: {}", path.display(), e);
                } else {
                    debug!("Registered watch for: {}", path.display());
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

            loop {
                if !inner.is_running.load(Ordering::SeqCst) {
                    break;
                }

                tokio::select! {
                    // Process incoming raw events
                    Some(raw_event) = raw_rx.recv() => {
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
                    _ = tokio::time::sleep(tick_interval) => {
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
}
