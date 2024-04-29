use sd_prisma::prisma::file_path;
use sd_utils::error::FileIOError;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;

mod tasks;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	MediaData(#[from] sd_media_metadata::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalError {
	#[error("failed to extract media data from <image='{}'>: {1}", .0.display())]
	FailedToExtractImageMediaData(PathBuf, String),
	#[error("processing thread panicked while extracting media data from <image='{}'>: {1}", .0.display())]
	PanicWhileExtractingImageMediaData(PathBuf, String),
	#[error("file path missing object id: <file_path_id='{0}'>")]
	FilePathMissingObjectId(file_path::id::Type),
	#[error("failed to construct isolated file path data: <file_path_id='{0}'>: {1}")]
	FailedToConstructIsolatedFilePathData(file_path::id::Type, String),
}
