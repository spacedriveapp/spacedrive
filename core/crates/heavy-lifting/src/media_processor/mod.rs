use sd_utils::error::FileIOError;

use serde::{Deserialize, Serialize};
use specta::Type;

mod helpers;
mod tasks;

pub use tasks::{
	media_data_extractor::{self, MediaDataExtractor},
	thumbnailer::{self, Thumbnailer},
};

pub use helpers::thumbnailer::{ThumbKey, ThumbnailKind};

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
	#[error(transparent)]
	MediaDataExtractor(#[from] tasks::media_data_extractor::NonCriticalError),
	#[error(transparent)]
	Thumbnailer(#[from] tasks::thumbnailer::NonCriticalError),
}
