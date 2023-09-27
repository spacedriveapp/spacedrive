use crate::{
	job::{JobRunErrors, JobRunMetadata},
	library::Library,
	location::file_path_helper::{
		file_path_for_media_processor, FilePathError, IsolatedFilePathData,
	},
};

use sd_file_ext::extensions::Extension;
use sd_prisma::prisma::{file_path, location, PrismaClient};

use std::path::Path;

use futures::try_join;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
	media_data_extractor::{self, MediaDataError, MediaDataExtractorMetadata},
	thumbnail::{self, ThumbnailerEntryKind, ThumbnailerError, ThumbnailerMetadata},
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum MediaProcessorEntryKind {
	MediaData,
	Thumbnailer(ThumbnailerEntryKind),
	MediaDataAndThumbnailer(ThumbnailerEntryKind),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MediaProcessorEntry {
	file_path: file_path_for_media_processor::Data,
	operation_kind: MediaProcessorEntryKind,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MediaProcessorMetadata {
	media_data: MediaDataExtractorMetadata,
	thumbnailer: ThumbnailerMetadata,
}

impl JobRunMetadata for MediaProcessorMetadata {
	fn update(&mut self, new_data: Self) {
		self.media_data.extracted += new_data.media_data.extracted;
		self.media_data.skipped += new_data.media_data.skipped;

		self.thumbnailer.created += new_data.thumbnailer.created;
		self.thumbnailer.skipped += new_data.thumbnailer.skipped;
	}
}

async fn get_all_children_files_by_extensions(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	db.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(parent_iso_file_path.location_id())),
			file_path::cas_id::not(None),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::materialized_path::starts_with(
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory"),
			),
		])
		.select(file_path_for_media_processor::select())
		.exec()
		.await
		.map_err(Into::into)
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<Vec<file_path_for_media_processor::Data>, MediaDataError> {
	db.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(parent_iso_file_path.location_id())),
			file_path::cas_id::not(None),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::materialized_path::equals(Some(
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory"),
			)),
		])
		.select(file_path_for_media_processor::select())
		.exec()
		.await
		.map_err(Into::into)
}

async fn process(
	entries: &[MediaProcessorEntry],
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	thumbnails_base_dir: impl AsRef<Path>,
	regenerate_thumbnails: bool,
	library: &Library,
	ctx_update_fn: impl Fn(usize),
) -> Result<(MediaProcessorMetadata, JobRunErrors), MediaProcessorError> {
	let location_path = location_path.as_ref();

	let ((media_data_metadata, mut media_data_errors), (thumbnailer_metadata, thumbnailer_errors)) =
		try_join!(
			async {
				media_data_extractor::process(
					entries.iter().filter_map(
						|MediaProcessorEntry {
						     file_path,
						     operation_kind,
						 }| {
							matches!(
								operation_kind,
								MediaProcessorEntryKind::MediaDataAndThumbnailer(_)
									| MediaProcessorEntryKind::MediaData
							)
							.then_some(file_path)
						},
					),
					location_id,
					location_path,
					&library.db,
				)
				.await
				.map_err(MediaProcessorError::from)
			},
			async {
				thumbnail::process(
					entries.iter().filter_map(
						|MediaProcessorEntry {
						     file_path,
						     operation_kind,
						 }| {
							if let MediaProcessorEntryKind::Thumbnailer(thumb_kind)
							| MediaProcessorEntryKind::MediaDataAndThumbnailer(thumb_kind) = operation_kind
							{
								Some((file_path, *thumb_kind))
							} else {
								None
							}
						},
					),
					location_id,
					location_path,
					thumbnails_base_dir,
					regenerate_thumbnails,
					library,
					ctx_update_fn,
				)
				.await
				.map_err(MediaProcessorError::from)
			},
		)?;

	media_data_errors.0.extend(thumbnailer_errors.0.into_iter());

	Ok((
		MediaProcessorMetadata {
			media_data: media_data_metadata,
			thumbnailer: thumbnailer_metadata,
		},
		media_data_errors,
	))
}
