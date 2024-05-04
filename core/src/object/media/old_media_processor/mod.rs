use crate::old_job::{JobRunErrors, JobRunMetadata};

use sd_core_file_path_helper::FilePathError;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_prisma::prisma::{location, PrismaClient};

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

use super::{
	exif_metadata_extractor::{self, ExifDataError, OldExifDataExtractorMetadata},
	ffmpeg_metadata_extractor::{self, FFmpegDataError, OldFFmpegDataExtractorMetadata},
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
	ExifMediaDataExtractor(#[from] ExifDataError),
	#[error(transparent)]
	FFmpegDataExtractor(#[from] FFmpegDataError),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OldMediaProcessorMetadata {
	exif_data: OldExifDataExtractorMetadata,
	ffmpeg_data: OldFFmpegDataExtractorMetadata,
	thumbs_processed: u32,
	labels_extracted: u32,
}

impl From<OldExifDataExtractorMetadata> for OldMediaProcessorMetadata {
	fn from(exif_data: OldExifDataExtractorMetadata) -> Self {
		Self {
			exif_data,
			ffmpeg_data: Default::default(),
			thumbs_processed: 0,
			labels_extracted: 0,
		}
	}
}

impl From<OldFFmpegDataExtractorMetadata> for OldMediaProcessorMetadata {
	fn from(ffmpeg_data: OldFFmpegDataExtractorMetadata) -> Self {
		Self {
			exif_data: Default::default(),
			ffmpeg_data,
			thumbs_processed: 0,
			labels_extracted: 0,
		}
	}
}

impl JobRunMetadata for OldMediaProcessorMetadata {
	fn update(&mut self, new_data: Self) {
		self.exif_data.extracted += new_data.exif_data.extracted;
		self.exif_data.skipped += new_data.exif_data.skipped;
		self.ffmpeg_data.extracted += new_data.ffmpeg_data.extracted;
		self.ffmpeg_data.skipped += new_data.ffmpeg_data.skipped;
		self.thumbs_processed += new_data.thumbs_processed;
		self.labels_extracted += new_data.labels_extracted;
	}
}

pub async fn process_images(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(OldMediaProcessorMetadata, JobRunErrors), MediaProcessorError> {
	exif_metadata_extractor::process(files_paths, location_id, location_path, db, ctx_update_fn)
		.await
		.map(|(exif_extraction_metadata, errors)| (exif_extraction_metadata.into(), errors))
		.map_err(Into::into)
}

pub async fn process_audio_and_video(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(OldMediaProcessorMetadata, JobRunErrors), MediaProcessorError> {
	ffmpeg_metadata_extractor::process(files_paths, location_id, location_path, db, ctx_update_fn)
		.await
		.map(|(ffmpeg_extraction_metadata, errors)| (ffmpeg_extraction_metadata.into(), errors))
		.map_err(Into::into)
}
