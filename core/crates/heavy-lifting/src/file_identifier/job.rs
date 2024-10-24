use crate::{
	file_identifier,
	job_system::{
		job::{Job, JobReturn, JobTaskDispatcher, ReturnStatus},
		report::ReportOutputMetadata,
		utils::cancel_pending_tasks,
		DispatcherError, JobErrorOrDispatcherError, SerializableJob, SerializedTasks,
	},
	utils::sub_path::maybe_get_iso_file_path_from_sub_path,
	Error, JobContext, JobName, LocationScanState, NonCriticalError, OuterContext, ProgressUpdate,
	UpdateEvent,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::{file_path_for_file_identifier, CasId};

use sd_prisma::{
	prisma::{device, file_path, location, SortOrder},
	prisma_sync,
};
use sd_sync::{sync_db_not_null_entry, OperationFactory};
use sd_task_system::{
	AnyTaskOutput, IntoTask, SerializableTask, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskOutput, TaskStatus,
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
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::Instant;
use tracing::{debug, instrument, trace, warn, Level};

use super::{
	accumulate_file_paths_by_cas_id, dispatch_object_processor_tasks, orphan_path_filters_deep,
	orphan_path_filters_shallow,
	tasks::{self, identifier, object_processor, FilePathToCreateOrLinkObject},
	CHUNK_SIZE,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
enum Phase {
	SearchingOrphansWithPriority,
	SearchingOrphans,
	IdentifyingFiles,
	ProcessingObjects,
}

impl Default for Phase {
	fn default() -> Self {
		Self::SearchingOrphansWithPriority
	}
}

impl fmt::Display for Phase {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::SearchingOrphans | Self::SearchingOrphansWithPriority => {
				write!(f, "searching_orphans")
			}
			Self::IdentifyingFiles => write!(f, "identifying_files"),
			Self::ProcessingObjects => write!(f, "processing_objects"),
		}
	}
}

impl From<Phase> for String {
	fn from(phase: Phase) -> Self {
		phase.to_string()
	}
}

#[derive(Debug)]
pub struct FileIdentifier {
	// Received arguments
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,

	// Inner state
	file_paths_accumulator: HashMap<CasId<'static>, Vec<FilePathToCreateOrLinkObject>>,
	file_paths_ids_with_priority: HashSet<file_path::id::Type>,
	last_orphan_file_path_id: Option<i32>,

	// Job control
	phase: Phase,

	// Run data
	metadata: Metadata,
	errors: Vec<NonCriticalError>,

	// On shutdown data
	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Box<dyn Task<Error>>>,
}

impl Hash for FileIdentifier {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

impl Job for FileIdentifier {
	const NAME: JobName = JobName::FileIdentifier;

	async fn resume_tasks<OuterCtx: OuterContext>(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext<OuterCtx>,
		SerializedTasks(serialized_tasks): SerializedTasks,
	) -> Result<(), Error> {
		if let Ok(tasks) = dispatcher
			.dispatch_many_boxed(
				rmp_serde::from_slice::<Vec<(TaskKind, Vec<u8>)>>(&serialized_tasks)
					.map_err(file_identifier::Error::from)?
					.into_iter()
					.map(|(task_kind, task_bytes)| async move {
						match task_kind {
							TaskKind::Identifier => tasks::Identifier::deserialize(
								&task_bytes,
								(Arc::clone(ctx.db()), ctx.sync().clone()),
							)
							.await
							.map(IntoTask::into_task),

							TaskKind::ObjectProcessor => tasks::ObjectProcessor::deserialize(
								&task_bytes,
								(Arc::clone(ctx.db()), ctx.sync().clone()),
							)
							.await
							.map(IntoTask::into_task),
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await
					.map_err(file_identifier::Error::from)?,
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
			location_path = %self.location_path.display(),
			sub_path = ?self.sub_path.as_ref().map(|path| path.display()),
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

		let device_pub_id = &ctx.sync().device_pub_id;
		let device_id = ctx
			.db()
			.device()
			.find_unique(device::pub_id::equals(device_pub_id.to_db()))
			.exec()
			.await
			.map_err(file_identifier::Error::from)?
			.ok_or(file_identifier::Error::DeviceNotFound(
				device_pub_id.clone(),
			))?
			.id;

		match self
			.init_or_resume(&mut pending_running_tasks, &ctx, device_id, &dispatcher)
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

		while let Some(task) = pending_running_tasks.next().await {
			match task {
				Ok(TaskStatus::Done((task_id, TaskOutput::Out(out)))) => {
					match self
						.process_task_output(task_id, out, &ctx, device_id, &dispatcher)
						.await
					{
						Ok(tasks) => pending_running_tasks.extend(tasks),
						Err(DispatcherError::JobCanceled(_)) => {
							return Ok(self.cancel_job(&mut pending_running_tasks).await);
						}
						Err(DispatcherError::Shutdown(tasks)) => {
							self.tasks_for_shutdown.extend(tasks);
						}
					}
				}

				Ok(TaskStatus::Done((task_id, TaskOutput::Empty))) => {
					warn!(%task_id, "Task returned an empty output");
				}

				Ok(TaskStatus::Shutdown(task)) => {
					self.tasks_for_shutdown.push(task);
				}

				Ok(TaskStatus::Error(e)) => {
					cancel_pending_tasks(&mut pending_running_tasks).await;

					return Err(e);
				}

				Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
					return Ok(self.cancel_job(&mut pending_running_tasks).await);
				}

				Err(e) => {
					cancel_pending_tasks(&mut pending_running_tasks).await;

					return Err(e.into());
				}
			}
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

		let (sync_param, db_param) = sync_db_not_null_entry!(
			LocationScanState::FilesIdentified as i32,
			location::scan_state
		);

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
			.map_err(file_identifier::Error::from)?;

		Ok(ReturnStatus::Completed(
			JobReturn::builder()
				.with_metadata(metadata)
				.with_non_critical_errors(errors)
				.build(),
		))
	}
}

impl FileIdentifier {
	pub fn new(
		location: location::Data,
		sub_path: Option<PathBuf>,
	) -> Result<Self, file_identifier::Error> {
		Ok(Self {
			location_path: maybe_missing(&location.path, "location.path")
				.map(PathBuf::from)
				.map(Arc::new)?,
			location: Arc::new(location),
			sub_path,
			file_paths_accumulator: HashMap::new(),
			file_paths_ids_with_priority: HashSet::new(),
			last_orphan_file_path_id: None,
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
		ctx: &impl JobContext<OuterCtx>,
		device_id: device::id::Type,
		dispatcher: &JobTaskDispatcher,
	) -> Result<(), JobErrorOrDispatcherError<file_identifier::Error>> {
		// if we don't have any pending task, then this is a fresh job
		let db = ctx.db();
		let maybe_sub_iso_file_path =
			maybe_get_iso_file_path_from_sub_path::<file_identifier::Error>(
				self.location.id,
				self.sub_path.as_ref(),
				&*self.location_path,
				db,
			)
			.await?;

		let start = Instant::now();

		let location_root_iso_file_path = IsolatedFilePathData::new(
			self.location.id,
			&*self.location_path,
			&*self.location_path,
			true,
		)
		.map_err(file_identifier::Error::from)?;

		if self.pending_tasks_on_resume.is_empty() {
			ctx.progress([ProgressUpdate::phase(self.phase)]).await;

			// First we dispatch some shallow priority tasks to quickly identify orphans in the location
			// root directory or in the desired sub-path
			self.dispatch_priority_identifier_tasks(
				maybe_sub_iso_file_path
					.as_ref()
					.unwrap_or(&location_root_iso_file_path),
				ctx,
				device_id,
				dispatcher,
				pending_running_tasks,
			)
			.await?;

			self.phase = Phase::SearchingOrphans;
			// Resetting the last orphan file path id for deep search
			self.last_orphan_file_path_id = None;

			self.dispatch_deep_identifier_tasks(
				maybe_sub_iso_file_path.as_ref(),
				ctx,
				device_id,
				dispatcher,
				pending_running_tasks,
			)
			.await?;

			self.last_orphan_file_path_id = None;
			self.phase = Phase::IdentifyingFiles;

			ctx.progress(vec![
				ProgressUpdate::TaskCount(u64::from(self.metadata.total_identifier_tasks)),
				ProgressUpdate::phase(self.phase),
				ProgressUpdate::Message(format!(
					"{} files to be identified",
					self.metadata.total_found_orphans
				)),
			])
			.await;

			self.metadata.seeking_orphans_time = start.elapsed();
		} else {
			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));

			// For these 2 phases, we need to keep dispatching tasks until we have no more orphans to identify
			// as we could have receive a shutdown command before being able to run through all orphans
			match self.phase {
				Phase::SearchingOrphansWithPriority => {
					self.dispatch_priority_identifier_tasks(
						maybe_sub_iso_file_path
							.as_ref()
							.unwrap_or(&location_root_iso_file_path),
						ctx,
						device_id,
						dispatcher,
						pending_running_tasks,
					)
					.await?;

					self.phase = Phase::SearchingOrphans;
					// Resetting the last orphan file path id for deep search
					self.last_orphan_file_path_id = None;

					self.dispatch_deep_identifier_tasks(
						maybe_sub_iso_file_path.as_ref(),
						ctx,
						device_id,
						dispatcher,
						pending_running_tasks,
					)
					.await?;

					self.last_orphan_file_path_id = None;
					self.phase = Phase::IdentifyingFiles;
				}

				Phase::SearchingOrphans => {
					self.dispatch_deep_identifier_tasks(
						maybe_sub_iso_file_path.as_ref(),
						ctx,
						device_id,
						dispatcher,
						pending_running_tasks,
					)
					.await?;

					self.last_orphan_file_path_id = None;
					self.phase = Phase::IdentifyingFiles;
				}

				_ => {}
			}

			ctx.progress(vec![
				ProgressUpdate::TaskCount(if matches!(self.phase, Phase::IdentifyingFiles) {
					u64::from(self.metadata.total_identifier_tasks)
				} else {
					u64::from(self.metadata.total_object_processor_tasks)
				}),
				ProgressUpdate::phase(self.phase),
				ProgressUpdate::Message(format!(
					"{} files to be identified",
					self.metadata.total_found_orphans
				)),
			])
			.await;
			debug!(
				resuming_tasks_count = self.pending_tasks_on_resume.len(),
				"Resuming tasks for FileIdentifier job;",
			);
		}

		Ok(())
	}

	/// Process output of tasks, according to the downcasted output type
	///
	/// # Panics
	/// Will panic if another task type is added in the job, but this function wasn't updated to handle it
	///
	async fn process_task_output<OuterCtx: OuterContext>(
		&mut self,
		task_id: TaskId,
		any_task_output: Box<dyn AnyTaskOutput>,
		ctx: &impl JobContext<OuterCtx>,
		device_id: device::id::Type,
		dispatcher: &JobTaskDispatcher,
	) -> Result<Vec<TaskHandle<Error>>, DispatcherError> {
		if any_task_output.is::<identifier::Output>() {
			return self
				.process_identifier_output(
					task_id,
					*any_task_output
						.downcast::<identifier::Output>()
						.expect("just checked"),
					ctx,
					device_id,
					dispatcher,
				)
				.await;
		} else if any_task_output.is::<object_processor::Output>() {
			self.process_object_processor_output(
				task_id,
				*any_task_output
					.downcast::<object_processor::Output>()
					.expect("just checked"),
				ctx,
			)
			.await;
		} else {
			unreachable!("Unexpected task output type: <id='{task_id}'>");
		}

		Ok(vec![])
	}

	#[instrument(
		skip_all,
		fields(
			%task_id,
			?extract_metadata_time,
			?save_db_time,
			file_paths_to_object_processor_count = file_paths_by_cas_id.len(),
			created_objects_count,
			total_identified_files,
			errors_count = errors.len()
		)
	)]
	async fn process_identifier_output<OuterCtx: OuterContext>(
		&mut self,
		task_id: TaskId,
		identifier::Output {
			file_path_ids_with_new_object,
			file_paths_by_cas_id,
			extract_metadata_time,
			save_db_time,
			created_objects_count,
			total_identified_files,
			errors,
		}: identifier::Output,
		ctx: &impl JobContext<OuterCtx>,
		device_id: device::id::Type,
		dispatcher: &JobTaskDispatcher,
	) -> Result<Vec<TaskHandle<Error>>, DispatcherError> {
		self.metadata.mean_extract_metadata_time += extract_metadata_time;
		self.metadata.mean_save_db_time_on_identifier_tasks += save_db_time;
		self.metadata.total_identified_files += total_identified_files;
		self.metadata.created_objects_count += created_objects_count;

		let file_paths_with_new_object_to_report = file_path_ids_with_new_object
			.into_iter()
			.filter_map(|id| self.file_paths_ids_with_priority.take(&id))
			.collect::<Vec<_>>();

		if !file_paths_with_new_object_to_report.is_empty() {
			ctx.report_update(UpdateEvent::NewIdentifiedObjects {
				file_path_ids: file_paths_with_new_object_to_report,
			});
		}

		if !errors.is_empty() {
			warn!(?errors, "Non critical errors while extracting metadata;");
			self.errors.extend(errors);
		}

		accumulate_file_paths_by_cas_id(file_paths_by_cas_id, &mut self.file_paths_accumulator);

		self.metadata.completed_identifier_tasks += 1;

		ctx.progress(vec![
			ProgressUpdate::CompletedTaskCount(u64::from(self.metadata.completed_identifier_tasks)),
			ProgressUpdate::Message(format!(
				"Identified {} of {} files",
				self.metadata.total_identified_files, self.metadata.total_found_orphans
			)),
		])
		.await;

		debug!(
			"Processed ({}/{}) identifier tasks, took: {extract_metadata_time:?};",
			self.metadata.completed_identifier_tasks, self.metadata.total_identifier_tasks,
		);

		// If we completed all identifier tasks, then we dispatch the object processor tasks
		if self.metadata.completed_identifier_tasks == self.metadata.total_identifier_tasks {
			self.phase = Phase::ProcessingObjects;
			let (tasks_count, res) = match dispatch_object_processor_tasks(
				self.file_paths_accumulator.drain(),
				ctx,
				device_id,
				dispatcher,
				false,
			)
			.await
			{
				Ok(task_handles) => (task_handles.len(), Ok(task_handles)),
				Err(DispatcherError::Shutdown(tasks)) => {
					(tasks.len(), Err(DispatcherError::Shutdown(tasks)))
				}
				Err(e) => return Err(e),
			};

			#[allow(clippy::cast_possible_truncation)]
			{
				// SAFETY: we know that `tasks.len()` is a valid u32 as we wouldn't dispatch more than `u32::MAX` tasks
				self.metadata.total_object_processor_tasks = tasks_count as u32;
			}

			ctx.progress(vec![
				ProgressUpdate::TaskCount(u64::from(self.metadata.total_object_processor_tasks)),
				ProgressUpdate::CompletedTaskCount(0),
				ProgressUpdate::phase(self.phase),
			])
			.await;

			res
		} else {
			Ok(vec![])
		}
	}

	#[instrument(skip(self, file_path_ids_with_new_object, ctx))]
	async fn process_object_processor_output<OuterCtx: OuterContext>(
		&mut self,
		task_id: TaskId,
		object_processor::Output {
			file_path_ids_with_new_object,
			fetch_existing_objects_time,
			assign_to_existing_object_time,
			create_object_time,
			created_objects_count,
			linked_objects_count,
		}: object_processor::Output,
		ctx: &impl JobContext<OuterCtx>,
	) {
		self.metadata.mean_fetch_existing_objects_time += fetch_existing_objects_time;
		self.metadata.mean_assign_to_existing_object_time += assign_to_existing_object_time;
		self.metadata.mean_create_object_time += create_object_time;
		self.metadata.created_objects_count += created_objects_count;
		self.metadata.linked_objects_count += linked_objects_count;

		self.metadata.completed_object_processor_tasks += 1;

		ctx.progress(vec![
			ProgressUpdate::CompletedTaskCount(u64::from(
				self.metadata.completed_object_processor_tasks,
			)),
			ProgressUpdate::Message(format!(
				"Processed {} of {} objects",
				self.metadata.created_objects_count + self.metadata.linked_objects_count,
				self.metadata.total_found_orphans
			)),
		])
		.await;

		let file_paths_with_new_object_to_report = file_path_ids_with_new_object
			.into_iter()
			.filter_map(|id| self.file_paths_ids_with_priority.take(&id))
			.collect::<Vec<_>>();

		if !file_paths_with_new_object_to_report.is_empty() {
			ctx.report_update(UpdateEvent::NewIdentifiedObjects {
				file_path_ids: file_paths_with_new_object_to_report,
			});
		}

		debug!(
			"Processed ({}/{}) object processor tasks, took: {:?};",
			self.metadata.completed_object_processor_tasks,
			self.metadata.total_object_processor_tasks,
			fetch_existing_objects_time + assign_to_existing_object_time + create_object_time,
		);
	}

	async fn dispatch_priority_identifier_tasks<OuterCtx: OuterContext>(
		&mut self,
		sub_iso_file_path: &IsolatedFilePathData<'static>,
		ctx: &impl JobContext<OuterCtx>,
		device_id: device::id::Type,
		dispatcher: &JobTaskDispatcher,
		pending_running_tasks: &FuturesUnordered<TaskHandle<Error>>,
	) -> Result<(), JobErrorOrDispatcherError<file_identifier::Error>> {
		let db = ctx.db();

		loop {
			let start = Instant::now();

			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we know that CHUNK_SIZE is a valid i64
			let orphan_paths = db
				.file_path()
				.find_many(orphan_path_filters_shallow(
					self.location.id,
					self.last_orphan_file_path_id,
					sub_iso_file_path,
				))
				.order_by(file_path::id::order(SortOrder::Asc))
				.take(CHUNK_SIZE as i64)
				.select(file_path_for_file_identifier::select())
				.exec()
				.await
				.map_err(file_identifier::Error::from)?;

			trace!(orphans_count = orphan_paths.len(), "Found orphan paths;");

			if orphan_paths.is_empty() {
				break;
			}

			self.file_paths_ids_with_priority.extend(
				orphan_paths
					.iter()
					.map(|file_path_for_file_identifier::Data { id, .. }| *id),
			);

			self.metadata.total_found_orphans += orphan_paths.len() as u64;
			self.last_orphan_file_path_id =
				Some(orphan_paths.last().expect("orphan_paths is not empty").id);

			self.metadata.total_identifier_tasks += 1;

			ctx.progress(vec![
				ProgressUpdate::TaskCount(u64::from(self.metadata.total_identifier_tasks)),
				ProgressUpdate::Message(format!(
					"Found {} orphan paths",
					self.metadata.total_found_orphans
				)),
			])
			.await;

			debug!(
				"Dispatched ({}/{}) identifier tasks, took: {:?};",
				self.metadata.completed_identifier_tasks,
				self.metadata.total_identifier_tasks,
				start.elapsed(),
			);

			pending_running_tasks.push(
				dispatcher
					.dispatch(tasks::Identifier::new(
						Arc::clone(&self.location),
						Arc::clone(&self.location_path),
						orphan_paths,
						true,
						Arc::clone(ctx.db()),
						ctx.sync().clone(),
						device_id,
					))
					.await?,
			);
		}

		Ok(())
	}

	async fn dispatch_deep_identifier_tasks<OuterCtx: OuterContext>(
		&mut self,
		maybe_sub_iso_file_path: Option<&IsolatedFilePathData<'static>>,
		ctx: &impl JobContext<OuterCtx>,
		device_id: device::id::Type,
		dispatcher: &JobTaskDispatcher,
		pending_running_tasks: &FuturesUnordered<TaskHandle<Error>>,
	) -> Result<(), JobErrorOrDispatcherError<file_identifier::Error>> {
		let db = ctx.db();

		loop {
			let start = Instant::now();

			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we know that CHUNK_SIZE is a valid i64
			let mut orphan_paths = db
				.file_path()
				.find_many(orphan_path_filters_deep(
					self.location.id,
					self.last_orphan_file_path_id,
					maybe_sub_iso_file_path,
				))
				.order_by(file_path::id::order(SortOrder::Asc))
				.take(CHUNK_SIZE as i64)
				.select(file_path_for_file_identifier::select())
				.exec()
				.await
				.map_err(file_identifier::Error::from)?;

			// No other orphans to identify, we can break the loop
			if orphan_paths.is_empty() {
				break;
			}

			// We grab the last id to use as a starting point for the next iteration, in case we skip this one
			self.last_orphan_file_path_id =
				Some(orphan_paths.last().expect("orphan_paths is not empty").id);

			orphan_paths.retain(|file_path_for_file_identifier::Data { id, .. }| {
				!self.file_paths_ids_with_priority.contains(id)
			});

			// If we don't have any new orphan paths after filtering out, we can skip this iteration
			if orphan_paths.is_empty() {
				continue;
			}

			self.metadata.total_found_orphans += orphan_paths.len() as u64;

			self.metadata.total_identifier_tasks += 1;

			ctx.progress(vec![
				ProgressUpdate::TaskCount(u64::from(self.metadata.total_identifier_tasks)),
				ProgressUpdate::Message(format!(
					"Found {} orphan paths",
					self.metadata.total_found_orphans
				)),
			])
			.await;

			debug!(
				"Dispatched ({}/{}) identifier tasks, took: {:?};",
				self.metadata.completed_identifier_tasks,
				self.metadata.total_identifier_tasks,
				start.elapsed(),
			);

			pending_running_tasks.push(
				dispatcher
					.dispatch(tasks::Identifier::new(
						Arc::clone(&self.location),
						Arc::clone(&self.location_path),
						orphan_paths,
						false,
						Arc::clone(ctx.db()),
						ctx.sync().clone(),
						device_id,
					))
					.await?,
			);
		}

		Ok(())
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TaskKind {
	Identifier,
	ObjectProcessor,
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	sub_path: Option<PathBuf>,

	file_paths_accumulator: HashMap<CasId<'static>, Vec<FilePathToCreateOrLinkObject>>,
	file_paths_ids_with_priority: HashSet<file_path::id::Type>,
	last_orphan_file_path_id: Option<i32>,

	phase: Phase,
	metadata: Metadata,

	errors: Vec<NonCriticalError>,

	tasks_for_shutdown_bytes: Option<SerializedTasks>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
	mean_extract_metadata_time: Duration,
	mean_save_db_time_on_identifier_tasks: Duration,
	mean_fetch_existing_objects_time: Duration,
	mean_assign_to_existing_object_time: Duration,
	mean_create_object_time: Duration,
	seeking_orphans_time: Duration,
	total_found_orphans: u64,
	total_identified_files: u64,
	created_objects_count: u64,
	linked_objects_count: u64,
	total_identifier_tasks: u32,
	completed_identifier_tasks: u32,
	total_object_processor_tasks: u32,
	completed_object_processor_tasks: u32,
}

impl From<Metadata> for Vec<ReportOutputMetadata> {
	fn from(
		Metadata {
			mut mean_extract_metadata_time,
			mut mean_save_db_time_on_identifier_tasks,
			mut mean_fetch_existing_objects_time,
			mut mean_assign_to_existing_object_time,
			mut mean_create_object_time,
			seeking_orphans_time,
			total_found_orphans,
			total_identified_files,
			created_objects_count,
			linked_objects_count,
			total_identifier_tasks,
			completed_identifier_tasks,
			total_object_processor_tasks,
			completed_object_processor_tasks,
		}: Metadata,
	) -> Self {
		// To avoid division by zero
		mean_extract_metadata_time /= u32::max(total_identifier_tasks, 1);
		mean_save_db_time_on_identifier_tasks /= u32::max(total_identifier_tasks, 1);

		mean_fetch_existing_objects_time /= u32::max(total_object_processor_tasks, 1);
		mean_assign_to_existing_object_time /= u32::max(total_object_processor_tasks, 1);
		mean_create_object_time /= u32::max(total_object_processor_tasks, 1);

		vec![
			ReportOutputMetadata::FileIdentifier {
				total_orphan_paths: u64_to_frontend(total_found_orphans),
				total_objects_created: u64_to_frontend(created_objects_count),
				total_objects_linked: u64_to_frontend(linked_objects_count),
			},
			ReportOutputMetadata::Metrics(HashMap::from([
				(
					"mean_extract_metadata_time".into(),
					json!(mean_extract_metadata_time),
				),
				(
					"mean_save_db_time_on_identifier_tasks".into(),
					json!(mean_save_db_time_on_identifier_tasks),
				),
				(
					"mean_fetch_existing_objects_time".into(),
					json!(mean_fetch_existing_objects_time),
				),
				(
					"mean_assign_to_existing_object_time".into(),
					json!(mean_assign_to_existing_object_time),
				),
				(
					"mean_create_object_time".into(),
					json!(mean_create_object_time),
				),
				("seeking_orphans_time".into(), json!(seeking_orphans_time)),
				("total_found_orphans".into(), json!(total_found_orphans)),
				(
					"total_identified_files".into(),
					json!(total_identified_files),
				),
				("created_objects_count".into(), json!(created_objects_count)),
				("linked_objects_count".into(), json!(linked_objects_count)),
				(
					"total_identifier_tasks".into(),
					json!(total_identifier_tasks),
				),
				(
					"completed_identifier_tasks".into(),
					json!(completed_identifier_tasks),
				),
				(
					"total_object_processor_tasks".into(),
					json!(total_object_processor_tasks),
				),
				(
					"completed_object_processor_tasks".into(),
					json!(completed_object_processor_tasks),
				),
			])),
		]
	}
}

impl<OuterCtx: OuterContext> SerializableJob<OuterCtx> for FileIdentifier {
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let Self {
			location,
			location_path,
			sub_path,
			file_paths_accumulator,
			file_paths_ids_with_priority,
			last_orphan_file_path_id,
			phase,
			metadata,
			errors,
			tasks_for_shutdown,
			..
		} = self;

		let serialized_tasks = tasks_for_shutdown
			.into_iter()
			.map(|task| async move {
				if task.is::<tasks::Identifier>() {
					SerializableTask::serialize(
						*task.downcast::<tasks::Identifier>().expect("just checked"),
					)
					.await
					.map(|bytes| (TaskKind::Identifier, bytes))
				} else if task.is::<tasks::ObjectProcessor>() {
					task.downcast::<tasks::ObjectProcessor>()
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
			file_paths_accumulator,
			file_paths_ids_with_priority,
			last_orphan_file_path_id,
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
			file_paths_accumulator,
			file_paths_ids_with_priority,
			last_orphan_file_path_id,
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
				file_paths_accumulator,
				file_paths_ids_with_priority,
				last_orphan_file_path_id,
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
