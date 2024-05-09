use crate::{
	media_processor, utils::sub_path::maybe_get_iso_file_path_from_sub_path, Error,
	NonCriticalError, OuterContext,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::Extension;
use sd_prisma::prisma::{location, PrismaClient};
use sd_task_system::{
	BaseTaskDispatcher, CancelTaskOnDrop, IntoTask, TaskDispatcher, TaskHandle, TaskOutput,
	TaskStatus,
};
use sd_utils::db::maybe_missing;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::StreamExt;
use futures_concurrency::future::{FutureGroup, TryJoin};
use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use tracing::{debug, warn};

use super::{
	helpers::{self, exif_media_data, ffmpeg_media_data, thumbnailer::THUMBNAIL_CACHE_DIR_NAME},
	tasks::{self, media_data_extractor, thumbnailer},
	NewThumbnailsReporter, BATCH_SIZE,
};

#[allow(clippy::missing_panics_doc)] // SAFETY: It doesn't actually panics
pub async fn shallow(
	location: location::Data,
	sub_path: impl AsRef<Path> + Send,
	dispatcher: BaseTaskDispatcher<Error>,
	ctx: impl OuterContext,
) -> Result<Vec<NonCriticalError>, Error> {
	let sub_path = sub_path.as_ref();

	let location_path = maybe_missing(&location.path, "location.path")
		.map(PathBuf::from)
		.map(Arc::new)
		.map_err(media_processor::Error::from)?;

	let location = Arc::new(location);

	let sub_iso_file_path = maybe_get_iso_file_path_from_sub_path(
		location.id,
		&Some(sub_path),
		&*location_path,
		ctx.db(),
	)
	.await
	.map_err(media_processor::Error::from)?
	.map_or_else(
		|| {
			IsolatedFilePathData::new(location.id, &*location_path, &*location_path, true)
				.map_err(media_processor::Error::from)
		},
		Ok,
	)?;

	let mut errors = vec![];

	let mut futures = dispatch_media_data_extractor_tasks(
		ctx.db(),
		&sub_iso_file_path,
		&location_path,
		&dispatcher,
	)
	.await?
	.into_iter()
	.map(CancelTaskOnDrop)
	.chain(
		dispatch_thumbnailer_tasks(&sub_iso_file_path, false, &location_path, &dispatcher, &ctx)
			.await?
			.into_iter()
			.map(CancelTaskOnDrop),
	)
	.collect::<FutureGroup<_>>();

	while let Some(res) = futures.next().await {
		match res {
			Ok(TaskStatus::Done((_, TaskOutput::Out(out)))) => {
				if out.is::<media_data_extractor::Output>() {
					errors.extend(
						out.downcast::<media_data_extractor::Output>()
							.expect("just checked")
							.errors,
					);
				} else if out.is::<thumbnailer::Output>() {
					errors.extend(
						out.downcast::<thumbnailer::Output>()
							.expect("just checked")
							.errors,
					);
				} else {
					unreachable!(
						"Task returned unexpected output type on media processor shallow job"
					);
				}
			}
			Ok(TaskStatus::Done((_, TaskOutput::Empty))) => {
				warn!("Task returned empty output on media processor shallow job");
			}
			Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion | TaskStatus::Shutdown(_)) => {
				return Ok(errors);
			}
			Ok(TaskStatus::Error(e)) => return Err(e),

			Err(e) => return Err(e.into()),
		}
	}

	Ok(errors)
}

async fn dispatch_media_data_extractor_tasks(
	db: &Arc<PrismaClient>,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	location_path: &Arc<PathBuf>,
	dispatcher: &BaseTaskDispatcher<Error>,
) -> Result<Vec<TaskHandle<Error>>, media_processor::Error> {
	let (extract_exif_file_paths, extract_ffmpeg_file_paths) = (
		get_files_by_extensions(
			db,
			parent_iso_file_path,
			&exif_media_data::AVAILABLE_EXTENSIONS,
		),
		get_files_by_extensions(
			db,
			parent_iso_file_path,
			&ffmpeg_media_data::AVAILABLE_EXTENSIONS,
		),
	)
		.try_join()
		.await?;

	let tasks = extract_exif_file_paths
		.into_iter()
		.chunks(BATCH_SIZE)
		.into_iter()
		.map(Iterator::collect::<Vec<_>>)
		.map(|chunked_file_paths| {
			tasks::MediaDataExtractor::new_exif(
				&chunked_file_paths,
				parent_iso_file_path.location_id(),
				Arc::clone(location_path),
				Arc::clone(db),
			)
		})
		.map(IntoTask::into_task)
		.chain(
			extract_ffmpeg_file_paths
				.into_iter()
				.chunks(BATCH_SIZE)
				.into_iter()
				.map(Iterator::collect::<Vec<_>>)
				.map(|chunked_file_paths| {
					tasks::MediaDataExtractor::new_ffmpeg(
						&chunked_file_paths,
						parent_iso_file_path.location_id(),
						Arc::clone(location_path),
						Arc::clone(db),
					)
				})
				.map(IntoTask::into_task),
		)
		.collect::<Vec<_>>();

	Ok(dispatcher.dispatch_many_boxed(tasks).await)
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<Vec<file_path_for_media_processor::Data>, media_processor::Error> {
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
		PrismaValue::Int(i64::from(parent_iso_file_path.location_id())),
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

async fn dispatch_thumbnailer_tasks(
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	should_regenerate: bool,
	location_path: &PathBuf,
	dispatcher: &BaseTaskDispatcher<Error>,
	ctx: &impl OuterContext,
) -> Result<Vec<TaskHandle<Error>>, media_processor::Error> {
	let thumbnails_directory_path =
		Arc::new(ctx.get_data_directory().join(THUMBNAIL_CACHE_DIR_NAME));
	let location_id = parent_iso_file_path.location_id();
	let library_id = ctx.id();
	let db = ctx.db();
	let reporter = Arc::new(NewThumbnailsReporter { ctx: ctx.clone() });

	let file_paths = get_files_by_extensions(
		db,
		parent_iso_file_path,
		&helpers::thumbnailer::ALL_THUMBNAILABLE_EXTENSIONS,
	)
	.await?;

	let thumbs_count = file_paths.len() as u64;

	let tasks = file_paths
		.into_iter()
		.chunks(BATCH_SIZE)
		.into_iter()
		.map(|chunk| {
			tasks::Thumbnailer::new_indexed(
				Arc::clone(&thumbnails_directory_path),
				&chunk.collect::<Vec<_>>(),
				(location_id, location_path),
				library_id,
				should_regenerate,
				true,
				Arc::clone(&reporter),
			)
		})
		.map(IntoTask::into_task)
		.collect::<Vec<_>>();

	debug!(
		"Dispatching {thumbs_count} thumbnails to be processed, in {} priority tasks",
		tasks.len(),
	);

	Ok(dispatcher.dispatch_many_boxed(tasks).await)
}
