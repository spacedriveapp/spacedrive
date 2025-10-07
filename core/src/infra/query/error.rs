//! Error types for the Query System

use crate::{common::errors::CoreError, library::LibraryError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Result type for query operations
pub type QueryResult<T> = Result<T, QueryError>;

/// Errors that can occur during query execution
#[derive(Debug, Error)]
pub enum QueryError {
	/// Query type not registered in the registry
	#[error("Query type '{0}' is not registered")]
	QueryNotRegistered(String),

	/// Invalid query type for the handler
	#[error("Invalid query type for this handler")]
	InvalidQueryType,

	/// Invalid input provided to query
	#[error("Invalid input: {0}")]
	InvalidInput(String),

	/// Permission denied for this query
	#[error("Permission denied for query '{query}': {reason}")]
	PermissionDenied { query: String, reason: String },

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
	FileSystem { path: String, error: String },

	/// Network error for cross-device operations
	#[error("Network error with device {device_id}: {error}")]
	Network { device_id: Uuid, error: String },

	/// Database operation error
	#[error("Database error: {0}")]
	Database(String),

	/// Validation error
	#[error("Validation error for field '{field}': {message}")]
	Validation { field: String, message: String },

	/// Query execution timeout
	#[error("Query execution timed out")]
	Timeout,

	/// Query was cancelled
	#[error("Query was cancelled")]
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

	/// Cache error
	#[error("Cache error: {0}")]
	Cache(String),

	/// Generic internal error
	#[error("Internal error: {0}")]
	Internal(String),
}

impl From<LibraryError> for QueryError {
	fn from(error: LibraryError) -> Self {
		match error {
			LibraryError::NotFound(_) => QueryError::Internal(error.to_string()),
			other => QueryError::Internal(other.to_string()),
		}
	}
}

impl From<CoreError> for QueryError {
	fn from(error: CoreError) -> Self {
		QueryError::Internal(error.to_string())
	}
}

impl From<std::io::Error> for QueryError {
	fn from(error: std::io::Error) -> Self {
		QueryError::Io {
			path: "unknown".to_string(),
			source: error,
		}
	}
}

impl From<anyhow::Error> for QueryError {
	fn from(error: anyhow::Error) -> Self {
		QueryError::Internal(error.to_string())
	}
}

/// Helper function to create IO errors with known paths
impl QueryError {
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
