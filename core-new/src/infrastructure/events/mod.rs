//! Event bus for decoupled communication

use std::path::PathBuf;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Library-related events
#[derive(Debug, Clone)]
pub enum Event {
    /// Core has started
    CoreStarted,
    
    /// Core is shutting down
    CoreShutdown,
    
    /// A new library was created
    LibraryCreated {
        id: Uuid,
        name: String,
        path: PathBuf,
    },
    
    /// A library was opened
    LibraryOpened {
        id: Uuid,
        name: String,
        path: PathBuf,
    },
    
    /// A library was closed
    LibraryClosed {
        id: Uuid,
        name: String,
    },
    
    /// A location was added to a library
    LocationAdded {
        library_id: Uuid,
        location_id: Uuid,
        path: PathBuf,
    },
    
    /// A location was removed from a library
    LocationRemoved {
        library_id: Uuid,
        location_id: Uuid,
    },
    
    /// Files were indexed
    FilesIndexed {
        library_id: Uuid,
        location_id: Uuid,
        count: usize,
    },
    
    /// Thumbnails were generated
    ThumbnailsGenerated {
        library_id: Uuid,
        count: usize,
    },
    
    /// A file operation completed
    FileOperationCompleted {
        library_id: Uuid,
        operation: FileOperation,
        affected_files: usize,
    },
    
    /// Files were modified
    FilesModified {
        library_id: Uuid,
        paths: Vec<PathBuf>,
    },
}

/// Types of file operations
#[derive(Debug, Clone)]
pub enum FileOperation {
    Copy,
    Move,
    Delete,
    Rename,
}

/// Type alias for compatibility
pub type Events = Event;

/// Event bus for broadcasting events
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new event bus with specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
    
    /// Emit an event
    pub fn emit(&self, event: Event) {
        // Ignore send errors (no receivers)
        let _ = self.sender.send(event);
    }
    
    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}