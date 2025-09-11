//! Volume-related error types

use thiserror::Error;

/// Errors that can occur during volume operations
#[derive(Error, Debug)]
pub enum VolumeError {
    /// IO error during volume operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Platform-specific error
    #[error("Platform error: {0}")]
    Platform(String),

    /// Volume not found
    #[error("Volume not found: {0}")]
    NotFound(String),

    /// Volume is not mounted
    #[error("Volume is not mounted: {0}")]
    NotMounted(String),

    /// Volume is read-only
    #[error("Volume is read-only: {0}")]
    ReadOnly(String),

    /// Insufficient space on volume
    #[error("Insufficient space on volume: required {required}, available {available}")]
    InsufficientSpace { required: u64, available: u64 },

    /// Speed test was cancelled or failed
    #[error("Speed test cancelled or failed")]
    SpeedTestFailed,

    /// Volume detection failed
    #[error("Volume detection failed: {0}")]
    DetectionFailed(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Operation timed out
    #[error("Operation timed out")]
    Timeout,

    /// Invalid volume data
    #[error("Invalid volume data: {0}")]
    InvalidData(String),

    /// Database operation failed
    #[error("Database error: {0}")]
    Database(String),

    /// Volume is already tracked
    #[error("Volume is already tracked: {0}")]
    AlreadyTracked(String),

    /// Volume is not tracked
    #[error("Volume is not tracked: {0}")]
    NotTracked(String),
}

impl VolumeError {
    /// Create a platform-specific error
    pub fn platform(msg: impl Into<String>) -> Self {
        Self::Platform(msg.into())
    }

    /// Create a detection failed error
    pub fn detection_failed(msg: impl Into<String>) -> Self {
        Self::DetectionFailed(msg.into())
    }

    /// Create an insufficient space error
    pub fn insufficient_space(required: u64, available: u64) -> Self {
        Self::InsufficientSpace { required, available }
    }
}

/// Result type for volume operations
pub type VolumeResult<T> = Result<T, VolumeError>;