use crate::{
	location::{file_path_helper::FilePathError, LocationError},
	util::error::FileIOError,
};

use std::path::Path;

use prisma_client_rust::QueryError;
use thiserror::Error;

/// Error type for file system related jobs errors
#[derive(Error, Debug)]
pub enum FileSystemJobsError {
	#[error(transparent)]
	Location(#[from] LocationError),
	#[error("file_path not in database: <path='{}'>", .0.display())]
	FilePathNotFound(Box<Path>),
	#[error("file_path id not in database: <id='{0}'>")]
	FilePathIdNotFound(i32),
	#[error("failed to create file or folder on disk")]
	CreateFileOrFolder(FileIOError),
	#[error("database error")]
	DatabaseError(#[from] QueryError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),
	#[error("source and destination path are the same: {}", .0.display())]
	MatchingSrcDest(Box<Path>),
	#[error("action would overwrite another file: {}", .0.display())]
	WouldOverwrite(Box<Path>),
}
