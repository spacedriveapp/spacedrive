use crate::LocationManagerError;

use std::path::PathBuf;

use rspc::{self, ErrorCode};
use thiserror::Error;
use tokio::io;
use uuid::Uuid;

use super::{file_path_helper::FilePathError, metadata::LocationMetadataError};

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
	#[error("Could not find directory in Location (path: {0:?})")]
	DirectoryNotFound(String),
	#[error("Library exists in the location metadata file, must relink: (old_path: {old_path:?}, new_path: {new_path:?})")]
	NeedRelink {
		old_path: PathBuf,
		new_path: PathBuf,
	},
	#[error("Exist a different library in the location metadata file, must add a new library: (path: {0:?})")]
	AddLibraryToMetadata(PathBuf),
	#[error("Location metadata file not found: (path: {0:?})")]
	MetadataNotFound(PathBuf),
	#[error("Location already exists (path: {0:?})")]
	LocationAlreadyExists(PathBuf),

	// Internal Errors
	#[error("Location metadata error (error: {0:?})")]
	LocationMetadataError(#[from] LocationMetadataError),
	#[error("Failed to read location path metadata info (path: {1:?}); (error: {0:?})")]
	LocationPathFilesystemMetadataAccess(io::Error, PathBuf),
	#[error("Location is read only (at path: {0:?})")]
	ReadonlyLocationFailure(PathBuf),
	#[error("Missing metadata file for location (path: {0:?})")]
	MissingMetadataFile(PathBuf),
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
	#[error("File path related error (error: {0})")]
	FilePathError(#[from] FilePathError),
}

impl From<LocationError> for rspc::Error {
	fn from(err: LocationError) -> Self {
		match err {
			// Not found errors
			LocationError::PathNotFound(_)
			| LocationError::UuidNotFound(_)
			| LocationError::IdNotFound(_) => {
				rspc::Error::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			// User's fault errors
			LocationError::NotDirectory(_)
			// | LocationError::MissingLocalPath(_)
			| LocationError::NeedRelink { .. }
			| LocationError::AddLibraryToMetadata(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}
