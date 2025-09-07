//! Windows-specific file system event handling

use super::EventHandler;
use crate::infrastructure::events::Event;
use crate::services::location_watcher::{WatchedLocation, WatcherEvent};
use crate::services::location_watcher::event_handler::WatcherEventKind;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};
use uuid::Uuid;

/// Windows-specific event handler that handles Windows filesystem quirks
pub struct WindowsHandler {
    /// Recently processed events for debouncing
    recent_events: Arc<RwLock<HashMap<PathBuf, Instant>>>,
    /// Files pending deletion (Windows has delayed deletion)
    pending_deletions: Arc<RwLock<HashMap<PathBuf, Instant>>>,
    /// Debounce duration
    debounce_duration: Duration,
}

impl WindowsHandler {
    pub fn new() -> Self {
        Self {
            recent_events: Arc::new(RwLock::new(HashMap::new())),
            pending_deletions: Arc::new(RwLock::new(HashMap::new())),
            debounce_duration: Duration::from_millis(200), // Windows needs more debouncing
        }
    }

    /// Check if event should be debounced
    async fn should_debounce(&self, path: &PathBuf) -> bool {
        let mut recent = self.recent_events.write().await;
        let now = Instant::now();

        if let Some(&last_seen) = recent.get(path) {
            if now.duration_since(last_seen) < self.debounce_duration {
                return true;
            }
        }

        recent.insert(path.clone(), now);

        // Cleanup old entries
        recent.retain(|_, &mut last_seen| {
            now.duration_since(last_seen) < Duration::from_secs(2)
        });

        false
    }

    /// Handle Windows-specific delayed deletion detection
    async fn handle_delayed_deletion(&self, path: &PathBuf) -> bool {
        // On Windows, files can appear to be deleted but still be accessible
        // for a short time due to file locking, antivirus, etc.
        match tokio::fs::metadata(path).await {
            Ok(_) => {
                // File still exists, might be a false deletion event
                let mut pending = self.pending_deletions.write().await;
                pending.insert(path.clone(), Instant::now());
                false // Don't process deletion yet
            }
            Err(_) => {
                // File actually doesn't exist
                let mut pending = self.pending_deletions.write().await;
                pending.remove(path);
                true // Process deletion
            }
        }
    }

    /// Process pending deletions that have timed out
    async fn process_pending_deletions(
        &self,
        watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
    ) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        let mut pending = self.pending_deletions.write().await;
        let now = Instant::now();

        let mut to_remove = Vec::new();
        for (path, timestamp) in pending.iter() {
            if now.duration_since(*timestamp) > Duration::from_millis(500) {
                // Check if file still doesn't exist
                if tokio::fs::metadata(path).await.is_err() {
                    // File is definitely gone, emit deletion event
                    let locations = watched_locations.read().await;
                    for location in locations.values() {
                        if location.enabled && path.starts_with(&location.path) {
                            let entry_id = Uuid::new_v4(); // TODO: Look up actual entry
                            events.push(Event::EntryDeleted {
                                library_id: location.library_id,
                                entry_id,
                            });
                            break;
                        }
                    }
                }
                to_remove.push(path.clone());
            }
        }

        for path in to_remove {
            pending.remove(&path);
        }

        Ok(events)
    }

    /// Handle Windows-specific temporary file patterns
    fn is_windows_temp_file(&self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Windows common temporary file patterns
        path_str.contains("~$") || // Office temp files
        path_str.ends_with(".tmp") ||
        path_str.ends_with(".temp") ||
        path_str.contains(".crdownload") || // Chrome downloads
        path_str.contains(".part") || // Firefox downloads
        path_str.contains("thumbs.db") ||
        path_str.contains("desktop.ini") ||
        path_str.contains("$recycle.bin")
    }
}

#[async_trait::async_trait]
impl EventHandler for WindowsHandler {
    async fn process_event(
        &self,
        event: WatcherEvent,
        watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
    ) -> Result<Vec<Event>> {
        if !event.should_process() {
            return Ok(vec![]);
        }

        let mut events = Vec::new();

        for path in &event.paths {
            // Skip Windows-specific temporary files
            if self.is_windows_temp_file(path) {
                trace!("Skipping Windows temp file: {}", path.display());
                continue;
            }

            // Check for debouncing
            if self.should_debounce(path).await {
                debug!("Debounced event for: {}", path.display());
                continue;
            }

            let locations = watched_locations.read().await;
            for location in locations.values() {
                if !location.enabled || !path.starts_with(&location.path) {
                    continue;
                }

                let entry_id = Uuid::new_v4(); // TODO: Look up or create actual entry

                match &event.kind {
                    WatcherEventKind::Create => {
                        events.push(Event::EntryCreated {
                            library_id: location.library_id,
                            entry_id,
                        });
                        trace!("Windows: Created {}", path.display());
                    }
                    WatcherEventKind::Modify => {
                        events.push(Event::EntryModified {
                            library_id: location.library_id,
                            entry_id,
                        });
                        trace!("Windows: Modified {}", path.display());
                    }
                    WatcherEventKind::Remove => {
                        // Handle Windows delayed deletion
                        if self.handle_delayed_deletion(path).await {
                            events.push(Event::EntryDeleted {
                                library_id: location.library_id,
                                entry_id,
                            });
                            trace!("Windows: Removed {}", path.display());
                        } else {
                            trace!("Windows: Pending deletion for {}", path.display());
                        }
                    }
                    WatcherEventKind::Rename { from, to } => {
                        events.push(Event::EntryMoved {
                            library_id: location.library_id,
                            entry_id,
                            old_path: from.to_string_lossy().to_string(),
                            new_path: to.to_string_lossy().to_string(),
                        });
                        trace!("Windows: Renamed {} -> {}", from.display(), to.display());
                    }
                    _ => {
                        trace!("Windows: Unhandled event type for {}", path.display());
                    }
                }
                break;
            }
        }

        Ok(events)
    }

    async fn tick(&self) -> Result<()> {
        // Clean up recent events
        let mut recent = self.recent_events.write().await;
        let now = Instant::now();
        recent.retain(|_, &mut last_seen| {
            now.duration_since(last_seen) < Duration::from_secs(5)
        });

        Ok(())
    }
}

/// Additional method for Windows handler to process pending deletions
impl WindowsHandler {
    pub async fn tick_with_locations(
        &self,
        watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
    ) -> Result<Vec<Event>> {
        // Process any pending deletions
        self.process_pending_deletions(watched_locations).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_windows_handler_creation() {
        let handler = WindowsHandler::new();
        assert_eq!(handler.debounce_duration, Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_windows_temp_file_detection() {
        let handler = WindowsHandler::new();

        // Should detect Windows temp files
        assert!(handler.is_windows_temp_file(&PathBuf::from(r"C:\temp\~$document.docx")));
        assert!(handler.is_windows_temp_file(&PathBuf::from(r"C:\temp\file.tmp")));
        assert!(handler.is_windows_temp_file(&PathBuf::from(r"C:\temp\Thumbs.db")));
        assert!(handler.is_windows_temp_file(&PathBuf::from(r"C:\temp\desktop.ini")));

        // Should not detect normal files
        assert!(!handler.is_windows_temp_file(&PathBuf::from(r"C:\temp\document.docx")));
        assert!(!handler.is_windows_temp_file(&PathBuf::from(r"C:\temp\image.jpg")));
    }

    #[tokio::test]
    async fn test_debounce_logic() {
        let handler = WindowsHandler::new();
        let path = PathBuf::from(r"C:\test\file.txt");

        // First event should not be debounced
        assert!(!handler.should_debounce(&path).await);

        // Second immediate event should be debounced
        assert!(handler.should_debounce(&path).await);
    }
}