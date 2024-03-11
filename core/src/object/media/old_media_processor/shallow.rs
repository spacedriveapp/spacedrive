use crate::{
	invalidate_query,
	library::Library,
	object::media::old_thumbnail::GenerateThumbnailArgs,
	old_job::{JobError, JobRunMetadata},
	Node,
};

use sd_core_file_path_helper::{
	ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	file_path_for_media_processor, IsolatedFilePathData,
};
use sd_file_ext::extensions::Extension;
use sd_prisma::prisma::{location, PrismaClient};
use sd_utils::db::maybe_missing;

#[cfg(feature = "ai")]
use sd_ai::old_image_labeler::LabelerOutput;

use std::path::{Path, PathBuf};

#[cfg(feature = "ai")]
use std::sync::Arc;

use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use tracing::{debug, error};

#[cfg(feature = "ai")]
use futures::StreamExt;

use super::{
	media_data_extractor::{self, process},
	old_thumbnail::{self, BatchToProcess},
	MediaProcessorError, OldMediaProcessorMetadata,
};

const BATCH_SIZE: usize = 10;

pub async fn old_shallow(
	location: &location::Data,
	sub_path: &PathBuf,
	library @ Library { db, sync, .. }: &Library,
	#[cfg(feature = "ai")] regenerate_labels: bool,
	node: &Node,
) -> Result<(), JobError> {
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

	debug!("Searching for media in location {location_id} at path {iso_file_path}");

	dispatch_thumbnails_for_processing(
		location.id,
		&location_path,
		&iso_file_path,
		library,
		node,
		false,
	)
	.await?;

	let file_paths = get_files_for_media_data_extraction(db, &iso_file_path).await?;

	#[cfg(feature = "ai")]
	let file_paths_for_labelling =
		get_files_for_labeling(db, &iso_file_path, regenerate_labels).await?;

	#[cfg(feature = "ai")]
	let has_labels = !file_paths_for_labelling.is_empty();

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

	#[cfg(feature = "ai")]
	// Check if we have an image labeller and has_labels then enqueue a new batch
	let labels_rx = node.old_image_labeller.as_ref().and_then(|image_labeller| {
		has_labels.then(|| {
			image_labeller.new_batch(
				location_id,
				location_path.clone(),
				file_paths_for_labelling,
				Arc::clone(db),
				sync.clone(),
			)
		})
	});

	let mut run_metadata = OldMediaProcessorMetadata::default();

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
		invalidate_query!(library, "search.objects");
	}

	#[cfg(feature = "ai")]
	{
		if has_labels {
			if let Some(labels_rx) = labels_rx {
				labels_rx
					.await
					.for_each(
						|LabelerOutput {
						     file_path_id,
						     has_new_labels,
						     result,
						 }| async move {
							if let Err(e) = result {
								error!(
								"Failed to generate labels <file_path_id='{file_path_id}'>: {e:#?}"
							);
							} else if has_new_labels {
								// invalidate_query!(library, "labels.count"); // TODO: This query doesn't exist on main yet
							}
						},
					)
					.await;

				invalidate_query!(library, "labels.list");
				invalidate_query!(library, "labels.getForObject");
				invalidate_query!(library, "labels.getWithObjects");
			}
		}
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

#[cfg(feature = "ai")]
async fn get_files_for_labeling(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	regenerate_labels: bool,
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
	// We have no data coming from the user, so this is sql injection safe
	db._query_raw(raw!(
		&format!(
			"SELECT id, materialized_path, is_dir, name, extension, cas_id, object_id
			FROM file_path f
			WHERE
				location_id={{}}
				AND cas_id IS NOT NULL
				AND LOWER(extension) IN ({})
				AND materialized_path = {{}}
				{}",
			&media_data_extractor::FILTERED_IMAGE_EXTENSIONS
				.iter()
				.map(|ext| format!("LOWER('{ext}')"))
				.collect::<Vec<_>>()
				.join(","),
			if !regenerate_labels {
				"AND NOT EXISTS (SELECT 1 FROM label_on_object WHERE object_id = f.object_id)"
			} else {
				""
			}
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

async fn dispatch_thumbnails_for_processing(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	library: &Library,
	node: &Node,
	should_regenerate: bool,
) -> Result<(), MediaProcessorError> {
	let Library { db, .. } = library;

	let location_path = location_path.as_ref();

	let file_paths = get_files_by_extensions(
		db,
		parent_iso_file_path,
		&old_thumbnail::ALL_THUMBNAILABLE_EXTENSIONS,
	)
	.await?;

	let current_batch = file_paths
		.into_iter()
		.filter_map(|file_path| {
			if let Some(cas_id) = file_path.cas_id.as_ref() {
				Some((cas_id.clone(), file_path))
			} else {
				error!("File path <id='{}'> has no cas_id, skipping", file_path.id);
				None
			}
		})
		.filter_map(|(cas_id, file_path)| {
			let file_path_id = file_path.id;
			IsolatedFilePathData::try_from((location_id, file_path))
				.map_err(|e| {
					error!("Failed to extract isolated file path data from file path <id='{file_path_id}'>: {e:#?}");
				})
				.ok()
				.map(|iso_file_path| (cas_id, iso_file_path))
		})
		.map(|(cas_id, iso_file_path)| {
			let full_path = location_path.join(&iso_file_path);

			GenerateThumbnailArgs::new(iso_file_path.extension().to_string(), cas_id, full_path)
		})
		.collect::<Vec<_>>();

	// Let's not send an empty batch lol
	if !current_batch.is_empty() {
		node.thumbnailer
			.new_indexed_thumbnails_batch(
				BatchToProcess::new(current_batch, should_regenerate, false),
				library.id,
			)
			.await;
	}

	Ok(())
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
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
