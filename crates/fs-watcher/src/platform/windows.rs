//! Windows-specific event handler
//!
//! Windows ReadDirectoryChangesW provides reasonable rename tracking,
//! but still benefits from event buffering for stability.

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

/// Windows event handler
pub struct WindowsHandler {
    /// Files pending stabilization
    pending_updates: RwLock<HashMap<PathBuf, Instant>>,

    /// Pending rename sources (waiting for target)
    pending_rename_from: RwLock<Option<(PathBuf, Instant)>>,
}

impl WindowsHandler {
    /// Create a new Windows handler
    pub fn new() -> Self {
        Self {
            pending_updates: RwLock::new(HashMap::new()),
            pending_rename_from: RwLock::new(None),
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

    /// Evict pending rename source if timed out
    async fn evict_pending_rename(&self, timeout: Duration) -> Vec<FsEvent> {
        let mut events = Vec::new();
        let mut pending = self.pending_rename_from.write().await;

        if let Some((path, timestamp)) = pending.take() {
            if timestamp.elapsed() > timeout {
                // Rename source without target - treat as remove
                events.push(FsEvent::remove(path));
            } else {
                // Put it back
                *pending = Some((path, timestamp));
            }
        }

        events
    }
}

impl Default for WindowsHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventHandler for WindowsHandler {
    async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>> {
        let Some(path) = event.primary_path().cloned() else {
            return Ok(vec![]);
        };

        match event.kind {
            RawEventKind::Create => {
                // Check if this matches a pending rename
                let pending = self.pending_rename_from.write().await.take();
                if let Some((from_path, _)) = pending {
                    return Ok(vec![FsEvent::rename(from_path, path)]);
                }
                Ok(vec![FsEvent::create(path)])
            }
            RawEventKind::Remove => {
                // Buffer as potential rename source
                let mut pending = self.pending_rename_from.write().await;
                *pending = Some((path, Instant::now()));
                Ok(vec![])
            }
            RawEventKind::Modify => {
                // Buffer modifications for stabilization
                let mut updates = self.pending_updates.write().await;
                updates.insert(path, Instant::now());
                Ok(vec![])
            }
            RawEventKind::Rename => {
                // Windows sometimes provides proper rename events
                if event.paths.len() >= 2 {
                    let from = event.paths[0].clone();
                    let to = event.paths[1].clone();
                    Ok(vec![FsEvent::rename(from, to)])
                } else {
                    // Incomplete rename, buffer it
                    let mut pending = self.pending_rename_from.write().await;
                    *pending = Some((path, Instant::now()));
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
        let mut events = Vec::new();

        events.extend(self.evict_updates(timeout).await);
        events.extend(self.evict_pending_rename(timeout).await);

        Ok(events)
    }

    async fn reset(&self) {
        self.pending_updates.write().await.clear();
        *self.pending_rename_from.write().await = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_creation() {
        let handler = WindowsHandler::new();
        assert!(handler.pending_updates.read().await.is_empty());
        assert!(handler.pending_rename_from.read().await.is_none());
    }

    #[tokio::test]
    async fn test_create_event() {
        let handler = WindowsHandler::new();
        let event = RawNotifyEvent {
            kind: RawEventKind::Create,
            paths: vec![PathBuf::from("C:\\test\\file.txt")],
            timestamp: std::time::SystemTime::now(),
        };

        let events = handler.process(event).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].kind.is_create());
    }

    #[tokio::test]
    async fn test_rename_detection() {
        let handler = WindowsHandler::new();

        // First, a remove event (potential rename source)
        let remove_event = RawNotifyEvent {
            kind: RawEventKind::Remove,
            paths: vec![PathBuf::from("C:\\test\\old.txt")],
            timestamp: std::time::SystemTime::now(),
        };
        let events = handler.process(remove_event).await.unwrap();
        assert!(events.is_empty()); // Buffered

        // Then, a create event (rename target)
        let create_event = RawNotifyEvent {
            kind: RawEventKind::Create,
            paths: vec![PathBuf::from("C:\\test\\new.txt")],
            timestamp: std::time::SystemTime::now(),
        };
        let events = handler.process(create_event).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].kind.is_rename());
    }
}
