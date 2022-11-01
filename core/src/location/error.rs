use rspc::{self, ErrorCode};
use std::path::PathBuf;
use thiserror::Error;
use tokio::io;
use uuid::Uuid;

/// Error type for location related errors
#[derive(Error, Debug)]
pub enum LocationError {
	// Not Found errors
	#[error("Location not found (path: {0:?})")]
	PathNotFound(PathBuf),
	#[error("Location not found (uuid: {0})")]
	UuidNotFound(Uuid),
	#[error("Location not found (id: {0})")]
	IdNotFound(i32),

	// User errors
	#[error("Location not a directory (path: {0:?})")]
	NotDirectory(PathBuf),
	#[error("Missing local_path (id: {0})")]
	MissingLocalPath(i32),
	#[error("Location already exists (path: {0:?})")]
	LocationAlreadyExists(PathBuf),

	// Internal Errors
	#[error("Failed to create location (uuid {uuid:?})")]
	CreateFailure { uuid: Uuid },
	#[error("Failed to read location dotfile (path: {1:?}); (error: {0:?})")]
	DotfileReadFailure(io::Error, PathBuf),
	#[error("Failed to serialize dotfile for location (at path: {1:?}); (error: {0:?})")]
	DotfileSerializeFailure(serde_json::Error, PathBuf),
	#[error("Dotfile location is read only (at path: {0:?})")]
	ReadonlyDotFileLocationFailure(PathBuf),
	#[error("Failed to write dotfile (path: {1:?}); (error: {0:?})")]
	DotfileWriteFailure(io::Error, PathBuf),
	#[error("Failed to open file from local os (error: {0:?})")]
	FileReadError(io::Error),
	#[error("Failed to read mounted volumes from local os (error: {0:?})")]
	VolumeReadError(String),
	#[error("Failed to connect to database (error: {0:?})")]
	IOError(io::Error),
	#[error("Database error (error: {0:?})")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
}

impl From<LocationError> for rspc::Error {
	fn from(err: LocationError) -> Self {
		match err {
			LocationError::PathNotFound(_)
			| LocationError::UuidNotFound(_)
			| LocationError::IdNotFound(_) => {
				rspc::Error::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			LocationError::NotDirectory(_) | LocationError::MissingLocalPath(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}
