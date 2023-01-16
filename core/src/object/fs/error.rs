use std::path::PathBuf;

use thiserror::Error;

use crate::location::LocationError;

/// Error type for location related errors
#[derive(Error, Debug)]
pub enum VirtualFSError {
	#[error("Location error")]
	LocationError(#[from] LocationError),
	#[error("Failed to create file or folder on disk at path (path: {0:?})")]
	CreateFileOrFolder(#[from] std::io::Error),
	#[error("Database error (error: {0:?})")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
}
