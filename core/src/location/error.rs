use sd_core_file_path_helper::FilePathError;

use sd_prisma::prisma::location;
use sd_utils::{
	db::MissingFieldError,
	error::{FileIOError, NonUtf8PathError},
};

use std::path::Path;

use rspc::ErrorCode;
use thiserror::Error;
use uuid::Uuid;

use super::{manager::LocationManagerError, metadata::LocationMetadataError};

/// Error type for location related errors
#[derive(Error, Debug)]
pub enum LocationError {
	// Not Found errors
	#[error("location not found <path='{}'>", .0.display())]
	PathNotFound(Box<Path>),
	#[error("location not found <uuid='{0}'>")]
	UuidNotFound(Uuid),
	#[error("location not found <id='{0}'>")]
	IdNotFound(location::id::Type),

	// User errors
	#[error("location not a directory <path='{}'>", .0.display())]
	NotDirectory(Box<Path>),
	#[error("could not find directory in location <path='{}'>", .0.display())]
	DirectoryNotFound(Box<Path>),
	#[error(
		"library exists in the location metadata file, must relink <old_path='{}', new_path='{}'>",
		.old_path.display(),
		.new_path.display(),
	)]
	NeedRelink {
		old_path: Box<Path>,
		new_path: Box<Path>,
	},
	#[error(
		"this location belongs to another library, must update .spacedrive file <path='{}'>",
		.0.display()
	)]
	AddLibraryToMetadata(Box<Path>),
	#[error("location metadata file not found <path='{}'>", .0.display())]
	MetadataNotFound(Box<Path>),
	#[error("location already exists in database <path='{}'>", .0.display())]
	LocationAlreadyExists(Box<Path>),
	#[error("nested location currently not supported <path='{}'>", .0.display())]
	NestedLocation(Box<Path>),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),

	// Internal Errors
	#[error(transparent)]
	LocationMetadata(#[from] LocationMetadataError),
	#[error("failed to read location path metadata info: {0}")]
	LocationPathFilesystemMetadataAccess(FileIOError),
	#[error("missing metadata file for location <path='{}'>", .0.display())]
	MissingMetadataFile(Box<Path>),
	#[error("failed to open file from local OS: {0}")]
	FileRead(FileIOError),
	#[error("failed to read mounted volumes from local OS: {0}")]
	VolumeReadError(String),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	LocationManager(#[from] LocationManagerError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error("location missing path <id='{0}'>")]
	MissingPath(location::id::Type),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("invalid location scan state value: {0}")]
	InvalidScanStateValue(i32),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),
	#[error("other error: {0}")]
	Other(String),
}

impl From<LocationError> for rspc::Error {
	fn from(e: LocationError) -> Self {
		use LocationError::*;

		match e {
			// Not found errors
			PathNotFound(_)
			| UuidNotFound(_)
			| IdNotFound(_)
			| FilePath(FilePathError::IdNotFound(_) | FilePathError::NotFound(_)) => {
				Self::with_cause(ErrorCode::NotFound, e.to_string(), e)
			}

			// User's fault errors
			NotDirectory(_) | NestedLocation(_) | LocationAlreadyExists(_) => {
				Self::with_cause(ErrorCode::BadRequest, e.to_string(), e)
			}

			// Custom error message is used to differentiate these errors in the frontend
			// TODO: A better solution would be for rspc to support sending custom data alongside errors
			NeedRelink { .. } => Self::with_cause(ErrorCode::Conflict, "NEED_RELINK".to_owned(), e),
			AddLibraryToMetadata(_) => {
				Self::with_cause(ErrorCode::Conflict, "ADD_LIBRARY".to_owned(), e)
			}

			// Internal errors
			MissingField(missing_error) => missing_error.into(),
			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}
