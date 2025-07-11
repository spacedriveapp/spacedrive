//! Message types for all CLI outputs

use super::VerbosityLevel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// All possible output messages in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    // Generic messages
    Success(String),
    Error(String),
    Warning(String),
    Info(String),
    Debug(String),

    // Library messages
    LibraryCreated {
        name: String,
        id: Uuid,
        path: PathBuf,
    },
    LibraryDeleted {
        id: Uuid,
    },
    LibrarySwitched {
        name: String,
        id: Uuid,
    },
    LibraryList {
        libraries: Vec<LibraryInfo>,
    },
    CurrentLibrary {
        library: Option<LibraryInfo>,
    },
    NoLibrariesFound,

    // Location messages
    LocationAdded {
        path: PathBuf,
        id: Uuid,
    },
    LocationRemoved {
        path: PathBuf,
    },
    LocationList {
        locations: Vec<LocationInfo>,
    },
    LocationIndexing {
        path: PathBuf,
        progress: f32,
    },

    // Daemon messages
    DaemonStarting {
        instance: String,
    },
    DaemonStarted {
        instance: String,
        pid: u32,
        socket_path: PathBuf,
    },
    DaemonStopping {
        instance: String,
    },
    DaemonStopped {
        instance: String,
    },
    DaemonNotRunning {
        instance: String,
    },
    DaemonStatus {
        version: String,
        uptime: u64,
        instance: String,
        networking_enabled: bool,
        libraries: Vec<LibraryInfo>,
    },

    // Network messages
    NetworkingInitialized,
    NetworkingStarted,
    NetworkingStopped,
    DeviceDiscovered {
        device: DeviceInfo,
    },
    DevicesList {
        devices: Vec<DeviceInfo>,
    },
    PairingCodeGenerated {
        code: String,
    },
    PairingInProgress {
        device_name: String,
    },
    PairingSuccess {
        device_name: String,
        device_id: String,
    },
    PairingFailed {
        reason: String,
    },
    PairingStatus {
        status: String,
        pending_requests: Vec<PairingRequest>,
    },
    SpacedropSent {
        file_name: String,
        device_name: String,
    },
    SpacedropReceived {
        file_name: String,
        sender_name: String,
    },

    // Job messages
    JobStarted {
        id: Uuid,
        name: String,
    },
    JobProgress {
        id: Uuid,
        name: String,
        progress: f32,
        message: Option<String>,
    },
    JobCompleted {
        id: Uuid,
        name: String,
        duration: u64,
    },
    JobFailed {
        id: Uuid,
        name: String,
        error: String,
    },
    JobList {
        jobs: Vec<JobInfo>,
    },

    // File operation messages
    FileCopied {
        source: PathBuf,
        destination: PathBuf,
    },
    FileDeleted {
        path: PathBuf,
    },
    FileRenamed {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    FileValidated {
        path: PathBuf,
        hash: String,
    },

    // System messages
    SystemInfo {
        version: String,
        platform: String,
        data_dir: PathBuf,
    },
    LogsShowing {
        path: PathBuf,
    },

    // Progress messages
    IndexingProgress {
        current: u64,
        total: u64,
        location: String,
    },
    CopyProgress {
        current: u64,
        total: u64,
        current_file: Option<String>,
    },
    ValidationProgress {
        current: u64,
        total: u64,
    },

    // Help messages
    HelpText {
        lines: Vec<String>,
    },
}

impl Message {
    /// Determine if this is an error message
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Message::Error(_)
                | Message::DaemonNotRunning { .. }
                | Message::PairingFailed { .. }
                | Message::JobFailed { .. }
        )
    }

    /// Get the verbosity level for this message
    pub fn verbosity_level(&self) -> VerbosityLevel {
        match self {
            Message::Debug(_) => VerbosityLevel::Debug,
            Message::IndexingProgress { .. }
            | Message::CopyProgress { .. }
            | Message::ValidationProgress { .. } => VerbosityLevel::Verbose,
            _ => VerbosityLevel::Normal,
        }
    }
}

// Helper structs for complex messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
    pub id: Uuid,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    pub id: Uuid,
    pub path: PathBuf,
    pub indexed_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub status: DeviceStatus,
    pub peer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceStatus {
    Online,
    Offline,
    Paired,
    Discovered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub id: String,
    pub device_name: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: Uuid,
    pub name: String,
    pub status: JobStatus,
    pub progress: Option<f32>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Paused,
}

