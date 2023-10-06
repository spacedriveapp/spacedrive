use crate::{
	invalidate_query,
	job::{JobError, JobRunMetadata},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_media_processor, IsolatedFilePathData,
	},
	object::media::{
		media_data_extractor,
		thumbnail::{self, init_thumbnail_dir, ThumbnailerEntryKind},
	},
	prisma::{location, PrismaClient},
	util::db::maybe_missing,
	Node,
};

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};

use itertools::Itertools;
use tracing::{debug, error};

use super::{
	get_files_by_extensions, process, MediaProcessorEntry, MediaProcessorEntryKind,
	MediaProcessorError, MediaProcessorMetadata,
};

const BATCH_SIZE: usize = 10;

pub async fn shallow(
	location: &location::Data,
	sub_path: &PathBuf,
	library: &Library,
	node: &Node,
) -> Result<(), JobError> {
	let Library { db, .. } = library;

	let thumbnails_base_dir = init_thumbnail_dir(node.config.data_directory())
		.await
		.map_err(MediaProcessorError::from)?;

	let location_id = location.id;
	let location_path = maybe_missing(&location.path, "location.path").map(PathBuf::from)?;

	let iso_file_path = if sub_path != Path::new("") {
		let full_path = ensure_sub_path_is_in_location(&location_path, &sub_path)
			.await
			.map_err(MediaProcessorError::from)?;
		ensure_sub_path_is_directory(&location_path, &sub_path)
			.await
			.map_err(MediaProcessorError::from)?;

		let sub_iso_file_path =
			IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
				.map_err(MediaProcessorError::from)?;

		ensure_file_path_exists(
			&sub_path,
			&sub_iso_file_path,
			db,
			MediaProcessorError::SubPathNotFound,
		)
		.await?;

		sub_iso_file_path
	} else {
		IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
			.map_err(MediaProcessorError::from)?
	};

	debug!("Searching for images in location {location_id} at path {iso_file_path}");

	let thumbnailer_files = get_files_for_thumbnailer(db, &iso_file_path).await?;

	let mut media_data_files_map = get_files_for_media_data_extraction(db, &iso_file_path)
		.await?
		.map(|file_path| (file_path.id, file_path))
		.collect::<HashMap<_, _>>();

	let mut total_files = 0;

	let chunked_files = thumbnailer_files
		.into_iter()
		.map(|(file_path, thumb_kind)| MediaProcessorEntry {
			operation_kind: if media_data_files_map.remove(&file_path.id).is_some() {
				MediaProcessorEntryKind::MediaDataAndThumbnailer(thumb_kind)
			} else {
				MediaProcessorEntryKind::Thumbnailer(thumb_kind)
			},
			file_path,
		})
		.collect::<Vec<_>>()
		.into_iter()
		.chain(
			media_data_files_map
				.into_values()
				.map(|file_path| MediaProcessorEntry {
					operation_kind: MediaProcessorEntryKind::MediaData,
					file_path,
				}),
		)
		.chunks(BATCH_SIZE)
		.into_iter()
		.map(|chunk| {
			let chunk = chunk.collect::<Vec<_>>();
			total_files += chunk.len();
			chunk
		})
		.collect::<Vec<_>>();

	debug!(
		"Preparing to process {total_files} files in {} chunks",
		chunked_files.len()
	);

	let mut run_metadata = MediaProcessorMetadata::default();

	for files in chunked_files {
		let (more_run_metadata, errors) = process(
			&files,
			location.id,
			&location_path,
			&thumbnails_base_dir,
			false,
			library,
			|_| {},
		)
		.await?;
		run_metadata.update(more_run_metadata);

		if !errors.is_empty() {
			error!("Errors processing chunk of media data shallow extraction:\n{errors}");
		}
	}

	debug!("Media shallow processor run metadata: {run_metadata:?}");

	if run_metadata.media_data.extracted > 0 || run_metadata.thumbnailer.created > 0 {
		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

async fn get_files_for_thumbnailer(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<
	impl Iterator<Item = (file_path_for_media_processor::Data, ThumbnailerEntryKind)>,
	MediaProcessorError,
> {
	// query database for all image files in this location that need thumbnails
	let image_thumb_files = get_files_by_extensions(
		db,
		parent_iso_file_path,
		&thumbnail::THUMBNAILABLE_EXTENSIONS,
	)
	.await?
	.into_iter()
	.map(|file_path| (file_path, ThumbnailerEntryKind::Image));

	#[cfg(feature = "ffmpeg")]
	let all_files = {
		// query database for all video files in this location that need thumbnails
		let video_files = get_files_by_extensions(
			db,
			parent_iso_file_path,
			&thumbnail::THUMBNAILABLE_VIDEO_EXTENSIONS,
		)
		.await?;

		image_thumb_files.chain(
			video_files
				.into_iter()
				.map(|file_path| (file_path, ThumbnailerEntryKind::Video)),
		)
	};
	#[cfg(not(feature = "ffmpeg"))]
	let all_files = { image_thumb_files };

	Ok(all_files)
}

async fn get_files_for_media_data_extraction(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<impl Iterator<Item = file_path_for_media_processor::Data>, MediaProcessorError> {
	get_files_by_extensions(
		db,
		parent_iso_file_path,
		&media_data_extractor::FILTERED_IMAGE_EXTENSIONS,
	)
	.await
	.map(|file_paths| file_paths.into_iter())
	.map_err(Into::into)
}
