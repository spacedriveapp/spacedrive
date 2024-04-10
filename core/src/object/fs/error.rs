use crate::location::LocationError;

use sd_core_file_path_helper::FilePathError;

use sd_prisma::prisma::file_path;
use sd_utils::{
	db::MissingFieldError,
	error::{FileIOError, NonUtf8PathError},
};

use std::path::Path;

use prisma_client_rust::QueryError;
use thiserror::Error;

/// Error type for file system related jobs errors
#[derive(Error, Debug)]
pub enum FileSystemJobsError {
	#[error("Location error: {0}")]
	Location(#[from] LocationError),
	#[error("file_path not in database: <path='{}'>", .0.display())]
	FilePathNotFound(Box<Path>),
	#[error("file_path id not in database: <id='{0}'>")]
	FilePathIdNotFound(file_path::id::Type),
	#[error("failed to create file or folder on disk")]
	CreateFileOrFolder(FileIOError),
	#[error("database error: {0}")]
	Database(#[from] QueryError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),
	#[error("action would overwrite another file: {}", .0.display())]
	WouldOverwrite(Box<Path>),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("no parent for path, which is supposed to be directory: <path='{}'>", .0.display())]
	MissingParentPath(Box<Path>),
	#[error("no stem on file path, but it's supposed to be a file: <path='{}'>", .0.display())]
	MissingFileStem(Box<Path>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUTF8Path(#[from] NonUtf8PathError),
	#[error("failed to find an available name to avoid duplication: <path='{}'>", .0.display())]
	FailedToFindAvailableName(Box<Path>),
}

impl From<FileSystemJobsError> for rspc::Error {
	fn from(e: FileSystemJobsError) -> Self {
		Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}
