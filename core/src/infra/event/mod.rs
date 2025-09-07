//! Event bus for decoupled communication

use crate::infra::jobs::output::JobOutput;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tracing::{debug, warn};
use uuid::Uuid;

/// Core events that can be emitted throughout the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // Core lifecycle events
    CoreStarted,
    CoreShutdown,

    // Library events
    LibraryCreated { id: Uuid, name: String, path: PathBuf },
    LibraryOpened { id: Uuid, name: String, path: PathBuf },
    LibraryClosed { id: Uuid, name: String },
    LibraryDeleted { id: Uuid },

    // Entry events (file/directory operations)
    EntryCreated { library_id: Uuid, entry_id: Uuid },
    EntryModified { library_id: Uuid, entry_id: Uuid },
    EntryDeleted { library_id: Uuid, entry_id: Uuid },
    EntryMoved {
        library_id: Uuid,
        entry_id: Uuid,
        old_path: String,
        new_path: String
    },

    // Volume events
    VolumeAdded(crate::volume::Volume),
    VolumeRemoved {
        fingerprint: crate::volume::VolumeFingerprint
    },
    VolumeUpdated {
        fingerprint: crate::volume::VolumeFingerprint,
        old_info: crate::volume::VolumeInfo,
        new_info: crate::volume::VolumeInfo,
    },
    VolumeSpeedTested {
        fingerprint: crate::volume::VolumeFingerprint,
        read_speed_mbps: u64,
        write_speed_mbps: u64,
    },
    VolumeMountChanged {
        fingerprint: crate::volume::VolumeFingerprint,
        is_mounted: bool,
    },
    VolumeError {
        fingerprint: crate::volume::VolumeFingerprint,
        error: String,
    },

    // Job events
    JobQueued { job_id: String, job_type: String },
    JobStarted { job_id: String, job_type: String },
    JobProgress {
        job_id: String,
        job_type: String,
        progress: f64,
        message: Option<String>,
        // Enhanced progress data - serialized GenericProgress
        generic_progress: Option<serde_json::Value>,
    },
    JobCompleted { job_id: String, job_type: String, output: JobOutput },
    JobFailed {
        job_id: String,
        job_type: String,
        error: String
    },
    JobCancelled { job_id: String, job_type: String },
    JobPaused { job_id: String },
    JobResumed { job_id: String },

    // Indexing events
    IndexingStarted { location_id: Uuid },
    IndexingProgress {
        location_id: Uuid,
        processed: u64,
        total: Option<u64>
    },
    IndexingCompleted {
        location_id: Uuid,
        total_files: u64,
        total_dirs: u64
    },
    IndexingFailed { location_id: Uuid, error: String },

    // Device events
    DeviceConnected { device_id: Uuid, device_name: String },
    DeviceDisconnected { device_id: Uuid },

    // Legacy events (for compatibility)
    LocationAdded {
        library_id: Uuid,
        location_id: Uuid,
        path: PathBuf,
    },
    LocationRemoved {
        library_id: Uuid,
        location_id: Uuid,
    },
    FilesIndexed {
        library_id: Uuid,
        location_id: Uuid,
        count: usize,
    },
    ThumbnailsGenerated {
        library_id: Uuid,
        count: usize,
    },
    FileOperationCompleted {
        library_id: Uuid,
        operation: FileOperation,
        affected_files: usize,
    },
    FilesModified {
        library_id: Uuid,
        paths: Vec<PathBuf>,
    },

    // Custom events for extensibility
    Custom {
        event_type: String,
        data: serde_json::Value
    },
}

/// Types of file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Copy,
    Move,
    Delete,
    Rename,
}

/// Event bus for broadcasting events
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new event bus with specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Emit an event to all subscribers
    pub fn emit(&self, event: Event) {
        match self.sender.send(event.clone()) {
            Ok(subscriber_count) => {
                debug!("Event emitted to {} subscribers", subscriber_count);
            }
            Err(_) => {
                // No subscribers - this is fine, just debug log it
                debug!("Event emitted but no subscribers: {:?}", event);
            }
        }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> EventSubscriber {
        EventSubscriber {
            receiver: self.sender.subscribe(),
        }
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Event subscriber for receiving events
#[derive(Debug)]
pub struct EventSubscriber {
    receiver: broadcast::Receiver<Event>,
}

impl EventSubscriber {
    /// Receive the next event (blocking)
    pub async fn recv(&mut self) -> Result<Event, broadcast::error::RecvError> {
        self.receiver.recv().await
    }

    /// Try to receive an event without blocking
    pub fn try_recv(&mut self) -> Result<Event, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Filter events by type using a closure
    pub async fn recv_filtered<F>(&mut self, filter: F) -> Result<Event, broadcast::error::RecvError>
    where
        F: Fn(&Event) -> bool,
    {
        loop {
            let event = self.recv().await?;
            if filter(&event) {
                return Ok(event);
            }
        }
    }
}

/// Helper trait for event filtering
pub trait EventFilter {
    fn is_library_event(&self) -> bool;
    fn is_volume_event(&self) -> bool;
    fn is_job_event(&self) -> bool;
    fn is_for_library(&self, library_id: Uuid) -> bool;
}

impl EventFilter for Event {
    fn is_library_event(&self) -> bool {
        matches!(
            self,
            Event::LibraryCreated { .. }
                | Event::LibraryOpened { .. }
                | Event::LibraryClosed { .. }
                | Event::LibraryDeleted { .. }
                | Event::EntryCreated { .. }
                | Event::EntryModified { .. }
                | Event::EntryDeleted { .. }
                | Event::EntryMoved { .. }
        )
    }

    fn is_volume_event(&self) -> bool {
        matches!(
            self,
            Event::VolumeAdded(_)
                | Event::VolumeRemoved { .. }
                | Event::VolumeUpdated { .. }
                | Event::VolumeSpeedTested { .. }
                | Event::VolumeMountChanged { .. }
                | Event::VolumeError { .. }
        )
    }

    fn is_job_event(&self) -> bool {
        matches!(
            self,
            Event::JobQueued { .. }
                | Event::JobStarted { .. }
                | Event::JobProgress { .. }
                | Event::JobCompleted { .. }
                | Event::JobFailed { .. }
                | Event::JobCancelled { .. }
        )
    }

    fn is_for_library(&self, library_id: Uuid) -> bool {
        match self {
            Event::LibraryCreated { id, .. }
            | Event::LibraryOpened { id, .. }
            | Event::LibraryClosed { id, .. }
            | Event::LibraryDeleted { id } => *id == library_id,
            Event::EntryCreated { library_id: lid, .. }
            | Event::EntryModified { library_id: lid, .. }
            | Event::EntryDeleted { library_id: lid, .. }
            | Event::EntryMoved { library_id: lid, .. } => *lid == library_id,
            Event::LocationAdded { library_id: lid, .. }
            | Event::LocationRemoved { library_id: lid, .. }
            | Event::FilesIndexed { library_id: lid, .. }
            | Event::ThumbnailsGenerated { library_id: lid, .. }
            | Event::FileOperationCompleted { library_id: lid, .. }
            | Event::FilesModified { library_id: lid, .. } => *lid == library_id,
            _ => false,
        }
    }
}