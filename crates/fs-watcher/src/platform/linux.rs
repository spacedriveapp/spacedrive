//! Linux-specific event handler
//!
//! Linux inotify provides better rename tracking than macOS FSEvents,
//! but still requires some buffering for reliable handling.

use crate::event::{FsEvent, RawEventKind, RawNotifyEvent};
use crate::platform::EventHandler;
use crate::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::trace;

/// Timeout for event stabilization
const STABILIZATION_TIMEOUT_MS: u64 = 100;

/// Linux event handler
pub struct LinuxHandler {
    /// Files pending stabilization
    pending_updates: RwLock<HashMap<PathBuf, Instant>>,
}

impl LinuxHandler {
    /// Create a new Linux handler
    pub fn new() -> Self {
        Self {
            pending_updates: RwLock::new(HashMap::new()),
        }
    }

    /// Evict pending updates that have stabilized
    async fn evict_updates(&self, timeout: Duration) -> Vec<FsEvent> {
        let mut events = Vec::new();
        let mut updates = self.pending_updates.write().await;
        let mut to_remove = Vec::new();

        for (path, timestamp) in updates.iter() {
            if timestamp.elapsed() > timeout {
                to_remove.push(path.clone());
                events.push(FsEvent::modify(path.clone()));
                trace!("Evicting update (stabilized): {}", path.display());
            }
        }

        for path in to_remove {
            updates.remove(&path);
        }

        events
    }
}

impl Default for LinuxHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventHandler for LinuxHandler {
    async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>> {
        let Some(path) = event.primary_path().cloned() else {
            return Ok(vec![]);
        };

        match event.kind {
            RawEventKind::Create => Ok(vec![FsEvent::create(path)]),
            RawEventKind::Remove => Ok(vec![FsEvent::remove(path)]),
            RawEventKind::Modify => {
                // Buffer modifications for stabilization
                let mut updates = self.pending_updates.write().await;
                updates.insert(path, Instant::now());
                Ok(vec![])
            }
            RawEventKind::Rename => {
                // inotify provides rename events with both paths
                if event.paths.len() >= 2 {
                    let from = event.paths[0].clone();
                    let to = event.paths[1].clone();
                    Ok(vec![FsEvent::rename(from, to)])
                } else {
                    // Incomplete rename, treat as modify
                    let mut updates = self.pending_updates.write().await;
                    updates.insert(path, Instant::now());
                    Ok(vec![])
                }
            }
            RawEventKind::Other(ref kind) => {
                trace!("Ignoring unknown event kind: {}", kind);
                Ok(vec![])
            }
        }
    }

    async fn tick(&self) -> Result<Vec<FsEvent>> {
        let timeout = Duration::from_millis(STABILIZATION_TIMEOUT_MS);
        Ok(self.evict_updates(timeout).await)
    }

    async fn reset(&self) {
        self.pending_updates.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_creation() {
        let handler = LinuxHandler::new();
        assert!(handler.pending_updates.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_create_event() {
        let handler = LinuxHandler::new();
        let event = RawNotifyEvent {
            kind: RawEventKind::Create,
            paths: vec![PathBuf::from("/test/file.txt")],
            timestamp: std::time::SystemTime::now(),
        };

        let events = handler.process(event).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].kind.is_create());
    }

    #[tokio::test]
    async fn test_remove_event() {
        let handler = LinuxHandler::new();
        let event = RawNotifyEvent {
            kind: RawEventKind::Remove,
            paths: vec![PathBuf::from("/test/file.txt")],
            timestamp: std::time::SystemTime::now(),
        };

        let events = handler.process(event).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].kind.is_remove());
    }

    #[tokio::test]
    async fn test_rename_event() {
        let handler = LinuxHandler::new();
        let event = RawNotifyEvent {
            kind: RawEventKind::Rename,
            paths: vec![
                PathBuf::from("/test/old.txt"),
                PathBuf::from("/test/new.txt"),
            ],
            timestamp: std::time::SystemTime::now(),
        };

        let events = handler.process(event).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].kind.is_rename());
    }
}
