use crate::{
	job_system::{
		job::{Job, JobTaskDispatcher, ReturnStatus, UpdateEvent},
		report::ReportOutputMetadata,
		SerializableJob, SerializedTasks,
	},
	media_processor,
	utils::sub_path::{self, maybe_get_iso_file_path_from_sub_path},
	Error, JobContext, JobName,
};
use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::Extension;
use sd_prisma::prisma::{location, PrismaClient};
use sd_task_system::{IntoTask, SerializableTask, Task, TaskDispatcher, TaskHandle};
use sd_utils::db::maybe_missing;

use std::{
	collections::HashMap,
	fmt,
	hash::{Hash, Hasher},
	mem,
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use futures::stream::FuturesUnordered;
use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::debug;

use super::{
	helpers,
	tasks::{self, media_data_extractor, thumbnailer},
	thumbnailer::NewThumbnailReporter,
};

const BATCH_SIZE: usize = 10;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TaskKind {
	MediaDataExtractor,
	Thumbnailer,
}

#[derive(Debug)]
pub struct MediaProcessor {
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,
	regenerate_thumbnails: bool,

	total_media_data_extraction_tasks: u32,
	total_thumbnailer_tasks: u32,

	metadata: Metadata,

	errors: Vec<crate::NonCriticalError>,

	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Box<dyn Task<Error>>>,
}

impl Job for MediaProcessor {
	const NAME: JobName = JobName::MediaProcessor;

	async fn resume_tasks(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext,
		SerializedTasks(serialized_tasks): SerializedTasks,
	) -> Result<(), Error> {
		let reporter = Arc::new(NewThumbnailsReporter {
			job_ctx: ctx.clone(),
		});

		self.pending_tasks_on_resume = dispatcher
			.dispatch_many_boxed(
				rmp_serde::from_slice::<Vec<(TaskKind, Vec<u8>)>>(&serialized_tasks)
					.map_err(media_processor::Error::from)?
					.into_iter()
					.map(|(task_kind, task_bytes)| {
						let reporter = Arc::clone(&reporter);
						async move {
							match task_kind {
								TaskKind::MediaDataExtractor => {
									tasks::MediaDataExtractor::deserialize(
										&task_bytes,
										Arc::clone(ctx.db()),
									)
									.await
									.map(IntoTask::into_task)
								}

								TaskKind::Thumbnailer => tasks::Thumbnailer::deserialize(
									&task_bytes,
									Arc::clone(&reporter),
								)
								.await
								.map(IntoTask::into_task),
							}
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await
					.map_err(media_processor::Error::from)?,
			)
			.await;

		Ok(())
	}

	async fn run<Ctx: JobContext>(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: Ctx,
	) -> Result<ReturnStatus, Error> {
		let mut pending_running_tasks = FuturesUnordered::new();

		self.init_or_resume(&mut pending_running_tasks, &ctx, &dispatcher)
			.await?;

		if let Some(res) = self
			.process_handles(&mut pending_running_tasks, &ctx, &dispatcher)
			.await
		{
			return res;
		}

		if !self.tasks_for_shutdown.is_empty() {
			return Ok(ReturnStatus::Shutdown(
				SerializableJob::<Ctx>::serialize(self).await,
			));
		}

		Ok(ReturnStatus::Canceled)
	}
}

impl MediaProcessor {
	pub fn new(
		location: location::Data,
		sub_path: Option<PathBuf>,
		regenerate_thumbnails: bool,
	) -> Result<Self, media_processor::Error> {
		Ok(Self {
			location_path: maybe_missing(&location.path, "location.path")
				.map(PathBuf::from)
				.map(Arc::new)?,
			location: Arc::new(location),
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_tasks: 0,
			total_thumbnailer_tasks: 0,
			metadata: Metadata::default(),
			errors: Vec::new(),
			pending_tasks_on_resume: Vec::new(),
			tasks_for_shutdown: Vec::new(),
		})
	}

	async fn init_or_resume(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		job_ctx: &impl JobContext,
		dispatcher: &JobTaskDispatcher,
	) -> Result<(), media_processor::Error> {
		// if we don't have any pending task, then this is a fresh job
		if self.pending_tasks_on_resume.is_empty() {
			let location_id = self.location.id;
			let location_path = &*self.location_path;

			let iso_file_path = if let Some(iso_file_path) = maybe_get_iso_file_path_from_sub_path(
				location_id,
				&self.sub_path,
				&*self.location_path,
				job_ctx.db(),
			)
			.await?
			{
				iso_file_path
			} else {
				IsolatedFilePathData::new(location_id, location_path, location_path, true)
					.map_err(sub_path::Error::from)?
			};

			debug!(
				"Searching for media files in location {location_id} at directory \"{iso_file_path}\""
			);

		// First we will dispatch all tasks for media data extraction so we have a nice reporting
		} else {
			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));
		}

		Ok(())
	}

	async fn process_handles(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		ctx: &impl JobContext,
		dispatcher: &JobTaskDispatcher,
	) -> Option<Result<ReturnStatus, Error>> {
		todo!()
	}
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Metadata {
	media_data_extracted: u32,
	media_data_skipped: u32,
	total_successful_media_data_extractor_tasks: u32,
	thumbnails_generated: u32,
	thumbnails_skipped: u32,
	total_thumbnails_generation_time: Duration,
	mean_thumbnails_generation_time: Duration,
	thumbnails_generation_std_dev: Duration,
	total_successful_thumbnailer_tasks: u32,
}

impl From<Metadata> for ReportOutputMetadata {
	fn from(value: Metadata) -> Self {
		Self::Metrics(HashMap::from([
			//
			// Media data extractor
			//
			(
				"media_data_extracted".into(),
				json!(value.media_data_extracted),
			),
			("media_data_skipped".into(), json!(value.media_data_skipped)),
			(
				"total_successful_media_data_extractor_tasks".into(),
				json!(value.total_successful_media_data_extractor_tasks),
			),
			//
			// Thumbnailer
			//
			(
				"thumbnails_generated".into(),
				json!(value.thumbnails_generated),
			),
			("thumbnails_skipped".into(), json!(value.thumbnails_skipped)),
			(
				"total_thumbnails_generation_time".into(),
				json!(value.total_thumbnails_generation_time),
			),
			(
				"mean_thumbnails_generation_time".into(),
				json!(value.mean_thumbnails_generation_time),
			),
			(
				"thumbnails_generation_std_dev".into(),
				json!(value.thumbnails_generation_std_dev),
			),
			(
				"total_successful_thumbnailer_tasks".into(),
				json!(value.total_successful_thumbnailer_tasks),
			),
		]))
	}
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,
	regenerate_thumbnails: bool,

	total_media_data_extraction_tasks: u32,
	total_thumbnailer_tasks: u32,

	metadata: Metadata,

	errors: Vec<crate::NonCriticalError>,

	tasks_for_shutdown_bytes: Option<SerializedTasks>,
}

impl<Ctx: JobContext> SerializableJob<Ctx> for MediaProcessor {
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let Self {
			location,
			location_path,
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_tasks,
			total_thumbnailer_tasks,
			metadata,
			errors,
			tasks_for_shutdown,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			location,
			location_path,
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_tasks,
			total_thumbnailer_tasks,
			metadata,
			tasks_for_shutdown_bytes: Some(SerializedTasks(rmp_serde::to_vec_named(
				&tasks_for_shutdown
					.into_iter()
					.map(|task| async move {
						if task.is::<tasks::MediaDataExtractor>() {
							task.downcast::<tasks::MediaDataExtractor>()
								.expect("just checked")
								.serialize()
								.await
								.map(|bytes| (TaskKind::MediaDataExtractor, bytes))
						} else if task.is::<tasks::Thumbnailer<NewThumbnailsReporter<Ctx>>>() {
							task.downcast::<tasks::Thumbnailer<NewThumbnailsReporter<Ctx>>>()
								.expect("just checked")
								.serialize()
								.await
								.map(|bytes| (TaskKind::Thumbnailer, bytes))
						} else {
							unreachable!("Unexpected task type")
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await?,
			)?)),
			errors,
		})
		.map(Some)
	}

	async fn deserialize(
		serialized_job: &[u8],
		_: &Ctx,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let SaveState {
			location,
			location_path,
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_tasks,
			total_thumbnailer_tasks,
			metadata,
			errors,
			tasks_for_shutdown_bytes,
		} = rmp_serde::from_slice::<SaveState>(serialized_job)?;

		Ok(Some((
			Self {
				location,
				location_path,
				sub_path,
				regenerate_thumbnails,
				total_media_data_extraction_tasks,
				total_thumbnailer_tasks,
				metadata,
				errors,
				pending_tasks_on_resume: Vec::new(),
				tasks_for_shutdown: Vec::new(),
			},
			tasks_for_shutdown_bytes,
		)))
	}
}

struct NewThumbnailsReporter<Ctx: JobContext> {
	job_ctx: Ctx,
}

impl<Ctx: JobContext> fmt::Debug for NewThumbnailsReporter<Ctx> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NewThumbnailsReporter").finish()
	}
}

impl<Ctx: JobContext> NewThumbnailReporter for NewThumbnailsReporter<Ctx> {
	fn new_thumbnail(&self, thumb_key: media_processor::ThumbKey) {
		self.job_ctx
			.report_update(UpdateEvent::NewThumbnailEvent { thumb_key });
	}
}

impl Hash for MediaProcessor {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

async fn dispatch_media_data_extractor_tasks(
	db: &Arc<PrismaClient>,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	location_path: &Arc<PathBuf>,
	dispatcher: &JobTaskDispatcher,
) -> Result<(u64, Vec<TaskHandle<Error>>), media_processor::Error> {
	let file_paths = get_files_for_media_data_extraction(db, parent_iso_file_path).await?;

	let files_count = file_paths.len() as u64;

	let tasks = file_paths
		.into_iter()
		.chunks(BATCH_SIZE)
		.into_iter()
		.map(Iterator::collect::<Vec<_>>)
		.map(|chunked_file_paths| {
			tasks::MediaDataExtractor::new_deep(
				&chunked_file_paths,
				parent_iso_file_path.location_id(),
				Arc::clone(location_path),
				Arc::clone(db),
			)
		})
		.map(IntoTask::into_task)
		.collect::<Vec<_>>();

	Ok((files_count, dispatcher.dispatch_many_boxed(tasks).await))
}

async fn get_files_for_media_data_extraction(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<Vec<file_path_for_media_processor::Data>, media_processor::Error> {
	get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&helpers::media_data_extractor::FILTERED_IMAGE_EXTENSIONS,
	)
	.await
	.map_err(Into::into)
}

async fn get_all_children_files_by_extensions(
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
				AND materialized_path LIKE {{}}
			ORDER BY materialized_path ASC",
			// Ordering by materialized_path so we can prioritize processing the first files
			// in the above part of the directories tree
			extensions
				.iter()
				.map(|ext| format!("LOWER('{ext}')"))
				.collect::<Vec<_>>()
				.join(",")
		),
		PrismaValue::Int(i64::from(parent_iso_file_path.location_id())),
		PrismaValue::String(format!(
			"{}%",
			parent_iso_file_path
				.materialized_path_for_children()
				.expect("sub path iso_file_path must be a directory")
		))
	))
	.exec()
	.await
	.map_err(Into::into)
}
