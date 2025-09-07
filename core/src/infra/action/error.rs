//! Error types for the Action System

use crate::{
    infra::jobs::error::JobError,
    library::LibraryError,
    common::errors::CoreError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Result type for action operations
pub type ActionResult<T> = Result<T, ActionError>;

/// Errors that can occur during action execution
#[derive(Debug, Error)]
pub enum ActionError {
    /// Action type not registered in the registry
    #[error("Action type '{0}' is not registered")]
    ActionNotRegistered(String),

    /// Invalid action type for the handler
    #[error("Invalid action type for this handler")]
    InvalidActionType,

    /// Invalid input provided to action
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Permission denied for this action
    #[error("Permission denied for action '{action}': {reason}")]
    PermissionDenied {
        action: String,
        reason: String,
    },

    /// Library not found
    #[error("Library {0} not found")]
    LibraryNotFound(Uuid),

    /// Location not found
    #[error("Location {0} not found")]
    LocationNotFound(Uuid),

    /// Device not found
    #[error("Device {0} not found")]
    DeviceNotFound(Uuid),

    /// File system error
    #[error("File system error at '{path}': {error}")]
    FileSystem {
        path: String,
        error: String,
    },

    /// Network error for cross-device operations
    #[error("Network error with device {device_id}: {error}")]
    Network {
        device_id: Uuid,
        error: String,
    },

    /// Job creation or execution error
    #[error("Job error: {0}")]
    Job(#[from] JobError),

    /// Database operation error
    #[error("Database error: {0}")]
    Database(String),

    /// Validation error
    #[error("Validation error for field '{field}': {message}")]
    Validation {
        field: String,
        message: String,
    },

    /// Action execution timeout
    #[error("Action execution timed out")]
    Timeout,

    /// Action was cancelled
    #[error("Action was cancelled")]
    Cancelled,

    /// Device manager error
    #[error("Device manager error: {0}")]
    DeviceManager(String),

    /// JSON serialization error
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    /// Sea-ORM database error
    #[error("Database operation failed: {0}")]
    SeaOrm(#[from] sea_orm::DbErr),

    /// IO error
    #[error("IO error at '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<LibraryError> for ActionError {
    fn from(error: LibraryError) -> Self {
        match error {
            LibraryError::NotFound(_) => ActionError::Internal(error.to_string()),
            other => ActionError::Internal(other.to_string()),
        }
    }
}

impl From<CoreError> for ActionError {
    fn from(error: CoreError) -> Self {
        ActionError::Internal(error.to_string())
    }
}

impl From<std::io::Error> for ActionError {
    fn from(error: std::io::Error) -> Self {
        ActionError::Io {
            path: "unknown".to_string(),
            source: error,
        }
    }
}

/// Helper function to create IO errors with known paths
impl ActionError {
    pub fn io_error(path: impl Into<String>, error: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source: error,
        }
    }

    pub fn device_manager_error(error: impl std::fmt::Display) -> Self {
        Self::DeviceManager(error.to_string())
    }
}