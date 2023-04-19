use std::path::PathBuf;

use rspc::{self, ErrorCode};
use thiserror::Error;
use tokio::io;
use uuid::Uuid;

use super::{
	file_path_helper::FilePathError, manager::LocationManagerError, metadata::LocationMetadataError,
};

/// Error type for location related errors
#[derive(Error, Debug)]
pub enum LocationError {
	// Not Found errors
	#[error("Location not found (path: {})", .0.display())]
	PathNotFound(PathBuf),
	#[error("Location not found (uuid: {0})")]
	UuidNotFound(Uuid),
	#[error("Location not found (id: {0})")]
	IdNotFound(i32),

	// User errors
	#[error("Location not a directory (path: {})", .0.display())]
	NotDirectory(PathBuf),
	#[error("Could not find directory in Location (path: {})", .0.display())]
	DirectoryNotFound(PathBuf),
	#[error(
		"Library exists in the location metadata file, must relink: (old_path: {}, new_path: {})",
		.old_path.display(),
		.new_path.display(),
	)]
	NeedRelink {
		old_path: PathBuf,
		new_path: PathBuf,
	},
	#[error(
		"This location belongs to another library, must update .spacedrive file: (path: {})",
		.0.display()
	)]
	AddLibraryToMetadata(PathBuf),
	#[error("Location metadata file not found: (path: {})", .0.display())]
	MetadataNotFound(PathBuf),
	#[error("Location already exists in database (path: {})", .0.display())]
	LocationAlreadyExists(PathBuf),
	#[error("Nested location currently not supported (path: {})", .0.display())]
	NestedLocation(PathBuf),

	// Internal Errors
	#[error("Location metadata error (error: {0:?})")]
	LocationMetadataError(#[from] LocationMetadataError),
	#[error("Failed to read location path metadata info (path: {}); (error: {0:?})", .1.display())]
	LocationPathFilesystemMetadataAccess(io::Error, PathBuf),
	#[error("Missing metadata file for location (path: {})", .0.display())]
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
			| LocationError::NestedLocation(_)
			| LocationError::LocationAlreadyExists(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			// Custom error message is used to differenciate these errors in the frontend
			// TODO: A better solution would be for rspc to support sending custom data alongside errors
			LocationError::NeedRelink { .. } => {
				rspc::Error::with_cause(ErrorCode::Conflict, "NEED_RELINK".to_owned(), err)
			}
			LocationError::AddLibraryToMetadata(_) => {
				rspc::Error::with_cause(ErrorCode::Conflict, "ADD_LIBRARY".to_owned(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}
