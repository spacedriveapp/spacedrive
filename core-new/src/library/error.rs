//! Library-specific error types

use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Library operation errors
#[derive(Error, Debug)]
pub enum LibraryError {
    /// Library is already open
    #[error("Library {0} is already open")]
    AlreadyOpen(Uuid),
    
    /// Library is already in use by another process
    #[error("Library is already in use by another process")]
    AlreadyInUse,
    
    /// Stale lock file detected
    #[error("Stale lock file detected - library may have crashed previously")]
    StaleLock,
    
    /// Not a valid library directory
    #[error("Not a valid library directory: {0}")]
    NotALibrary(PathBuf),
    
    /// Library not found
    #[error("Library not found: {0}")]
    NotFound(String),
    
    /// Invalid library name
    #[error("Invalid library name: {0}")]
    InvalidName(String),
    
    /// Library already exists
    #[error("Library already exists at: {0}")]
    AlreadyExists(PathBuf),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),
    
    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    /// JSON error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// Job system error
    #[error("Job system error: {0}")]
    JobError(#[from] crate::infrastructure::jobs::error::JobError),
    
    /// Generic error
    #[error("{0}")]
    Other(String),
}

/// Result type for library operations
pub type Result<T> = std::result::Result<T, LibraryError>;