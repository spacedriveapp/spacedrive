use crate::LocationManagerError;
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

	// Internal Errors
	#[error("Failed to create location (uuid {uuid:?})")]
	CreateFailure { uuid: Uuid },
	#[error("Failed to read location metadata file (path: {1:?}); (error: {0:?})")]
	LocationMetadataReadFailure(io::Error, PathBuf),
	#[error("Failed to read location path metadata info (path: {1:?}); (error: {0:?})")]
	LocationPathMetadataAccess(io::Error, PathBuf),
	#[error("Failed to create location metadata hidden directory (path: {1:?}); (error: {0:?})")]
	LocationMetadataDir(io::Error, PathBuf),
	#[error("Failed to serialize dotfile for location (at path: {1:?}); (error: {0:?})")]
	DotfileSerializeFailure(serde_json::Error, PathBuf),
	#[error("Location is read only (at path: {0:?})")]
	ReadonlyLocationFailure(PathBuf),
	#[error("Location metadata file contains a location pub_id that is not in the database: (path: {1:?}); (uuid: {0:?})")]
	LocationMetadataInvalidPubId(Uuid, PathBuf),
	#[error("Failed to write dotfile (path: {1:?}); (error: {0:?})")]
	LocationMetadataWriteFailure(io::Error, PathBuf),
	#[error("Corrupted location metadata file (path: {0:?})")]
	CorruptedLocationMetadataFile(PathBuf),
	#[error("Failed to open file from local os (error: {0:?})")]
	FileReadError(io::Error),
	#[error("Failed to read mounted volumes from local os (error: {0:?})")]
	VolumeReadError(String),
	#[error("Failed to connect to database (error: {0:?})")]
	IOError(io::Error),
	#[error("Database error (error: {0:?})")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("Location manager error (error: {0:?})")]
	LocationManagerError(#[from] LocationManagerError),
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
