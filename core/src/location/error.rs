use crate::util::error::FileIOError;

use std::path::PathBuf;

use rspc::{self, ErrorCode};
use thiserror::Error;
use uuid::Uuid;

use super::{
	file_path_helper::FilePathError, manager::LocationManagerError, metadata::LocationMetadataError,
};

/// Error type for location related errors
#[derive(Error, Debug)]
pub enum LocationError {
	// Not Found errors
	#[error("location not found <path='{}'>", .0.display())]
	PathNotFound(PathBuf),
	#[error("location not found <uuid='{0}'>")]
	UuidNotFound(Uuid),
	#[error("location not found <id='{0}'>")]
	IdNotFound(i32),

	// User errors
	#[error("location not a directory <path='{}'>", .0.display())]
	NotDirectory(PathBuf),
	#[error("could not find directory in location <path='{}'>", .0.display())]
	DirectoryNotFound(PathBuf),
	#[error(
		"library exists in the location metadata file, must relink <old_path='{}', new_path='{}'>",
		.old_path.display(),
		.new_path.display(),
	)]
	NeedRelink {
		old_path: PathBuf,
		new_path: PathBuf,
	},
	#[error(
		"this location belongs to another library, must update .spacedrive file <path='{}'>",
		.0.display()
	)]
	AddLibraryToMetadata(PathBuf),
	#[error("location metadata file not found <path='{}'>", .0.display())]
	MetadataNotFound(PathBuf),
	#[error("location already exists in database <path='{}'>", .0.display())]
	LocationAlreadyExists(PathBuf),
	#[error("nested location currently not supported <path='{}'>", .0.display())]
	NestedLocation(PathBuf),

	// Internal Errors
	#[error(transparent)]
	LocationMetadataError(#[from] LocationMetadataError),
	#[error("failed to read location path metadata info")]
	LocationPathFilesystemMetadataAccess(FileIOError),
	#[error("missing metadata file for location <path='{}'>", .0.display())]
	MissingMetadataFile(PathBuf),
	#[error("failed to open file from local OS")]
	FileReadError(FileIOError),
	#[error("failed to read mounted volumes from local OS")]
	VolumeReadError(String),
	#[error("database error")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	LocationManagerError(#[from] LocationManagerError),
	#[error(transparent)]
	FilePathError(#[from] FilePathError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
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
