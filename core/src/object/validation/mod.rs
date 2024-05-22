use sd_core_file_path_helper::FilePathError;
use sd_utils::error::FileIOError;

use std::path::Path;

use thiserror::Error;

pub mod hash;
pub mod old_validator_job;

#[derive(Error, Debug)]
pub enum ValidatorError {
	#[error("sub path not found: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
}
