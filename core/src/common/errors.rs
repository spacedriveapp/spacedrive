//! Unified error handling for the core

use thiserror::Error;

/// Main error type for core operations
#[derive(Error, Debug)]
pub enum CoreError {
	#[error("Database error: {0}")]
	Database(#[from] sea_orm::DbErr),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("File operation error: {0}")]
	FileOp(#[from] FileOpError),

	#[error("Not found: {0}")]
	NotFound(String),

	#[error("Invalid operation: {0}")]
	InvalidOperation(String),

	#[error("Other error: {0}")]
	Other(#[from] anyhow::Error),
}

/// Errors specific to file operations
#[derive(Error, Debug)]
pub enum FileOpError {
	#[error("Source not found: {0}")]
	SourceNotFound(String),

	#[error("Destination not found: {0}")]
	DestinationNotFound(String),

	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	#[error("File exists: {0}")]
	FileExists(String),

	#[error("Not a directory: {0}")]
	NotADirectory(String),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Other: {0}")]
	Other(String),
}

impl From<&str> for FileOpError {
	fn from(s: &str) -> Self {
		FileOpError::Other(s.to_string())
	}
}

impl From<String> for FileOpError {
	fn from(s: String) -> Self {
		FileOpError::Other(s)
	}
}

/// Result type alias for core operations
pub type Result<T> = std::result::Result<T, CoreError>;
