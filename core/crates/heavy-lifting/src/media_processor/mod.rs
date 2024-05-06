use crate::{utils::sub_path, OuterContext, UpdateEvent};

use sd_core_file_path_helper::FilePathError;

use sd_utils::db::MissingFieldError;

use std::fmt;

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
pub use shallow::shallow;

use self::thumbnailer::NewThumbnailReporter;

const BATCH_SIZE: usize = 10;

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

struct NewThumbnailsReporter<Ctx: OuterContext> {
	ctx: Ctx,
}

impl<Ctx: OuterContext> fmt::Debug for NewThumbnailsReporter<Ctx> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NewThumbnailsReporter").finish()
	}
}

impl<Ctx: OuterContext> NewThumbnailReporter for NewThumbnailsReporter<Ctx> {
	fn new_thumbnail(&self, thumb_key: ThumbKey) {
		self.ctx
			.report_update(UpdateEvent::NewThumbnailEvent { thumb_key });
	}
}
