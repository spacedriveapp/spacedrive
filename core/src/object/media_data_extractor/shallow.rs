use crate::{
	invalidate_query,
	job::{JobError, JobRunMetadata},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_media_data, IsolatedFilePathData,
	},
	prisma::{file_path, location, PrismaClient},
	util::db::maybe_missing,
};

use sd_file_ext::extensions::Extension;

use std::path::{Path, PathBuf};

use itertools::Itertools;
use tracing::{debug, error, info};

use super::{
	inner_process_step, MediaDataError, MediaDataJobRunMetadata, MediaDataJobStep,
	FILTERED_IMAGE_EXTENSIONS,
};

const BATCH_SIZE: usize = 100;

pub async fn shallow(
	location: &location::Data,
	sub_path: &PathBuf,
	library: &Library,
) -> Result<(), JobError> {
	let Library { db, .. } = library;

	let location_id = location.id;
	let location_path = maybe_missing(&location.path, "location.path").map(PathBuf::from)?;

	let iso_file_path = if sub_path != Path::new("") {
		let full_path = ensure_sub_path_is_in_location(&location_path, &sub_path)
			.await
			.map_err(MediaDataError::from)?;
		ensure_sub_path_is_directory(&location_path, &sub_path)
			.await
			.map_err(MediaDataError::from)?;

		let sub_iso_file_path =
			IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
				.map_err(MediaDataError::from)?;

		ensure_file_path_exists(
			&sub_path,
			&sub_iso_file_path,
			db,
			MediaDataError::SubPathNotFound,
		)
		.await?;

		sub_iso_file_path
	} else {
		IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
			.map_err(MediaDataError::from)?
	};

	debug!("Searching for images in location {location_id} at path {iso_file_path}");

	let image_files = get_files_by_extensions(
		&library.db,
		location_id,
		&iso_file_path,
		&FILTERED_IMAGE_EXTENSIONS,
	)
	.await?;

	debug!("Found {:?} image files", image_files.len());

	let mut run_metadata = MediaDataJobRunMetadata::default();

	let chunked_files = image_files
		.into_iter()
		.chunks(BATCH_SIZE)
		.into_iter()
		.map(|chunk| chunk.collect_vec())
		// Had to collect here because `Chunk` isn't `Send` and this function's future gets marked as `!Send`
		.collect::<Vec<_>>();

	for files in chunked_files {
		let (more_run_metadata, errors) =
			inner_process_step(&files, &location_path, location, library).await?;

		run_metadata.update(more_run_metadata);

		error!("Errors processing chunk of media data shallow extraction:\n{errors}");
	}

	info!("Media data shallow extraction run metadata: {run_metadata:#?}");

	if run_metadata.media_data_extracted > 0 {
		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	location_id: location::id::Type,
	parent_isolated_file_path_data: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<MediaDataJobStep, MediaDataError> {
	db.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(location_id)),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::materialized_path::equals(Some(
				parent_isolated_file_path_data
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory"),
			)),
		])
		.select(file_path_for_media_data::select())
		.exec()
		.await
		.map_err(Into::into)
}
