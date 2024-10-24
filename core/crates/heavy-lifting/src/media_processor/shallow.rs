use crate::{
	media_processor, utils::sub_path::maybe_get_iso_file_path_from_sub_path, Error,
	NonCriticalError, OuterContext,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_sync::SyncManager;

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

use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use tracing::{debug, warn};

use super::{
	get_direct_children_files_by_extensions,
	helpers::{self, exif_media_data, ffmpeg_media_data, thumbnailer::THUMBNAIL_CACHE_DIR_NAME},
	tasks::{
		self, media_data_extractor,
		thumbnailer::{self, NewThumbnailReporter},
	},
	NewThumbnailsReporter, BATCH_SIZE,
};

#[allow(clippy::missing_panics_doc)] // SAFETY: It doesn't actually panics
pub async fn shallow(
	location: location::Data,
	sub_path: impl AsRef<Path> + Send,
	dispatcher: &BaseTaskDispatcher<Error>,
	ctx: &impl OuterContext,
) -> Result<Vec<NonCriticalError>, Error> {
	let sub_path = sub_path.as_ref();

	let location_path = maybe_missing(&location.path, "location.path")
		.map(PathBuf::from)
		.map(Arc::new)
		.map_err(media_processor::Error::from)?;

	let location = Arc::new(location);

	let sub_iso_file_path = maybe_get_iso_file_path_from_sub_path::<media_processor::Error>(
		location.id,
		Some(sub_path),
		&*location_path,
		ctx.db(),
	)
	.await?
	.map_or_else(
		|| {
			IsolatedFilePathData::new(location.id, &*location_path, &*location_path, true)
				.map_err(media_processor::Error::from)
		},
		Ok,
	)?;

	let mut errors = vec![];

	let media_data_extraction_tasks = dispatch_media_data_extractor_tasks(
		ctx.db(),
		ctx.sync(),
		&sub_iso_file_path,
		&location_path,
		dispatcher,
	)
	.await?;

	let total_media_data_extraction_tasks = media_data_extraction_tasks.len();

	let thumbnailer_tasks =
		dispatch_thumbnailer_tasks(&sub_iso_file_path, false, &location_path, dispatcher, ctx)
			.await?;

	let total_thumbnailer_tasks = thumbnailer_tasks.len();

	let mut futures = media_data_extraction_tasks
		.into_iter()
		.chain(thumbnailer_tasks.into_iter())
		.map(CancelTaskOnDrop::new)
		.collect::<FuturesUnordered<_>>();

	let mut completed_media_data_extraction_tasks = 0;
	let mut completed_thumbnailer_tasks = 0;

	while let Some(res) = futures.next().await {
		match res {
			Ok(TaskStatus::Done((_, TaskOutput::Out(out)))) => {
				if out.is::<media_data_extractor::Output>() {
					let media_data_extractor::Output {
						db_read_time,
						filtering_time,
						extraction_time,
						db_write_time,
						errors: new_errors,
						..
					} = *out
						.downcast::<media_data_extractor::Output>()
						.expect("just checked");

					errors.extend(new_errors);

					completed_media_data_extraction_tasks += 1;

					debug!(
						"Media data extraction task ({completed_media_data_extraction_tasks}/\
					{total_media_data_extraction_tasks}) completed in {:?};",
						db_read_time + filtering_time + extraction_time + db_write_time
					);
				} else if out.is::<thumbnailer::Output>() {
					let thumbnailer::Output {
						total_time,
						errors: new_errors,
						..
					} = *out.downcast::<thumbnailer::Output>().expect("just checked");

					errors.extend(new_errors);

					completed_thumbnailer_tasks += 1;

					debug!(
						"Thumbnailer task ({completed_thumbnailer_tasks}/{total_thumbnailer_tasks}) \
						completed in {total_time:?};",
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
	sync: &SyncManager,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	location_path: &Arc<PathBuf>,
	dispatcher: &BaseTaskDispatcher<Error>,
) -> Result<Vec<TaskHandle<Error>>, Error> {
	let (extract_exif_file_paths, extract_ffmpeg_file_paths) = (
		get_direct_children_files_by_extensions(
			parent_iso_file_path,
			&exif_media_data::AVAILABLE_EXTENSIONS,
			db,
		),
		get_direct_children_files_by_extensions(
			parent_iso_file_path,
			&ffmpeg_media_data::AVAILABLE_EXTENSIONS,
			db,
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
				sync.clone(),
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
						sync.clone(),
					)
				})
				.map(IntoTask::into_task),
		)
		.collect::<Vec<_>>();

	dispatcher.dispatch_many_boxed(tasks).await.map_or_else(
		|_| {
			debug!("Task system is shutting down while a shallow media processor was in progress");
			Ok(vec![])
		},
		Ok,
	)
}

async fn dispatch_thumbnailer_tasks(
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	should_regenerate: bool,
	location_path: &Path,
	dispatcher: &BaseTaskDispatcher<Error>,
	ctx: &impl OuterContext,
) -> Result<Vec<TaskHandle<Error>>, Error> {
	let thumbnails_directory_path =
		Arc::new(ctx.get_data_directory().join(THUMBNAIL_CACHE_DIR_NAME));
	let location_id = parent_iso_file_path.location_id();
	let library_id = ctx.id();
	let db = ctx.db();
	let reporter: Arc<dyn NewThumbnailReporter> =
		Arc::new(NewThumbnailsReporter { ctx: ctx.clone() });

	let file_paths = get_direct_children_files_by_extensions(
		parent_iso_file_path,
		&helpers::thumbnailer::ALL_THUMBNAILABLE_EXTENSIONS,
		db,
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

	debug!(%thumbs_count, priority_tasks_count = tasks.len(), "Dispatching thumbnails to be processed;");

	dispatcher.dispatch_many_boxed(tasks).await.map_or_else(
		|_| {
			debug!("Task system is shutting down while a shallow media processor was in progress");
			Ok(vec![])
		},
		Ok,
	)
}
