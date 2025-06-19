//! Linux-specific file system event handling using inotify

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
use tracing::{debug, trace};
use uuid::Uuid;

/// Linux-specific event handler that uses inotify
pub struct LinuxHandler {
    /// Recently processed events for debouncing
    recent_events: Arc<RwLock<HashMap<PathBuf, Instant>>>,
    /// Debounce duration
    debounce_duration: Duration,
}

impl LinuxHandler {
    pub fn new() -> Self {
        Self {
            recent_events: Arc::new(RwLock::new(HashMap::new())),
            debounce_duration: Duration::from_millis(50), // Linux inotify is generally faster
        }
    }

    /// Check if event should be debounced
    async fn should_debounce(&self, path: &PathBuf) -> bool {
        let mut recent = self.recent_events.write().await;
        let now = Instant::now();
        
        // Check if we've seen this path recently
        if let Some(&last_seen) = recent.get(path) {
            if now.duration_since(last_seen) < self.debounce_duration {
                return true;
            }
        }
        
        recent.insert(path.clone(), now);
        
        // Cleanup old entries
        recent.retain(|_, &mut last_seen| {
            now.duration_since(last_seen) < Duration::from_secs(1)
        });
        
        false
    }

    /// Handle Linux-specific rename detection
    /// Linux inotify provides cleaner rename events compared to macOS
    async fn handle_rename_events(
        &self,
        event: &WatcherEvent,
        watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
    ) -> Result<Vec<Event>> {
        let mut events = Vec::new();

        if let WatcherEventKind::Rename { from, to } = &event.kind {
            let locations = watched_locations.read().await;
            for location in locations.values() {
                if location.enabled && (from.starts_with(&location.path) || to.starts_with(&location.path)) {
                    let entry_id = Uuid::new_v4(); // TODO: Look up actual entry
                    events.push(Event::EntryMoved {
                        library_id: location.library_id,
                        entry_id,
                        old_path: from.to_string_lossy().to_string(),
                        new_path: to.to_string_lossy().to_string(),
                    });
                    break;
                }
            }
        }

        Ok(events)
    }
}

#[async_trait::async_trait]
impl EventHandler for LinuxHandler {
    async fn process_event(
        &self,
        event: WatcherEvent,
        watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
    ) -> Result<Vec<Event>> {
        if !event.should_process() {
            return Ok(vec![]);
        }

        let mut events = Vec::new();

        // Handle rename events specially
        if let WatcherEventKind::Rename { .. } = &event.kind {
            return self.handle_rename_events(&event, watched_locations).await;
        }

        // Process other event types
        for path in &event.paths {
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
                        trace!("Linux: Created {}", path.display());
                    }
                    WatcherEventKind::Modify => {
                        events.push(Event::EntryModified {
                            library_id: location.library_id,
                            entry_id,
                        });
                        trace!("Linux: Modified {}", path.display());
                    }
                    WatcherEventKind::Remove => {
                        events.push(Event::EntryDeleted {
                            library_id: location.library_id,
                            entry_id,
                        });
                        trace!("Linux: Removed {}", path.display());
                    }
                    _ => {
                        trace!("Linux: Unhandled event type for {}", path.display());
                    }
                }
                break;
            }
        }

        Ok(events)
    }

    async fn tick(&self) -> Result<()> {
        // Linux inotify is generally more reliable and doesn't need as much cleanup
        let mut recent = self.recent_events.write().await;
        let now = Instant::now();
        
        // Clean up old debounce entries
        recent.retain(|_, &mut last_seen| {
            now.duration_since(last_seen) < Duration::from_secs(5)
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_linux_handler_creation() {
        let handler = LinuxHandler::new();
        assert_eq!(handler.debounce_duration, Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_debounce_logic() {
        let handler = LinuxHandler::new();
        let path = PathBuf::from("/test/file.txt");
        
        // First event should not be debounced
        assert!(!handler.should_debounce(&path).await);
        
        // Second immediate event should be debounced
        assert!(handler.should_debounce(&path).await);
        
        // Wait for debounce period and try again
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(!handler.should_debounce(&path).await);
    }

    #[tokio::test]
    async fn test_rename_event_handling() {
        let handler = LinuxHandler::new();
        let watched_locations = Arc::new(RwLock::new(HashMap::new()));
        
        let event = WatcherEvent {
            kind: WatcherEventKind::Rename {
                from: PathBuf::from("/test/old.txt"),
                to: PathBuf::from("/test/new.txt"),
            },
            paths: vec![PathBuf::from("/test/old.txt"), PathBuf::from("/test/new.txt")],
            timestamp: SystemTime::now(),
            attrs: vec![],
        };

        let events = handler.handle_rename_events(&event, &watched_locations).await.unwrap();
        // Should be empty since no locations are configured
        assert_eq!(events.len(), 0);
    }
}