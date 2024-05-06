use sd_core_file_path_helper::FilePathError;
use sd_utils::db::MissingFieldError;

use serde::{Deserialize, Serialize};
use specta::Type;

mod helpers;
pub mod job;
mod shallow;
mod tasks;

pub use tasks::{
	media_data_extractor::{self, MediaDataExtractor},
	thumbnailer::{self, Thumbnailer},
};

pub use helpers::thumbnailer::{ThumbKey, ThumbnailKind};

use crate::utils::sub_path;

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
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalError {
	#[error(transparent)]
	MediaDataExtractor(#[from] media_data_extractor::NonCriticalError),
	#[error(transparent)]
	Thumbnailer(#[from] thumbnailer::NonCriticalError),
}
