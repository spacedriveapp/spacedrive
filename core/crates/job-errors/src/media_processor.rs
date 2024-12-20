use crate::sub_path;
use sd_core_file_helper::FilePathError;
use sd_prisma::prisma::file_path;
use sd_task_system::TaskId;
use sd_utils::db::MissingFieldError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("missing field on database: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("failed to deserialized stored tasks for job resume: {0}")]
	DeserializeTasks(#[from] rmp_serde::decode::Error),

	#[error(transparent)]
	FilePathError(#[from] FilePathError),
	#[error(transparent)]
	SubPath(#[from] sub_path::Error),
	#[error(transparent)]
	Sync(#[from] sd_core_library_sync::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::SubPath(sub_path_err) => sub_path_err.into(),

			_ => Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NonCriticalMediaProcessorError {
	#[error(transparent)]
	MediaDataExtractor(#[from] NonCriticalMediaDataExtractorError),
	#[error(transparent)]
	Thumbnailer(#[from] NonCriticalThumbnailerError),
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
pub enum NonCriticalMediaDataExtractorError {
	#[error("failed to extract media data from <file='{}'>: {1}", .0.display())]
	FailedToExtractImageMediaData(PathBuf, String),
	#[error("file path missing object id: <file_path_id='{0}'>")]
	FilePathMissingObjectId(file_path::id::Type),
	#[error("failed to construct isolated file path data: <file_path_id='{0}'>: {1}")]
	FailedToConstructIsolatedFilePathData(file_path::id::Type, String),
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
pub enum NonCriticalThumbnailerError {
	#[error("file path <id='{0}'> has no cas_id")]
	MissingCasId(file_path::id::Type),
	#[error("failed to extract isolated file path data from file path <id='{0}'>: {1}")]
	FailedToExtractIsolatedFilePathData(file_path::id::Type, String),
	#[error("failed to generate video file thumbnail <path='{}'>: {1}", .0.display())]
	VideoThumbnailGenerationFailed(PathBuf, String),
	#[error("failed to format image <path='{}'>: {1}", .0.display())]
	FormatImage(PathBuf, String),
	#[error("failed to encode webp image <path='{}'>: {1}", .0.display())]
	WebPEncoding(PathBuf, String),
	#[error("processing thread panicked while generating thumbnail from <path='{}'>: {1}", .0.display())]
	PanicWhileGeneratingThumbnail(PathBuf, String),
	#[error("failed to create shard directory for thumbnail: {0}")]
	CreateShardDirectory(String),
	#[error("failed to save thumbnail <path='{}'>: {1}", .0.display())]
	SaveThumbnail(PathBuf, String),
	#[error("task timed out: {0}")]
	TaskTimeout(TaskId),
}
