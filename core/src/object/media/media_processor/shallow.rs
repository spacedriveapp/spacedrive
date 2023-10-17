use crate::{
	invalidate_query,
	job::{JobError, JobRunMetadata},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_media_processor, IsolatedFilePathData,
	},
	prisma::{location, PrismaClient},
	util::db::maybe_missing,
	Node,
};

use std::{
	future::Future,
	path::{Path, PathBuf},
};

use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use sd_file_ext::extensions::Extension;
use tracing::{debug, error};

use super::{
	dispatch_thumbnails_for_processing,
	media_data_extractor::{self, process},
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

	dispatch_thumbnails_for_processing(
		location.id,
		&location_path,
		&iso_file_path,
		library,
		node,
		false,
		get_files_by_extensions,
	)
	.await?;

	let file_paths = get_files_for_media_data_extraction(db, &iso_file_path).await?;

	let total_files = file_paths.len();

	let chunked_files = file_paths
		.into_iter()
		.chunks(BATCH_SIZE)
		.into_iter()
		.map(Iterator::collect)
		.collect::<Vec<Vec<_>>>();

	debug!(
		"Preparing to process {total_files} files in {} chunks",
		chunked_files.len()
	);

	let mut run_metadata = MediaProcessorMetadata::default();

	for files in chunked_files {
		let (more_run_metadata, errors) = process(&files, location.id, &location_path, db, &|_| {})
			.await
			.map_err(MediaProcessorError::from)?;

		run_metadata.update(more_run_metadata.into());

		if !errors.is_empty() {
			error!("Errors processing chunk of media data shallow extraction:\n{errors}");
		}
	}

	debug!("Media shallow processor run metadata: {run_metadata:?}");

	if run_metadata.media_data.extracted > 0 {
		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

async fn get_files_for_media_data_extraction(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	get_files_by_extensions(
		db,
		parent_iso_file_path,
		&media_data_extractor::FILTERED_IMAGE_EXTENSIONS,
	)
	.await
	.map_err(Into::into)
}

fn get_files_by_extensions<'d, 'p, 'e, 'ret>(
	db: &'d PrismaClient,
	parent_iso_file_path: &'p IsolatedFilePathData<'_>,
	extensions: &'e [Extension],
) -> impl Future<Output = Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError>> + 'ret
where
	'd: 'ret,
	'p: 'ret,
	'e: 'ret,
{
	async move {
		// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
		// We have no data coming from the user, so this is sql injection safe
		db._query_raw(raw!(
			&format!(
				"SELECT id, materialized_path, is_dir, name, extension, cas_id, object_id
			FROM file_path
			WHERE
				location_id={{}}
				AND cas_id IS NOT NULL
				AND LOWER(extension) IN ({})
				AND materialized_path = {{}}",
				extensions
					.iter()
					.map(|ext| format!("LOWER('{ext}')"))
					.collect::<Vec<_>>()
					.join(",")
			),
			PrismaValue::Int(parent_iso_file_path.location_id() as i64),
			PrismaValue::String(
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory")
			)
		))
		.exec()
		.await
		.map_err(Into::into)
	}
}
