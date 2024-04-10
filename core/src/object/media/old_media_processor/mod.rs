use crate::old_job::{JobRunErrors, JobRunMetadata};

use sd_core_file_path_helper::FilePathError;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_prisma::prisma::{location, PrismaClient};

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

use super::{
	exif_data_extractor::{self, ExifDataError, OldExifDataExtractorMetadata},
	old_thumbnail::{self, BatchToProcess, ThumbnailerError},
};

mod job;
mod shallow;

pub use job::OldMediaProcessorJobInit;
pub use shallow::old_shallow;

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
	MediaDataExtractor(#[from] ExifDataError),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OldMediaProcessorMetadata {
	exif_data: OldExifDataExtractorMetadata,
	thumbs_processed: u32,
	labels_extracted: u32,
}

impl From<OldExifDataExtractorMetadata> for OldMediaProcessorMetadata {
	fn from(exif_data: OldExifDataExtractorMetadata) -> Self {
		Self {
			exif_data,
			thumbs_processed: 0,
			labels_extracted: 0,
		}
	}
}

impl JobRunMetadata for OldMediaProcessorMetadata {
	fn update(&mut self, new_data: Self) {
		self.exif_data.extracted += new_data.exif_data.extracted;
		self.exif_data.skipped += new_data.exif_data.skipped;
		self.thumbs_processed += new_data.thumbs_processed;
		self.labels_extracted += new_data.labels_extracted;
	}
}

pub async fn process(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(OldMediaProcessorMetadata, JobRunErrors), MediaProcessorError> {
	// Add here new kinds of media processing if necessary in the future

	exif_data_extractor::process(files_paths, location_id, location_path, db, ctx_update_fn)
		.await
		.map(|(exif_data, errors)| (exif_data.into(), errors))
		.map_err(Into::into)
}
