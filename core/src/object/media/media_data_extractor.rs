use crate::{
	job::JobRunErrors,
	location::file_path_helper::{file_path_for_media_processor, IsolatedFilePathData},
	prisma::{location, media_data, PrismaClient},
	util::error::FileIOError,
};

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ImageMetadata;

use std::{collections::HashSet, path::Path};

use futures_concurrency::future::Join;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::spawn_blocking;
use tracing::error;

use super::media_data_image_to_query;

#[derive(Error, Debug)]
pub enum MediaDataError {
	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	MediaData(#[from] sd_media_metadata::Error),
	#[error("failed to join tokio task: {0}")]
	TokioJoinHandle(#[from] tokio::task::JoinError),
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MediaDataExtractorMetadata {
	pub extracted: u32,
	pub skipped: u32,
}

pub(super) static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_extract_media_data_for_image)
		.map(Extension::Image)
		.collect()
});

pub const fn can_extract_media_data_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::*;
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
}

pub async fn extract_media_data(path: impl AsRef<Path>) -> Result<ImageMetadata, MediaDataError> {
	let path = path.as_ref().to_path_buf();

	// Running in a separated blocking thread due to MediaData blocking behavior (due to sync exif lib)
	spawn_blocking(|| ImageMetadata::from_path(path))
		.await?
		.map_err(Into::into)
}

pub async fn process(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(MediaDataExtractorMetadata, JobRunErrors), MediaDataError> {
	let mut run_metadata = MediaDataExtractorMetadata::default();
	if files_paths.is_empty() {
		return Ok((run_metadata, JobRunErrors::default()));
	}

	let location_path = location_path.as_ref();

	let objects_already_with_media_data = db
		.media_data()
		.find_many(vec![media_data::object_id::in_vec(
			files_paths
				.iter()
				.filter_map(|file_path| file_path.object_id)
				.collect(),
		)])
		.select(media_data::select!({ object_id }))
		.exec()
		.await?;

	if files_paths.len() == objects_already_with_media_data.len() {
		// All files already have media data, skipping
		run_metadata.skipped = files_paths.len() as u32;
		return Ok((run_metadata, JobRunErrors::default()));
	}

	let objects_already_with_media_data = objects_already_with_media_data
		.into_iter()
		.map(|media_data| media_data.object_id)
		.collect::<HashSet<_>>();

	run_metadata.skipped = objects_already_with_media_data.len() as u32;

	let (media_datas, errors) = {
		let maybe_media_data = files_paths
			.iter()
			.enumerate()
			.filter_map(|(idx, file_path)| {
				file_path.object_id.and_then(|object_id| {
					(!objects_already_with_media_data.contains(&object_id))
						.then_some((idx, file_path, object_id))
				})
			})
			.filter_map(|(idx, file_path, object_id)| {
				IsolatedFilePathData::try_from((location_id, file_path))
					.map_err(|e| error!("{e:#?}"))
					.ok()
					.map(|iso_file_path| (idx, location_path.join(iso_file_path), object_id))
			})
			.map(|(idx, path, object_id)| async move {
				let res = extract_media_data(&path).await;
				ctx_update_fn(idx + 1);
				(res, path, object_id)
			})
			.collect::<Vec<_>>()
			.join()
			.await;

		let total_media_data = maybe_media_data.len();

		maybe_media_data.into_iter().fold(
			// In the good case, all media data were extracted
			(Vec::with_capacity(total_media_data), Vec::new()),
			|(mut media_datas, mut errors), (maybe_media_data, path, object_id)| {
				match maybe_media_data {
					Ok(media_data) => media_datas.push((media_data, object_id)),
					Err(MediaDataError::MediaData(sd_media_metadata::Error::NoExifDataOnPath(
						_,
					))) => {
						// No exif data on path, skipping
						run_metadata.skipped += 1;
					}
					Err(e) => errors.push((e, path)),
				}
				(media_datas, errors)
			},
		)
	};

	let created = db
		.media_data()
		.create_many(
			media_datas
				.into_iter()
				.filter_map(|(media_data, object_id)| {
					media_data_image_to_query(media_data, object_id)
						.map_err(|e| error!("{e:#?}"))
						.ok()
				})
				.collect(),
		)
		.skip_duplicates()
		.exec()
		.await?;

	run_metadata.extracted = created as u32;
	run_metadata.skipped += errors.len() as u32;

	Ok((
		run_metadata,
		errors
			.into_iter()
			.map(|(e, path)| format!("Couldn't process file: \"{}\"; Error: {e}", path.display()))
			.collect::<Vec<_>>()
			.into(),
	))
}
