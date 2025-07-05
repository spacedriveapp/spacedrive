//! Action execution output types

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

/// Output returned from action execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionOutput {
    /// Action completed successfully with no specific output
    Success,
    
    /// Library creation output
    LibraryCreate {
        library_id: Uuid,
        name: String,
        path: PathBuf,
    },
    
    /// Library deletion output
    LibraryDelete {
        library_id: Uuid,
    },
    
    /// Folder creation output
    FolderCreate {
        folder_id: Uuid,
        path: PathBuf,
    },
    
    /// File copy dispatch output (action just dispatches to job)
    FileCopyDispatched {
        job_id: Uuid,
        sources_count: usize,
    },
    
    /// File delete dispatch output
    FileDeleteDispatched {
        job_id: Uuid,
        targets_count: usize,
    },
    
    /// Location management outputs
    LocationAdd {
        location_id: Uuid,
        path: PathBuf,
    },
    
    LocationRemove {
        location_id: Uuid,
    },
    
    /// File indexing dispatch output
    FileIndexDispatched {
        job_id: Uuid,
        location_id: Uuid,
    },
    
    /// Generic output with custom data
    Custom(serde_json::Value),
}

impl ActionOutput {
    /// Create a custom output with serializable data
    pub fn custom<T: Serialize>(data: T) -> Self {
        Self::Custom(serde_json::to_value(data).unwrap_or(serde_json::Value::Null))
    }
    
    /// Create a library creation output
    pub fn library_create(library_id: Uuid, name: String, path: PathBuf) -> Self {
        Self::LibraryCreate { library_id, name, path }
    }
    
    /// Create a library deletion output
    pub fn library_delete(library_id: Uuid) -> Self {
        Self::LibraryDelete { library_id }
    }
    
    /// Create a folder creation output
    pub fn folder_create(folder_id: Uuid, path: PathBuf) -> Self {
        Self::FolderCreate { folder_id, path }
    }
    
    /// Create a file copy dispatch output
    pub fn file_copy_dispatched(job_id: Uuid, sources_count: usize) -> Self {
        Self::FileCopyDispatched { job_id, sources_count }
    }
    
    /// Create a file delete dispatch output
    pub fn file_delete_dispatched(job_id: Uuid, targets_count: usize) -> Self {
        Self::FileDeleteDispatched { job_id, targets_count }
    }
    
    /// Create a location add output
    pub fn location_add(location_id: Uuid, path: PathBuf) -> Self {
        Self::LocationAdd { location_id, path }
    }
    
    /// Create a location remove output
    pub fn location_remove(location_id: Uuid) -> Self {
        Self::LocationRemove { location_id }
    }
    
    /// Create a file index dispatch output
    pub fn file_index_dispatched(job_id: Uuid, location_id: Uuid) -> Self {
        Self::FileIndexDispatched { job_id, location_id }
    }
}

impl Default for ActionOutput {
    fn default() -> Self {
        Self::Success
    }
}

impl fmt::Display for ActionOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionOutput::Success => write!(f, "Action completed successfully"),
            ActionOutput::LibraryCreate { library_id, name, path } => {
                write!(f, "Created library '{}' (ID: {}) at path: {}", name, library_id, path.display())
            }
            ActionOutput::LibraryDelete { library_id } => {
                write!(f, "Deleted library with ID: {}", library_id)
            }
            ActionOutput::FolderCreate { folder_id, path } => {
                write!(f, "Created folder (ID: {}) at path: {}", folder_id, path.display())
            }
            ActionOutput::FileCopyDispatched { job_id, sources_count } => {
                write!(f, "Dispatched file copy job {} for {} source(s)", job_id, sources_count)
            }
            ActionOutput::FileDeleteDispatched { job_id, targets_count } => {
                write!(f, "Dispatched file delete job {} for {} target(s)", job_id, targets_count)
            }
            ActionOutput::LocationAdd { location_id, path } => {
                write!(f, "Added location (ID: {}) at path: {}", location_id, path.display())
            }
            ActionOutput::LocationRemove { location_id } => {
                write!(f, "Removed location with ID: {}", location_id)
            }
            ActionOutput::FileIndexDispatched { job_id, location_id } => {
                write!(f, "Dispatched file index job {} for location {}", job_id, location_id)
            }
            ActionOutput::Custom(value) => {
                write!(f, "Custom output: {}", value)
            }
        }
    }
}