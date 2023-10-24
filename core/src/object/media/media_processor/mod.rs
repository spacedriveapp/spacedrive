use crate::{
	job::{JobRunErrors, JobRunMetadata},
	location::file_path_helper::{file_path_for_media_processor, FilePathError},
};

use sd_prisma::prisma::{location, PrismaClient};

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

use super::{
	media_data_extractor::{self, MediaDataError, MediaDataExtractorMetadata},
	thumbnail::{self, BatchToProcess, ThumbnailerError},
};

mod job;
mod shallow;

pub use job::MediaProcessorJobInit;
pub use shallow::shallow;

#[derive(Error, Debug)]
pub enum MediaProcessorError {
	#[error("sub path not found: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),

	#[error(transparent)]
	Thumbnailer(#[from] ThumbnailerError),
	#[error(transparent)]
	MediaDataExtractor(#[from] MediaDataError),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MediaProcessorMetadata {
	media_data: MediaDataExtractorMetadata,
	thumbs_processed: u32,
}

impl From<MediaDataExtractorMetadata> for MediaProcessorMetadata {
	fn from(media_data: MediaDataExtractorMetadata) -> Self {
		Self {
			media_data,
			thumbs_processed: 0,
		}
	}
}

impl JobRunMetadata for MediaProcessorMetadata {
	fn update(&mut self, new_data: Self) {
		self.media_data.extracted += new_data.media_data.extracted;
		self.media_data.skipped += new_data.media_data.skipped;
		self.thumbs_processed += new_data.thumbs_processed;
	}
}

pub async fn process(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(MediaProcessorMetadata, JobRunErrors), MediaProcessorError> {
	// Add here new kinds of media processing if necessary in the future

	media_data_extractor::process(files_paths, location_id, location_path, db, ctx_update_fn)
		.await
		.map(|(media_data, errors)| (media_data.into(), errors))
		.map_err(Into::into)
}
