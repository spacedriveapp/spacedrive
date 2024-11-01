use crate::{
	job_system::{
		job::{Job, JobReturn, JobTaskDispatcher, ReturnStatus},
		report::ReportOutputMetadata,
		utils::cancel_pending_tasks,
		DispatcherError, JobErrorOrDispatcherError, SerializableJob, SerializedTasks,
	},
	media_processor::{self, helpers::thumbnailer::THUMBNAIL_CACHE_DIR_NAME},
	utils::sub_path::maybe_get_iso_file_path_from_sub_path,
	Error, JobContext, JobName, LocationScanState, OuterContext, ProgressUpdate,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::Extension;
use sd_prisma::{
	prisma::{location, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_not_null_entry, OperationFactory};
use sd_task_system::{
	AnyTaskOutput, IntoTask, SerializableTask, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskOutput, TaskStatus, TaskSystemError,
};
use sd_utils::{db::maybe_missing, u64_to_frontend};

use std::{
	collections::{HashMap, HashSet},
	fmt,
	hash::{Hash, Hasher},
	mem,
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, instrument, trace, warn, Level};

use super::{
	get_direct_children_files_by_extensions, helpers,
	tasks::{
		self, media_data_extractor,
		thumbnailer::{self, NewThumbnailReporter},
	},
	NewThumbnailsReporter, RawFilePathForMediaProcessor, BATCH_SIZE,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TaskKind {
	MediaDataExtractor,
	Thumbnailer,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
enum Phase {
	MediaDataExtraction,
	ThumbnailGeneration,
	// LabelsGeneration, // TODO: Implement labels generation
}

impl Default for Phase {
	fn default() -> Self {
		Self::MediaDataExtraction
	}
}

impl fmt::Display for Phase {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::MediaDataExtraction => write!(f, "media_data"),
			Self::ThumbnailGeneration => write!(f, "thumbnails"),
			// Self::LabelsGeneration => write!(f, "labels"), // TODO: Implement labels generation
		}
	}
}

#[derive(Debug)]
pub struct MediaProcessor {
	// Received arguments
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,
	regenerate_thumbnails: bool,

	// Job control
	total_media_data_extraction_files: u64,
	total_media_data_extraction_tasks: u64,
	total_thumbnailer_tasks: u64,
	total_thumbnailer_files: u64,
	phase: Phase,

	// Run data
	metadata: Metadata,
	errors: Vec<crate::NonCriticalError>,

	// On shutdown data
	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Box<dyn Task<Error>>>,
}

impl Job for MediaProcessor {
	const NAME: JobName = JobName::MediaProcessor;

	async fn resume_tasks<OuterCtx: OuterContext>(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext<OuterCtx>,
		SerializedTasks(serialized_tasks): SerializedTasks,
	) -> Result<(), Error> {
		let reporter: Arc<dyn NewThumbnailReporter> =
			Arc::new(NewThumbnailsReporter { ctx: ctx.clone() });

		if let Ok(tasks) = dispatcher
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
										(Arc::clone(ctx.db()), ctx.sync().clone()),
									)
									.await
									.map(IntoTask::into_task)
								}

								TaskKind::Thumbnailer => {
									tasks::Thumbnailer::deserialize(&task_bytes, reporter)
										.await
										.map(IntoTask::into_task)
								}
							}
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await
					.map_err(media_processor::Error::from)?,
			)
			.await
		{
			self.pending_tasks_on_resume = tasks;
		} else {
			warn!("Failed to dispatch tasks to resume as job was already canceled");
		}

		Ok(())
	}

	#[instrument(
		skip_all,
		fields(
			location_id = self.location.id,
			location_path = ?self.location.path,
			sub_path = ?self.sub_path.as_ref().map(|path| path.display()),
			regenerate_thumbnails = self.regenerate_thumbnails,
		),
		ret(level = Level::TRACE),
		err,
	)]
	async fn run<OuterCtx: OuterContext>(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: impl JobContext<OuterCtx>,
	) -> Result<ReturnStatus, Error> {
		let mut pending_running_tasks = FuturesUnordered::new();

		match self
			.init_or_resume(&mut pending_running_tasks, &ctx, &dispatcher)
			.await
		{
			Ok(()) => { /* Everything is awesome! */ }
			Err(JobErrorOrDispatcherError::JobError(e)) => {
				return Err(e.into());
			}
			Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::JobCanceled(_))) => {
				return Ok(self.cancel_job(&mut pending_running_tasks).await);
			}
			Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::Shutdown(tasks))) => {
				self.tasks_for_shutdown.extend(tasks);

				if pending_running_tasks.is_empty() {
					// If no task managed to be dispatched, we can just shutdown
					// otherwise we have to process handles below and wait for them to be shutdown too
					return Ok(ReturnStatus::Shutdown(
						SerializableJob::<OuterCtx>::serialize(self).await,
					));
				}
			}
		}

		if let Some(res) = self.process_handles(&mut pending_running_tasks, &ctx).await {
			return res;
		}

		if !self.tasks_for_shutdown.is_empty() {
			return Ok(ReturnStatus::Shutdown(
				SerializableJob::<OuterCtx>::serialize(self).await,
			));
		}

		// From this point onward, we are done with the job and it can't be interrupted anymore
		let Self {
			location,
			metadata,
			errors,
			..
		} = self;

		let (sync_param, db_param) =
			sync_db_not_null_entry!(LocationScanState::Completed as i32, location::scan_state);

		ctx.sync()
			.write_op(
				ctx.db(),
				ctx.sync().shared_update(
					prisma_sync::location::SyncId {
						pub_id: location.pub_id.clone(),
					},
					[sync_param],
				),
				ctx.db()
					.location()
					.update(location::id::equals(location.id), vec![db_param])
					.select(location::select!({ id })),
			)
			.await
			.map_err(media_processor::Error::from)?;

		Ok(ReturnStatus::Completed(
			JobReturn::builder()
				.with_metadata(metadata)
				.with_non_critical_errors(errors)
				.build(),
		))
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
			total_media_data_extraction_files: 0,
			total_media_data_extraction_tasks: 0,
			total_thumbnailer_tasks: 0,
			total_thumbnailer_files: 0,
			phase: Phase::default(),
			metadata: Metadata::default(),
			errors: Vec::new(),
			pending_tasks_on_resume: Vec::new(),
			tasks_for_shutdown: Vec::new(),
		})
	}

	#[allow(clippy::too_many_lines)]
	async fn init_or_resume<OuterCtx: OuterContext>(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		job_ctx: &impl JobContext<OuterCtx>,
		dispatcher: &JobTaskDispatcher,
	) -> Result<(), JobErrorOrDispatcherError<media_processor::Error>> {
		// if we don't have any pending task, then this is a fresh job
		if self.pending_tasks_on_resume.is_empty() {
			let location_id = self.location.id;
			let location_path = &*self.location_path;

			let iso_file_path = maybe_get_iso_file_path_from_sub_path::<media_processor::Error>(
				location_id,
				self.sub_path.as_ref(),
				&*self.location_path,
				job_ctx.db(),
			)
			.await?
			.map_or_else(
				|| {
					IsolatedFilePathData::new(location_id, location_path, location_path, true)
						.map_err(media_processor::Error::from)
				},
				Ok,
			)?;

			// First we will dispatch all tasks for media data extraction so we have a nice reporting
			let media_data_extraction_tasks_res = self
				.dispatch_media_data_extractor_tasks(&iso_file_path, dispatcher, job_ctx)
				.await;

			// Now we dispatch thumbnailer tasks
			let thumbnailer_tasks_res = self
				.dispatch_thumbnailer_tasks(
					&iso_file_path,
					self.regenerate_thumbnails,
					dispatcher,
					job_ctx,
				)
				.await;

			match (media_data_extraction_tasks_res, thumbnailer_tasks_res) {
				(Ok(media_data_extraction_task_handles), Ok(thumbnailer_task_handles)) => {
					pending_running_tasks.extend(
						media_data_extraction_task_handles
							.into_iter()
							.chain(thumbnailer_task_handles),
					);
				}

				(
					Ok(task_handles),
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::JobCanceled(e))),
				)
				| (
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::JobCanceled(e))),
					Ok(task_handles),
				) => {
					pending_running_tasks.extend(task_handles);
					return Err(JobErrorOrDispatcherError::Dispatcher(
						DispatcherError::JobCanceled(e),
					));
				}

				(
					Ok(task_handles),
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::Shutdown(tasks))),
				)
				| (
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::Shutdown(tasks))),
					Ok(task_handles),
				) => {
					self.tasks_for_shutdown.extend(tasks);
					pending_running_tasks.extend(task_handles);
				}

				(
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::Shutdown(
						media_data_extraction_tasks,
					))),
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::Shutdown(
						thumbnailer_tasks,
					))),
				) => {
					self.tasks_for_shutdown.extend(
						media_data_extraction_tasks
							.into_iter()
							.chain(thumbnailer_tasks),
					);
				}

				(
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::JobCanceled(e))),
					_,
				)
				| (
					_,
					Err(JobErrorOrDispatcherError::Dispatcher(DispatcherError::JobCanceled(e))),
				) => {
					return Err(JobErrorOrDispatcherError::Dispatcher(
						DispatcherError::JobCanceled(e),
					));
				}

				(Err(JobErrorOrDispatcherError::JobError(e)), _)
				| (_, Err(JobErrorOrDispatcherError::JobError(e))) => {
					return Err(e.into());
				}
			}
		} else {
			let updates = match self.phase {
				Phase::MediaDataExtraction => vec![
					ProgressUpdate::TaskCount(self.total_media_data_extraction_files),
					ProgressUpdate::CompletedTaskCount(
						self.metadata.media_data_metrics.extracted
							+ self.metadata.media_data_metrics.skipped,
					),
					ProgressUpdate::Phase(self.phase.to_string()),
					ProgressUpdate::Message(format!(
						"Preparing to process {} files in {} chunks",
						self.total_media_data_extraction_files,
						self.total_media_data_extraction_tasks
					)),
				],
				Phase::ThumbnailGeneration => vec![
					ProgressUpdate::TaskCount(self.total_thumbnailer_files),
					ProgressUpdate::CompletedTaskCount(
						self.metadata.thumbnailer_metrics_acc.generated
							+ self.metadata.thumbnailer_metrics_acc.skipped,
					),
					ProgressUpdate::Phase(self.phase.to_string()),
					ProgressUpdate::Message(format!(
						"Preparing to process {} files in {} chunks",
						self.total_thumbnailer_files, self.total_thumbnailer_tasks
					)),
				],
			};

			job_ctx.progress(updates).await;

			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));
		}

		Ok(())
	}

	async fn process_handles<OuterCtx: OuterContext>(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		job_ctx: &impl JobContext<OuterCtx>,
	) -> Option<Result<ReturnStatus, Error>> {
		while let Some(task) = pending_running_tasks.next().await {
			match task {
				Ok(TaskStatus::Done((task_id, TaskOutput::Out(out)))) => {
					self.process_task_output(task_id, out, job_ctx).await;
				}

				Ok(TaskStatus::Done((task_id, TaskOutput::Empty))) => {
					warn!(%task_id, "Task returned an empty output;");
				}

				Ok(TaskStatus::Shutdown(task)) => {
					self.tasks_for_shutdown.push(task);
				}

				Ok(TaskStatus::Error(e)) => {
					cancel_pending_tasks(pending_running_tasks).await;

					return Some(Err(e));
				}

				Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
					return Some(Ok(self.cancel_job(pending_running_tasks).await));
				}

				Err(TaskSystemError::TaskTimeout(task_id)) => {
					warn!(
						%task_id,
						"Thumbnailer task timed out, we will keep processing the rest of the tasks;",
					);
					self.errors.push(
						media_processor::NonCriticalMediaProcessorError::Thumbnailer(
							media_processor::NonCriticalThumbnailerError::TaskTimeout(task_id),
						)
						.into(),
					);
				}

				Err(e) => {
					error!(?e, "Task System error;");
					cancel_pending_tasks(pending_running_tasks).await;

					return Some(Err(e.into()));
				}
			}
		}

		None
	}

	async fn process_task_output<OuterCtx: OuterContext>(
		&mut self,
		task_id: TaskId,
		any_task_output: Box<dyn AnyTaskOutput>,
		job_ctx: &impl JobContext<OuterCtx>,
	) {
		if any_task_output.is::<media_data_extractor::Output>() {
			let media_data_extractor::Output {
				extracted,
				skipped,
				db_read_time,
				filtering_time,
				extraction_time,
				db_write_time,
				errors,
			} = *any_task_output.downcast().expect("just checked");

			self.metadata.media_data_metrics.extracted += extracted;
			self.metadata.media_data_metrics.skipped += skipped;
			self.metadata.media_data_metrics.mean_db_read_time += db_read_time;
			self.metadata.media_data_metrics.mean_filtering_time += filtering_time;
			self.metadata.media_data_metrics.mean_extraction_time += extraction_time;
			self.metadata.media_data_metrics.mean_db_write_time += db_write_time;
			self.metadata.media_data_metrics.total_successful_tasks += 1;

			if !errors.is_empty() {
				warn!(?errors, "Non critical errors while extracting media data;");
				self.errors.extend(errors);
			}

			debug!(
				"Processed ({}/{}) media data extraction tasks, took: {:?};",
				self.metadata.media_data_metrics.total_successful_tasks,
				self.total_media_data_extraction_tasks,
				db_read_time + filtering_time + extraction_time + db_write_time,
			);
			job_ctx
				.progress(vec![ProgressUpdate::CompletedTaskCount(
					self.metadata.media_data_metrics.extracted
						+ self.metadata.media_data_metrics.skipped,
				)])
				.await;

			if self.total_media_data_extraction_tasks
				== self.metadata.media_data_metrics.total_successful_tasks
			{
				debug!("All media data extraction tasks have been processed");

				self.phase = Phase::ThumbnailGeneration;

				job_ctx
					.progress(vec![
						ProgressUpdate::TaskCount(self.total_thumbnailer_files),
						ProgressUpdate::Phase(self.phase.to_string()),
						ProgressUpdate::Message(format!(
							"Waiting for processing of {} thumbnails in {} tasks",
							self.total_thumbnailer_files, self.total_thumbnailer_tasks
						)),
					])
					.await;
			}
		} else if any_task_output.is::<thumbnailer::Output>() {
			let thumbnailer::Output {
				generated,
				skipped,
				errors,
				total_time,
				mean_time_acc,
				std_dev_acc,
			} = *any_task_output.downcast().expect("just checked");

			self.metadata.thumbnailer_metrics_acc.generated += generated;
			self.metadata.thumbnailer_metrics_acc.skipped += skipped;
			self.metadata.thumbnailer_metrics_acc.mean_total_time += total_time;
			self.metadata.thumbnailer_metrics_acc.mean_time_acc += mean_time_acc;
			self.metadata.thumbnailer_metrics_acc.std_dev_acc += std_dev_acc;
			self.metadata.thumbnailer_metrics_acc.total_successful_tasks += 1;

			if !errors.is_empty() {
				warn!(?errors, "Non critical errors while generating thumbnails;");
				self.errors.extend(errors);
			}

			debug!(
				"Processed ({}/{}) thumbnailer tasks, took: {total_time:?}",
				self.metadata.thumbnailer_metrics_acc.total_successful_tasks,
				self.total_thumbnailer_tasks
			);

			if matches!(self.phase, Phase::ThumbnailGeneration) {
				job_ctx
					.progress(vec![ProgressUpdate::CompletedTaskCount(
						self.metadata.thumbnailer_metrics_acc.generated
							+ self.metadata.thumbnailer_metrics_acc.skipped,
					)])
					.await;
			}

		// if self.total_thumbnailer_tasks
		// 	== self.metadata.thumbnailer_metrics_acc.total_successful_tasks
		// {
		// 	debug!("All thumbnailer tasks have been processed");

		// 	self.phase = Phase::LabelsGeneration;

		// 	ctx.progress(vec![
		// 		ProgressUpdate::TaskCount(self.total_thumbnailer_files),
		// 		ProgressUpdate::Phase(self.phase.to_string()),
		// 		ProgressUpdate::Message(format!(
		// 			"Waiting for processing of {} labels in {} tasks",
		// 			self.total_labeller_files, self.total_labeller_tasks
		// 		)),
		// 	]).await;
		// }
		} else {
			unreachable!("Unexpected task output type: <id='{task_id}'>");
		}
	}

	async fn cancel_job(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
	) -> ReturnStatus {
		cancel_pending_tasks(pending_running_tasks).await;

		ReturnStatus::Canceled(
			JobReturn::builder()
				.with_metadata(mem::take(&mut self.metadata))
				.with_non_critical_errors(mem::take(&mut self.errors))
				.build(),
		)
	}

	#[instrument(skip_all, fields(parent_iso_file_path = %parent_iso_file_path.as_ref().display()))]
	async fn dispatch_media_data_extractor_tasks<OuterCtx: OuterContext>(
		&mut self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		dispatcher: &JobTaskDispatcher,
		job_ctx: &impl JobContext<OuterCtx>,
	) -> Result<Vec<TaskHandle<Error>>, JobErrorOrDispatcherError<media_processor::Error>> {
		let db = job_ctx.db();
		let sync = job_ctx.sync();

		let (extract_exif_file_paths, extract_ffmpeg_file_paths) = (
			get_all_children_files_by_extensions(
				parent_iso_file_path,
				&helpers::exif_media_data::AVAILABLE_EXTENSIONS,
				db,
			),
			get_all_children_files_by_extensions(
				parent_iso_file_path,
				&helpers::ffmpeg_media_data::AVAILABLE_EXTENSIONS,
				db,
			),
		)
			.try_join()
			.await?;

		let files_count = (extract_exif_file_paths.len() + extract_ffmpeg_file_paths.len()) as u64;

		let tasks = extract_exif_file_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(Iterator::collect::<Vec<_>>)
			.map(|chunked_file_paths| {
				tasks::MediaDataExtractor::new_exif(
					&chunked_file_paths,
					parent_iso_file_path.location_id(),
					Arc::clone(&self.location_path),
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
							Arc::clone(&self.location_path),
							Arc::clone(db),
							sync.clone(),
						)
					})
					.map(IntoTask::into_task),
			)
			.collect::<Vec<_>>();

		trace!(
			tasks_count = tasks.len(),
			%files_count,
			"Dispatching media data extraction tasks;",
		);

		self.total_media_data_extraction_files = files_count;
		self.total_media_data_extraction_tasks = tasks.len() as u64;

		job_ctx
			.progress(vec![
				ProgressUpdate::TaskCount(self.total_media_data_extraction_files),
				ProgressUpdate::Phase(self.phase.to_string()),
				ProgressUpdate::Message(format!(
					"Preparing to process {} files in {} chunks",
					self.total_media_data_extraction_files, self.total_media_data_extraction_tasks
				)),
			])
			.await;

		dispatcher
			.dispatch_many_boxed(tasks)
			.await
			.map_err(Into::into)
	}

	async fn dispatch_thumbnailer_tasks(
		&mut self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		should_regenerate: bool,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl OuterContext,
	) -> Result<Vec<TaskHandle<Error>>, JobErrorOrDispatcherError<media_processor::Error>> {
		let thumbnails_directory_path =
			Arc::new(ctx.get_data_directory().join(THUMBNAIL_CACHE_DIR_NAME));
		let location_id = parent_iso_file_path.location_id();
		let library_id = ctx.id();
		let db = ctx.db();
		let reporter: Arc<dyn NewThumbnailReporter> =
			Arc::new(NewThumbnailsReporter { ctx: ctx.clone() });

		let priority_file_paths = get_direct_children_files_by_extensions(
			parent_iso_file_path,
			&helpers::thumbnailer::ALL_THUMBNAILABLE_EXTENSIONS,
			db,
		)
		.await?;

		let priority_file_path_ids = priority_file_paths
			.iter()
			.map(|file_path| file_path.id)
			.collect::<HashSet<_>>();

		let mut file_paths = get_all_children_files_by_extensions(
			parent_iso_file_path,
			&helpers::thumbnailer::ALL_THUMBNAILABLE_EXTENSIONS,
			db,
		)
		.await?;

		file_paths.retain(|file_path| !priority_file_path_ids.contains(&file_path.id));

		if priority_file_path_ids.is_empty() && file_paths.is_empty() {
			return Ok(Vec::new());
		}

		let thumbs_count = (priority_file_paths.len() + file_paths.len()) as u64;

		let priority_tasks = priority_file_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| {
				tasks::Thumbnailer::new_indexed(
					Arc::clone(&thumbnails_directory_path),
					&chunk.collect::<Vec<_>>(),
					(location_id, &self.location_path),
					library_id,
					should_regenerate,
					true,
					Arc::clone(&reporter),
				)
			})
			.map(IntoTask::into_task)
			.collect::<Vec<_>>();

		let non_priority_tasks = file_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| {
				tasks::Thumbnailer::new_indexed(
					Arc::clone(&thumbnails_directory_path),
					&chunk.collect::<Vec<_>>(),
					(location_id, &self.location_path),
					library_id,
					should_regenerate,
					false,
					Arc::clone(&reporter),
				)
			})
			.map(IntoTask::into_task)
			.collect::<Vec<_>>();

		debug!(
			%thumbs_count,
			priority_tasks_count = priority_tasks.len(),
			non_priority_tasks_count = non_priority_tasks.len(),
			"Dispatching thumbnails to be processed;",
		);

		self.total_thumbnailer_tasks = (priority_tasks.len() + non_priority_tasks.len()) as u64;
		self.total_thumbnailer_files = thumbs_count;

		dispatcher
			.dispatch_many_boxed(priority_tasks.into_iter().chain(non_priority_tasks))
			.await
			.map_err(Into::into)
	}
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Metadata {
	media_data_metrics: MediaExtractorMetrics,
	thumbnailer_metrics_acc: ThumbnailerMetricsAccumulator,
}

impl From<Metadata> for Vec<ReportOutputMetadata> {
	fn from(
		Metadata {
			media_data_metrics,
			thumbnailer_metrics_acc: thumbnailer_metrics_accumulator,
		}: Metadata,
	) -> Self {
		let thumbnailer_metrics = ThumbnailerMetrics::from(thumbnailer_metrics_accumulator);

		vec![
			ReportOutputMetadata::MediaProcessor {
				media_data_extracted: u64_to_frontend(media_data_metrics.extracted),
				media_data_skipped: u64_to_frontend(media_data_metrics.skipped),
				thumbnails_generated: u64_to_frontend(thumbnailer_metrics.generated),
				thumbnails_skipped: u64_to_frontend(thumbnailer_metrics.skipped),
			},
			ReportOutputMetadata::Metrics(HashMap::from([
				//
				// Media data extractor
				//
				(
					"media_data_extraction_metrics".into(),
					json!(media_data_metrics),
				),
				//
				// Thumbnailer
				//
				("thumbnailer_metrics".into(), json!(thumbnailer_metrics)),
			])),
		]
	}
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct MediaExtractorMetrics {
	extracted: u64,
	skipped: u64,
	mean_db_read_time: Duration,
	mean_filtering_time: Duration,
	mean_extraction_time: Duration,
	mean_db_write_time: Duration,
	total_successful_tasks: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ThumbnailerMetricsAccumulator {
	generated: u64,
	skipped: u64,
	mean_total_time: Duration,
	mean_time_acc: f64,
	std_dev_acc: f64,
	total_successful_tasks: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ThumbnailerMetrics {
	generated: u64,
	skipped: u64,
	mean_total_time: Duration,
	mean_generation_time: Duration,
	std_dev: Duration,
	total_successful_tasks: u64,
}

impl From<ThumbnailerMetricsAccumulator> for ThumbnailerMetrics {
	fn from(
		ThumbnailerMetricsAccumulator {
			generated,
			skipped,
			mean_total_time,
			mean_time_acc: mean_generation_time_acc,
			std_dev_acc,
			total_successful_tasks,
		}: ThumbnailerMetricsAccumulator,
	) -> Self {
		if generated + skipped == 0 {
			return Self {
				generated,
				skipped,
				mean_total_time,
				mean_generation_time: Duration::ZERO,
				std_dev: Duration::ZERO,
				total_successful_tasks,
			};
		}

		#[allow(clippy::cast_precision_loss)]
		// SAFETY: we're probably won't have 2^52 thumbnails being generated on a single job for this cast to have
		// a precision loss issue
		let total = (generated + skipped) as f64;
		let mean_generation_time = mean_generation_time_acc / total;

		let std_dev = if generated > 1 {
			Duration::from_secs_f64(
				(mean_generation_time.mul_add(-mean_generation_time, std_dev_acc / total)).sqrt(),
			)
		} else {
			Duration::ZERO
		};

		Self {
			generated,
			skipped,
			mean_total_time,
			mean_generation_time: Duration::from_secs_f64(if generated > 1 {
				mean_generation_time
			} else {
				mean_generation_time_acc
			}),
			std_dev,
			total_successful_tasks,
		}
	}
}

async fn get_all_children_files_by_extensions(
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
	db: &PrismaClient,
) -> Result<Vec<file_path_for_media_processor::Data>, media_processor::Error> {
	// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
	// We have no data coming from the user, so this is sql injection safe
	let unique_by_object_id = db
		._query_raw::<RawFilePathForMediaProcessor>(raw!(
			&format!(
				"SELECT
				file_path.id,
				file_path.materialized_path,
				file_path.is_dir,
				file_path.name,
				file_path.extension,
				file_path.cas_id,
				object.id as 'object_id',
				object.pub_id as 'object_pub_id'
			FROM file_path
			INNER JOIN object ON object.id = file_path.object_id
			WHERE
				file_path.location_id={{}}
				AND file_path.cas_id IS NOT NULL
				AND LOWER(file_path.extension) IN ({})
				AND file_path.materialized_path LIKE {{}}
			ORDER BY materialized_path ASC, name ASC",
				// Ordering by materialized_path so we can prioritize processing the first files
				// in the above part of the directories tree
				extensions
					.iter()
					.map(|ext| format!("LOWER('{ext}')"))
					.collect::<Vec<_>>()
					.join(",")
			),
			PrismaValue::Int(parent_iso_file_path.location_id()),
			PrismaValue::String(format!(
				"{}%",
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory")
			))
		))
		.exec()
		.await?
		.into_iter()
		.map(|raw_file_path| (raw_file_path.object_id, raw_file_path))
		.collect::<HashMap<_, _>>();

	Ok(unique_by_object_id.into_values().map(Into::into).collect())
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,
	regenerate_thumbnails: bool,

	total_media_data_extraction_files: u64,
	total_media_data_extraction_tasks: u64,
	total_thumbnailer_tasks: u64,
	total_thumbnailer_files: u64,

	phase: Phase,

	metadata: Metadata,

	errors: Vec<crate::NonCriticalError>,

	tasks_for_shutdown_bytes: Option<SerializedTasks>,
}

impl<OuterCtx: OuterContext> SerializableJob<OuterCtx> for MediaProcessor {
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let Self {
			location,
			location_path,
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_files,
			total_media_data_extraction_tasks,
			total_thumbnailer_tasks,
			total_thumbnailer_files,
			phase,
			metadata,
			errors,
			tasks_for_shutdown,
			..
		} = self;

		let serialized_tasks = tasks_for_shutdown
			.into_iter()
			.map(|task| async move {
				if task.is::<tasks::MediaDataExtractor>() {
					task.downcast::<tasks::MediaDataExtractor>()
						.expect("just checked")
						.serialize()
						.await
						.map(|bytes| (TaskKind::MediaDataExtractor, bytes))
				} else if task.is::<tasks::Thumbnailer>() {
					task.downcast::<tasks::Thumbnailer>()
						.expect("just checked")
						.serialize()
						.await
						.map(|bytes| (TaskKind::Thumbnailer, bytes))
				} else {
					unreachable!("Unexpected task type: <task='{task:#?}'>")
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		let tasks_for_shutdown_bytes = if serialized_tasks.is_empty() {
			None
		} else {
			Some(SerializedTasks(rmp_serde::to_vec_named(&serialized_tasks)?))
		};

		rmp_serde::to_vec_named(&SaveState {
			location,
			location_path,
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_files,
			total_media_data_extraction_tasks,
			total_thumbnailer_tasks,
			total_thumbnailer_files,
			phase,
			metadata,
			errors,
			tasks_for_shutdown_bytes,
		})
		.map(Some)
	}

	async fn deserialize(
		serialized_job: &[u8],
		_: &OuterCtx,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let SaveState {
			location,
			location_path,
			sub_path,
			regenerate_thumbnails,
			total_media_data_extraction_files,
			total_media_data_extraction_tasks,
			total_thumbnailer_tasks,
			total_thumbnailer_files,
			phase,
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
				total_media_data_extraction_files,
				total_media_data_extraction_tasks,
				total_thumbnailer_tasks,
				total_thumbnailer_files,
				phase,
				metadata,
				errors,
				pending_tasks_on_resume: Vec::new(),
				tasks_for_shutdown: Vec::new(),
			},
			tasks_for_shutdown_bytes,
		)))
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
