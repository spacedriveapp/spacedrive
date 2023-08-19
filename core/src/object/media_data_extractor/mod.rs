use crate::{
	job::{JobRunErrors, JobRunMetadata},
	library::Library,
	location::file_path_helper::{file_path_for_media_data, FilePathError, IsolatedFilePathData},
	prisma::{location, media_data},
	util::error::FileIOError,
};

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_data::MediaDataImage;

use std::{collections::HashSet, path::Path};

use futures_concurrency::future::Join;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::spawn_blocking;
use tracing::error;

mod full_job;
mod shallow;

pub use full_job::MediaDataJobInit;
pub use shallow::shallow;

#[derive(Error, Debug)]
pub enum MediaDataError {
	#[error("sub path not found: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	MediaData(#[from] sd_media_data::Error),
	#[error("failed to join tokio task: {0}")]
	TokioJoinHandle(#[from] tokio::task::JoinError),
}

pub type MediaDataJobStep = Vec<file_path_for_media_data::Data>;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MediaDataJobRunMetadata {
	media_data_extracted: u32,
	media_data_skipped: u32,
}

impl JobRunMetadata for MediaDataJobRunMetadata {
	fn update(&mut self, new_data: Self) {
		self.media_data_extracted += new_data.media_data_extracted;
		self.media_data_skipped += new_data.media_data_skipped;
	}
}

static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_generate_media_data_for_image)
		.map(Extension::Image)
		.collect()
});

pub const fn can_generate_media_data_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::*;
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
}

pub async fn extract_media_data(path: impl AsRef<Path>) -> Result<MediaDataImage, MediaDataError> {
	let path = path.as_ref().to_path_buf();

	// Running in a separated blocking thread due to MediaData blocking behavior (due to sync exif lib)
	spawn_blocking(|| MediaDataImage::from_path(path))
		.await?
		.map_err(Into::into)
}

pub async fn inner_process_step(
	step: &MediaDataJobStep,
	location_path: impl AsRef<Path>,
	location: &location::Data,
	library: &Library,
) -> Result<(MediaDataJobRunMetadata, JobRunErrors), MediaDataError> {
	let mut run_metadata = MediaDataJobRunMetadata::default();

	let location_path = location_path.as_ref();

	let objects_already_with_media_data = library
		.db
		.media_data()
		.find_many(vec![media_data::object_id::in_vec(
			step.iter()
				.filter_map(|file_path| file_path.object_id)
				.collect(),
		)])
		.select(media_data::select!({ object_id }))
		.exec()
		.await?
		.into_iter()
		.map(|media_data| media_data.object_id)
		.collect::<HashSet<_>>();

	run_metadata.media_data_skipped = objects_already_with_media_data.len() as u32;

	let (media_datas, errors) = {
		let maybe_media_data = step
			.iter()
			.filter_map(|file_path| {
				file_path.object_id.and_then(|object_id| {
					(!objects_already_with_media_data.contains(&object_id))
						.then_some((file_path, object_id))
				})
			})
			.filter_map(|(file_path, object_id)| {
				IsolatedFilePathData::try_from((location.id, file_path))
					.map_err(|e| error!("{e:#?}"))
					.ok()
					.map(|iso_file_path| (location_path.join(iso_file_path), object_id))
			})
			.map(
				|(path, object_id)| async move { (extract_media_data(&path).await, path, object_id) },
			)
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
					Err(e) => errors.push((e, path)),
				}
				(media_datas, errors)
			},
		)
	};

	let created = library
		.db
		.media_data()
		.create_many(
			media_datas
				.into_iter()
				.filter_map(|(media_data, object_id)| {
					media_data
						.to_query(object_id)
						.map_err(|e| error!("{e:#?}"))
						.ok()
				})
				.collect(),
		)
		.exec()
		.await?;

	run_metadata.media_data_extracted = created as u32;
	run_metadata.media_data_skipped += errors.len() as u32;

	Ok((
		run_metadata,
		errors
			.into_iter()
			.map(|(e, path)| format!("Couldn't process file: \"{}\"; Error: {e}", path.display()))
			.collect::<Vec<_>>()
			.into(),
	))
}
