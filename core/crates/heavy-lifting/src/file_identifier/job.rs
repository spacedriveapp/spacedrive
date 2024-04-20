use crate::{
	job_system::{
		job::{Job, JobReturn, JobTaskDispatcher, ReturnStatus},
		report::ReportOutputMetadata,
		utils::cancel_pending_tasks,
		SerializableJob, SerializedTasks,
	},
	utils::sub_path::maybe_get_iso_file_path_from_sub_path,
	Error, JobContext, JobName, LocationScanState, NonCriticalJobError, ProgressUpdate,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_file_identifier;

use sd_prisma::prisma::{file_path, location, SortOrder};
use sd_task_system::{
	AnyTaskOutput, IntoTask, SerializableTask, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskOutput, TaskStatus,
};
use sd_utils::db::maybe_missing;

use std::{
	collections::HashMap,
	hash::{Hash, Hasher},
	mem,
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::TryJoin;
use prisma_client_rust::or;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::Instant;
use tracing::warn;

use super::{
	tasks::{
		ExtractFileMetadataTask, ExtractFileMetadataTaskOutput, ObjectProcessorTask,
		ObjectProcessorTaskMetrics,
	},
	FileIdentifierError, CHUNK_SIZE,
};

#[derive(Debug)]
pub struct FileIdentifierJob {
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,

	metadata: Metadata,

	errors: Vec<NonCriticalJobError>,

	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Box<dyn Task<Error>>>,
}

impl Hash for FileIdentifierJob {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

impl Job for FileIdentifierJob {
	const NAME: JobName = JobName::FileIdentifier;

	async fn resume_tasks(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext,
		SerializedTasks(serialized_tasks): SerializedTasks,
	) -> Result<(), Error> {
		self.pending_tasks_on_resume = dispatcher
			.dispatch_many_boxed(
				rmp_serde::from_slice::<Vec<(TaskKind, Vec<u8>)>>(&serialized_tasks)
					.map_err(FileIdentifierError::from)?
					.into_iter()
					.map(|(task_kind, task_bytes)| async move {
						match task_kind {
							TaskKind::ExtractFileMetadata => {
								<ExtractFileMetadataTask as SerializableTask<Error>>::deserialize(
									&task_bytes,
									(),
								)
								.await
								.map(IntoTask::into_task)
							}

							TaskKind::ObjectProcessor => ObjectProcessorTask::deserialize(
								&task_bytes,
								(Arc::clone(ctx.db()), Arc::clone(ctx.sync())),
							)
							.await
							.map(IntoTask::into_task),
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await
					.map_err(FileIdentifierError::from)?,
			)
			.await;

		Ok(())
	}

	async fn run(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: impl JobContext,
	) -> Result<ReturnStatus, Error> {
		let mut pending_running_tasks = FuturesUnordered::new();

		self.init_or_resume(&mut pending_running_tasks, &ctx, &dispatcher)
			.await?;

		while let Some(task) = pending_running_tasks.next().await {
			match task {
				Ok(TaskStatus::Done((task_id, TaskOutput::Out(out)))) => {
					if let Some(new_object_processor_task) = self
						.process_task_output(task_id, out, &ctx, &dispatcher)
						.await
					{
						pending_running_tasks.push(new_object_processor_task);
					};
				}

				Ok(TaskStatus::Done((task_id, TaskOutput::Empty))) => {
					warn!("Task <id='{task_id}'> returned an empty output");
				}

				Ok(TaskStatus::Shutdown(task)) => {
					self.tasks_for_shutdown.push(task);
				}

				Ok(TaskStatus::Error(e)) => {
					cancel_pending_tasks(&pending_running_tasks).await;

					return Err(e);
				}

				Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
					cancel_pending_tasks(&pending_running_tasks).await;

					return Ok(ReturnStatus::Canceled);
				}

				Err(e) => {
					cancel_pending_tasks(&pending_running_tasks).await;

					return Err(e.into());
				}
			}
		}

		if !self.tasks_for_shutdown.is_empty() {
			return Ok(ReturnStatus::Shutdown(self.serialize().await));
		}

		// From this point onward, we are done with the job and it can't be interrupted anymore
		let Self {
			location,
			metadata,
			errors,
			..
		} = self;

		ctx.db()
			.location()
			.update(
				location::id::equals(location.id),
				vec![location::scan_state::set(
					LocationScanState::FilesIdentified as i32,
				)],
			)
			.exec()
			.await
			.map_err(FileIdentifierError::from)?;

		Ok(ReturnStatus::Completed(
			JobReturn::builder()
				.with_metadata(metadata)
				.with_non_critical_errors(errors)
				.build(),
		))
	}
}

impl FileIdentifierJob {
	pub fn new(
		location: location::Data,
		sub_path: Option<PathBuf>,
	) -> Result<Self, FileIdentifierError> {
		Ok(Self {
			location_path: maybe_missing(&location.path, "location.path")
				.map(PathBuf::from)
				.map(Arc::new)?,
			location: Arc::new(location),
			sub_path,
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
	) -> Result<(), FileIdentifierError> {
		// if we don't have any pending task, then this is a fresh job
		if self.pending_tasks_on_resume.is_empty() {
			let db = job_ctx.db();
			let maybe_sub_iso_file_path = maybe_get_iso_file_path_from_sub_path(
				self.location.id,
				&self.sub_path,
				&*self.location_path,
				db,
			)
			.await?;

			let mut orphans_count = 0;
			let mut last_orphan_file_path_id = None;

			let start = Instant::now();

			loop {
				#[allow(clippy::cast_possible_wrap)]
				// SAFETY: we know that CHUNK_SIZE is a valid i64
				let orphan_paths = db
					.file_path()
					.find_many(orphan_path_filters(
						self.location.id,
						last_orphan_file_path_id,
						&maybe_sub_iso_file_path,
					))
					.order_by(file_path::id::order(SortOrder::Asc))
					.take(CHUNK_SIZE as i64)
					.select(file_path_for_file_identifier::select())
					.exec()
					.await?;

				if orphan_paths.is_empty() {
					break;
				}

				orphans_count += orphan_paths.len() as u64;
				last_orphan_file_path_id =
					Some(orphan_paths.last().expect("orphan_paths is not empty").id);

				job_ctx.progress(vec![
					ProgressUpdate::TaskCount(orphans_count),
					ProgressUpdate::Message(format!("{orphans_count} files to be identified")),
				]);

				pending_running_tasks.push(
					dispatcher
						.dispatch(ExtractFileMetadataTask::new_deep(
							Arc::clone(&self.location),
							Arc::clone(&self.location_path),
							orphan_paths,
						))
						.await,
				);
			}

			self.metadata.seeking_orphans_time = start.elapsed();
			self.metadata.total_found_orphans = orphans_count;
		} else {
			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));
		}

		Ok(())
	}

	/// Process output of tasks, according to the downcasted output type
	///
	/// # Panics
	/// Will panic if another task type is added in the job, but this function wasn't updated to handle it
	///
	async fn process_task_output(
		&mut self,
		task_id: TaskId,
		any_task_output: Box<dyn AnyTaskOutput>,
		job_ctx: &impl JobContext,
		dispatcher: &JobTaskDispatcher,
	) -> Option<TaskHandle<Error>> {
		if any_task_output.is::<ExtractFileMetadataTaskOutput>() {
			return self
				.process_extract_file_metadata_output(
					*any_task_output
						.downcast::<ExtractFileMetadataTaskOutput>()
						.expect("just checked"),
					job_ctx,
					dispatcher,
				)
				.await;
		} else if any_task_output.is::<ObjectProcessorTaskMetrics>() {
			self.process_object_processor_output(
				*any_task_output
					.downcast::<ObjectProcessorTaskMetrics>()
					.expect("just checked"),
				job_ctx,
			);
		} else {
			unreachable!("Unexpected task output type: <id='{task_id}'>");
		}

		None
	}

	async fn process_extract_file_metadata_output(
		&mut self,
		ExtractFileMetadataTaskOutput {
			identified_files,
			extract_metadata_time,
			errors,
		}: ExtractFileMetadataTaskOutput,
		job_ctx: &impl JobContext,
		dispatcher: &JobTaskDispatcher,
	) -> Option<TaskHandle<Error>> {
		self.metadata.extract_metadata_time += extract_metadata_time;
		self.errors.extend(errors);

		if identified_files.is_empty() {
			self.metadata.completed_tasks += 1;

			job_ctx.progress(vec![ProgressUpdate::CompletedTaskCount(
				self.metadata.completed_tasks,
			)]);

			None
		} else {
			job_ctx.progress_msg(format!("Identified {} files", identified_files.len()));

			Some(
				dispatcher
					.dispatch(ObjectProcessorTask::new_deep(
						identified_files,
						Arc::clone(job_ctx.db()),
						Arc::clone(job_ctx.sync()),
					))
					.await,
			)
		}
	}

	fn process_object_processor_output(
		&mut self,
		ObjectProcessorTaskMetrics {
			assign_cas_ids_time,
			fetch_existing_objects_time,
			assign_to_existing_object_time,
			create_object_time,
			created_objects_count,
			linked_objects_count,
		}: ObjectProcessorTaskMetrics,
		job_ctx: &impl JobContext,
	) {
		self.metadata.assign_cas_ids_time += assign_cas_ids_time;
		self.metadata.fetch_existing_objects_time += fetch_existing_objects_time;
		self.metadata.assign_to_existing_object_time += assign_to_existing_object_time;
		self.metadata.create_object_time += create_object_time;
		self.metadata.created_objects_count += created_objects_count;
		self.metadata.linked_objects_count += linked_objects_count;

		self.metadata.completed_tasks += 1;

		job_ctx.progress(vec![
			ProgressUpdate::CompletedTaskCount(self.metadata.completed_tasks),
			ProgressUpdate::Message(format!(
				"Processed {} of {} objects",
				self.metadata.created_objects_count + self.metadata.linked_objects_count,
				self.metadata.total_found_orphans
			)),
		]);
	}
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TaskKind {
	ExtractFileMetadata,
	ObjectProcessor,
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,

	metadata: Metadata,

	errors: Vec<NonCriticalJobError>,

	tasks_for_shutdown_bytes: Option<SerializedTasks>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
	extract_metadata_time: Duration,
	assign_cas_ids_time: Duration,
	fetch_existing_objects_time: Duration,
	assign_to_existing_object_time: Duration,
	create_object_time: Duration,
	seeking_orphans_time: Duration,
	total_found_orphans: u64,
	created_objects_count: u64,
	linked_objects_count: u64,
	completed_tasks: u64,
}

impl From<Metadata> for ReportOutputMetadata {
	fn from(value: Metadata) -> Self {
		Self::Metrics(HashMap::from([
			(
				"extract_metadata_time".into(),
				json!(value.extract_metadata_time),
			),
			(
				"assign_cas_ids_time".into(),
				json!(value.assign_cas_ids_time),
			),
			(
				"fetch_existing_objects_time".into(),
				json!(value.fetch_existing_objects_time),
			),
			(
				"assign_to_existing_object_time".into(),
				json!(value.assign_to_existing_object_time),
			),
			("create_object_time".into(), json!(value.create_object_time)),
			(
				"seeking_orphans_time".into(),
				json!(value.seeking_orphans_time),
			),
			(
				"total_found_orphans".into(),
				json!(value.total_found_orphans),
			),
			(
				"created_objects_count".into(),
				json!(value.created_objects_count),
			),
			(
				"linked_objects_count".into(),
				json!(value.linked_objects_count),
			),
			("total_tasks".into(), json!(value.completed_tasks)),
		]))
	}
}

impl SerializableJob for FileIdentifierJob {
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let Self {
			location,
			location_path,
			sub_path,
			metadata,
			errors,
			tasks_for_shutdown,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			location,
			location_path,
			sub_path,
			metadata,
			tasks_for_shutdown_bytes: Some(SerializedTasks(rmp_serde::to_vec_named(
				&tasks_for_shutdown
					.into_iter()
					.map(|task| async move {
						if task.is::<ExtractFileMetadataTask>() {
							SerializableTask::serialize(
								*task
									.downcast::<ExtractFileMetadataTask>()
									.expect("just checked"),
							)
							.await
							.map(|bytes| (TaskKind::ExtractFileMetadata, bytes))
						} else if task.is::<ObjectProcessorTask>() {
							task.downcast::<ObjectProcessorTask>()
								.expect("just checked")
								.serialize()
								.await
								.map(|bytes| (TaskKind::ObjectProcessor, bytes))
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
		_: &impl JobContext,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let SaveState {
			location,
			location_path,
			sub_path,
			metadata,

			errors,
			tasks_for_shutdown_bytes,
		} = rmp_serde::from_slice::<SaveState>(serialized_job)?;

		Ok(Some((
			Self {
				location,
				location_path,
				sub_path,
				metadata,
				errors,
				pending_tasks_on_resume: Vec::new(),
				tasks_for_shutdown: Vec::new(),
			},
			tasks_for_shutdown_bytes,
		)))
	}
}

fn orphan_path_filters(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	maybe_sub_iso_file_path: &Option<IsolatedFilePathData<'_>>,
) -> Vec<file_path::WhereParam> {
	sd_utils::chain_optional_iter(
		[
			or!(
				file_path::object_id::equals(None),
				file_path::cas_id::equals(None)
			),
			file_path::is_dir::equals(Some(false)),
			file_path::location_id::equals(Some(location_id)),
			file_path::size_in_bytes_bytes::not(Some(0u64.to_be_bytes().to_vec())),
		],
		[
			// this is a workaround for the cursor not working properly
			file_path_id.map(file_path::id::gte),
			maybe_sub_iso_file_path.as_ref().map(|sub_iso_file_path| {
				file_path::materialized_path::starts_with(
					sub_iso_file_path
						.materialized_path_for_children()
						.expect("sub path iso_file_path must be a directory"),
				)
			}),
		],
	)
}
