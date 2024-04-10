use crate::old_job::JobRunErrors;

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ImageMetadata;
use sd_prisma::prisma::{exif_data, location, PrismaClient};
use sd_utils::error::FileIOError;

use std::{collections::HashSet, path::Path};

use futures_concurrency::future::Join;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::spawn_blocking;
use tracing::error;

use super::exif_data_image_to_query;

#[derive(Error, Debug)]
pub enum ExifDataError {
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
pub struct OldExifDataExtractorMetadata {
	pub extracted: u32,
	pub skipped: u32,
}

pub(super) static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_extract_exif_data_for_image)
		.map(Extension::Image)
		.collect()
});

pub const fn can_extract_exif_data_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::*;
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
}

pub async fn extract_exif_data(path: impl AsRef<Path>) -> Result<ImageMetadata, ExifDataError> {
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
) -> Result<(OldExifDataExtractorMetadata, JobRunErrors), ExifDataError> {
	let mut run_metadata = OldExifDataExtractorMetadata::default();
	if files_paths.is_empty() {
		return Ok((run_metadata, JobRunErrors::default()));
	}

	let location_path = location_path.as_ref();

	let objects_already_with_exif_data = db
		.exif_data()
		.find_many(vec![exif_data::object_id::in_vec(
			files_paths
				.iter()
				.filter_map(|file_path| file_path.object_id)
				.collect(),
		)])
		.select(exif_data::select!({ object_id }))
		.exec()
		.await?;

	if files_paths.len() == objects_already_with_exif_data.len() {
		// All files already have media data, skipping
		run_metadata.skipped = files_paths.len() as u32;
		return Ok((run_metadata, JobRunErrors::default()));
	}

	let objects_already_with_exif_data = objects_already_with_exif_data
		.into_iter()
		.map(|exif_data| exif_data.object_id)
		.collect::<HashSet<_>>();

	run_metadata.skipped = objects_already_with_exif_data.len() as u32;

	let (exif_datas, errors) = {
		let maybe_exif_data = files_paths
			.iter()
			.enumerate()
			.filter_map(|(idx, file_path)| {
				file_path.object_id.and_then(|object_id| {
					(!objects_already_with_exif_data.contains(&object_id))
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
				let res = extract_exif_data(&path).await;
				ctx_update_fn(idx + 1);
				(res, path, object_id)
			})
			.collect::<Vec<_>>()
			.join()
			.await;

		let total_exif_data = maybe_exif_data.len();

		maybe_exif_data.into_iter().fold(
			// In the good case, all exif data were extracted
			(Vec::with_capacity(total_exif_data), Vec::new()),
			|(mut exif_datas, mut errors), (maybe_exif_data, path, object_id)| {
				match maybe_exif_data {
					Ok(exif_data) => exif_datas.push((exif_data, object_id)),
					Err(ExifDataError::MediaData(sd_media_metadata::Error::NoExifDataOnPath(
						_,
					))) => {
						// No exif data on path, skipping
						run_metadata.skipped += 1;
					}
					Err(e) => errors.push((e, path)),
				}
				(exif_datas, errors)
			},
		)
	};

	let created = db
		.exif_data()
		.create_many(
			exif_datas
				.into_iter()
				.filter_map(|(exif_data, object_id)| {
					exif_data_image_to_query(exif_data, object_id)
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
