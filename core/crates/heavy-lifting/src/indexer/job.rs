use crate::{
	indexer,
	job_system::{
		job::{
			Job, JobContext, JobName, JobReturn, JobTaskDispatcher, ProgressUpdate, ReturnStatus,
		},
		report::ReportOutputMetadata,
		utils::cancel_pending_tasks,
		SerializableJob, SerializedTasks,
	},
	utils::sub_path::get_full_path_from_sub_path,
	Error, LocationScanState, NonCriticalError, OuterContext,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_indexer_rules::{IndexerRule, IndexerRuler};
use sd_core_prisma_helpers::location_with_indexer_rules;

use sd_prisma::prisma::location;
use sd_task_system::{
	AnyTaskOutput, IntoTask, SerializableTask, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskOutput, TaskStatus,
};
use sd_utils::{db::maybe_missing, u64_to_frontend};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	hash::{Hash, Hasher},
	mem,
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::Instant;
use tracing::{debug, error, trace, warn};

use super::{
	remove_non_existing_file_paths, reverse_update_directories_sizes,
	tasks::{
		saver::{SaveTask, SaveTaskOutput},
		updater::{UpdateTask, UpdateTaskOutput},
		walker::{WalkDirTask, WalkTaskOutput, WalkedEntry},
	},
	update_directory_sizes, update_location_size, IsoFilePathFactory, WalkerDBProxy, BATCH_SIZE,
};

#[derive(Debug)]
pub struct Indexer {
	location: location_with_indexer_rules::Data,
	sub_path: Option<PathBuf>,
	metadata: Metadata,

	iso_file_path_factory: IsoFilePathFactory,
	indexer_ruler: IndexerRuler,
	walker_root_path: Option<Arc<PathBuf>>,
	ancestors_needing_indexing: HashSet<WalkedEntry>,
	ancestors_already_indexed: HashSet<IsolatedFilePathData<'static>>,
	iso_paths_and_sizes: HashMap<IsolatedFilePathData<'static>, u64>,

	processing_first_directory: bool,

	to_create_buffer: VecDeque<WalkedEntry>,
	to_update_buffer: VecDeque<WalkedEntry>,

	errors: Vec<NonCriticalError>,

	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Box<dyn Task<Error>>>,
}

impl Job for Indexer {
	const NAME: JobName = JobName::Indexer;

	async fn resume_tasks<OuterCtx: OuterContext>(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext<OuterCtx>,
		SerializedTasks(serialized_tasks): SerializedTasks,
	) -> Result<(), Error> {
		let location_id = self.location.id;

		self.pending_tasks_on_resume = dispatcher
			.dispatch_many_boxed(
				rmp_serde::from_slice::<Vec<(TaskKind, Vec<u8>)>>(&serialized_tasks)
					.map_err(indexer::Error::from)?
					.into_iter()
					.map(|(task_kind, task_bytes)| {
						let indexer_ruler = self.indexer_ruler.clone();
						let iso_file_path_factory = self.iso_file_path_factory.clone();
						async move {
							match task_kind {
								TaskKind::Walk => WalkDirTask::deserialize(
									&task_bytes,
									(
										indexer_ruler.clone(),
										WalkerDBProxy {
											location_id,
											db: Arc::clone(ctx.db()),
										},
										iso_file_path_factory.clone(),
										dispatcher.clone(),
									),
								)
								.await
								.map(IntoTask::into_task),

								TaskKind::Save => SaveTask::deserialize(
									&task_bytes,
									(Arc::clone(ctx.db()), Arc::clone(ctx.sync())),
								)
								.await
								.map(IntoTask::into_task),
								TaskKind::Update => UpdateTask::deserialize(
									&task_bytes,
									(Arc::clone(ctx.db()), Arc::clone(ctx.sync())),
								)
								.await
								.map(IntoTask::into_task),
							}
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await
					.map_err(indexer::Error::from)?,
			)
			.await;

		Ok(())
	}

	async fn run<OuterCtx: OuterContext>(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: impl JobContext<OuterCtx>,
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

		if let Some(res) = self
			.dispatch_last_save_and_update_tasks(&mut pending_running_tasks, &ctx, &dispatcher)
			.await
		{
			return res;
		}

		if let Some(res) = self
			.index_pending_ancestors(&mut pending_running_tasks, &ctx, &dispatcher)
			.await
		{
			return res;
		}

		if !self.tasks_for_shutdown.is_empty() {
			return Ok(ReturnStatus::Shutdown(
				SerializableJob::<OuterCtx>::serialize(self).await,
			));
		}

		// From here onward, job will not be interrupted anymore

		let Self {
			location,
			mut metadata,
			iso_file_path_factory,
			walker_root_path,
			iso_paths_and_sizes,
			mut errors,
			tasks_for_shutdown,
			..
		} = self;

		if metadata.indexed_count > 0 || metadata.removed_count > 0 || metadata.updated_count > 0 {
			let start_size_update_time = Instant::now();

			update_directory_sizes(iso_paths_and_sizes, ctx.db(), ctx.sync()).await?;

			let root_path = walker_root_path.expect("must be set");
			if root_path != iso_file_path_factory.location_path {
				reverse_update_directories_sizes(
					&*root_path,
					location.id,
					&*iso_file_path_factory.location_path,
					ctx.db(),
					ctx.sync(),
					&mut errors,
				)
				.await?;
			}

			update_location_size(location.id, ctx.db(), &ctx).await?;

			metadata.db_write_time += start_size_update_time.elapsed();
		}

		if metadata.removed_count > 0 {
			// TODO: Dispatch a task to remove orphan objects
		}

		if metadata.indexed_count > 0 || metadata.removed_count > 0 {
			ctx.invalidate_query("search.paths");
		}

		assert!(
			tasks_for_shutdown.is_empty(),
			"all tasks must be completed here"
		);

		ctx.db()
			.location()
			.update(
				location::id::equals(location.id),
				vec![location::scan_state::set(LocationScanState::Indexed as i32)],
			)
			.exec()
			.await
			.map_err(indexer::Error::from)?;

		Ok(ReturnStatus::Completed(
			JobReturn::builder()
				.with_metadata(metadata)
				.with_non_critical_errors(errors)
				.build(),
		))
	}
}

impl Indexer {
	pub fn new(
		location: location_with_indexer_rules::Data,
		sub_path: Option<PathBuf>,
	) -> Result<Self, indexer::Error> {
		Ok(Self {
			indexer_ruler: location
				.indexer_rules
				.iter()
				.map(|rule| IndexerRule::try_from(&rule.indexer_rule))
				.collect::<Result<Vec<_>, _>>()
				.map(IndexerRuler::new)?,
			iso_file_path_factory: IsoFilePathFactory {
				location_id: location.id,
				location_path: maybe_missing(&location.path, "location.path")
					.map(PathBuf::from)
					.map(Arc::new)?,
			},
			walker_root_path: None,
			ancestors_needing_indexing: HashSet::new(),
			ancestors_already_indexed: HashSet::new(),
			iso_paths_and_sizes: HashMap::new(),
			location,
			sub_path,
			metadata: Metadata::default(),

			processing_first_directory: true,

			to_create_buffer: VecDeque::new(),
			to_update_buffer: VecDeque::new(),

			errors: Vec::new(),

			pending_tasks_on_resume: Vec::new(),
			tasks_for_shutdown: Vec::new(),
		})
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
		dispatcher: &JobTaskDispatcher,
	) -> Result<Vec<TaskHandle<Error>>, indexer::Error> {
		self.metadata.completed_tasks += 1;

		ctx.progress(vec![ProgressUpdate::CompletedTaskCount(
			self.metadata.completed_tasks,
		)])
		.await;

		if any_task_output.is::<WalkTaskOutput>() {
			return self
				.process_walk_output(
					*any_task_output
						.downcast::<WalkTaskOutput>()
						.expect("just checked"),
					ctx,
					dispatcher,
				)
				.await;
		} else if any_task_output.is::<SaveTaskOutput>() {
			self.process_save_output(
				*any_task_output
					.downcast::<SaveTaskOutput>()
					.expect("just checked"),
				ctx,
			)
			.await;
		} else if any_task_output.is::<UpdateTaskOutput>() {
			self.process_update_output(
				*any_task_output
					.downcast::<UpdateTaskOutput>()
					.expect("just checked"),
				ctx,
			)
			.await;
		} else {
			unreachable!("Unexpected task output type: <id='{task_id}'>");
		}

		Ok(Vec::new())
	}

	async fn process_walk_output<OuterCtx: OuterContext>(
		&mut self,
		WalkTaskOutput {
			to_create,
			to_update,
			to_remove,
			accepted_ancestors,
			errors,
			directory_iso_file_path,
			total_size,
			mut handles,
			scan_time,
			..
		}: WalkTaskOutput,
		ctx: &impl JobContext<OuterCtx>,
		dispatcher: &JobTaskDispatcher,
	) -> Result<Vec<TaskHandle<Error>>, indexer::Error> {
		self.metadata.scan_read_time += scan_time;

		let (to_create_count, to_update_count) = (to_create.len(), to_update.len());

		debug!("Scanned {directory_iso_file_path} in {scan_time:?}");

		*self
			.iso_paths_and_sizes
			.entry(directory_iso_file_path)
			.or_default() += total_size;

		for ancestor_iso_file_path in accepted_ancestors
			.iter()
			.map(|ancestor_entry| &ancestor_entry.iso_file_path)
		{
			if self
				.iso_paths_and_sizes
				.contains_key(ancestor_iso_file_path)
			{
				*self
					.iso_paths_and_sizes
					.get_mut(ancestor_iso_file_path)
					.expect("we just checked") += total_size;
			} else {
				self.iso_paths_and_sizes
					.insert(ancestor_iso_file_path.clone(), total_size);
			}
		}

		// First we add ancestors, filtering out ancestors already indexed in previous iterations
		self.ancestors_needing_indexing
			.extend(accepted_ancestors.into_iter().filter(|ancestor_entry| {
				!self
					.ancestors_already_indexed
					.contains(&ancestor_entry.iso_file_path)
			}));

		// Then we add new directories to be indexed as they can be received as ancestors in coming iterations
		self.ancestors_already_indexed.extend(
			to_create
				.iter()
				.filter(|&WalkedEntry { iso_file_path, .. }| iso_file_path.is_dir())
				.map(|WalkedEntry { iso_file_path, .. }| iso_file_path.clone()),
		);

		if !errors.is_empty() {
			error!("Non critical errors while indexing: {errors:#?}");
			self.errors.extend(errors);
		}

		let db_delete_time = Instant::now();
		self.metadata.removed_count +=
			remove_non_existing_file_paths(to_remove, ctx.db(), ctx.sync()).await?;
		self.metadata.db_write_time += db_delete_time.elapsed();

		let (save_tasks, update_tasks) =
			self.prepare_save_and_update_tasks(to_create, to_update, ctx);

		debug!(
			"Dispatching more ({}W/{}S/{}U) tasks, completed ({}/{})",
			handles.len(),
			save_tasks.len(),
			update_tasks.len(),
			self.metadata.completed_tasks,
			self.metadata.total_tasks
		);

		handles.extend(dispatcher.dispatch_many(save_tasks).await);
		handles.extend(dispatcher.dispatch_many(update_tasks).await);

		self.metadata.total_tasks += handles.len() as u64;

		ctx.progress(vec![
			ProgressUpdate::TaskCount(self.metadata.total_tasks),
			ProgressUpdate::message(format!(
				"Found {to_create_count} new files and {to_update_count} to update"
			)),
		])
		.await;

		Ok(handles)
	}

	async fn process_save_output<OuterCtx: OuterContext>(
		&mut self,
		SaveTaskOutput {
			saved_count,
			save_duration,
		}: SaveTaskOutput,
		ctx: &impl JobContext<OuterCtx>,
	) {
		self.metadata.indexed_count += saved_count;
		self.metadata.db_write_time += save_duration;

		ctx.progress_msg(format!("Saved {saved_count} files")).await;

		debug!(
			"Processed save task in the indexer ({}/{}), took: {save_duration:?}",
			self.metadata.completed_tasks, self.metadata.total_tasks
		);
	}

	async fn process_update_output<OuterCtx: OuterContext>(
		&mut self,
		UpdateTaskOutput {
			updated_count,
			update_duration,
		}: UpdateTaskOutput,
		ctx: &impl JobContext<OuterCtx>,
	) {
		self.metadata.updated_count += updated_count;
		self.metadata.db_write_time += update_duration;

		ctx.progress_msg(format!("Updated {updated_count} files"))
			.await;

		debug!(
			"Processed update task in the indexer ({}/{}), took: {update_duration:?}",
			self.metadata.completed_tasks, self.metadata.total_tasks
		);
	}

	async fn process_handles<OuterCtx: OuterContext>(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		ctx: &impl JobContext<OuterCtx>,
		dispatcher: &JobTaskDispatcher,
	) -> Option<Result<ReturnStatus, Error>> {
		while let Some(task) = pending_running_tasks.next().await {
			match task {
				Ok(TaskStatus::Done((task_id, TaskOutput::Out(out)))) => {
					let more_handles = match self
						.process_task_output(task_id, out, ctx, dispatcher)
						.await
					{
						Ok(more_handles) => more_handles,
						Err(e) => {
							cancel_pending_tasks(&*pending_running_tasks).await;

							return Some(Err(e.into()));
						}
					};

					pending_running_tasks.extend(more_handles);
				}

				Ok(TaskStatus::Done((task_id, TaskOutput::Empty))) => {
					warn!("Task <id='{task_id}'> returned an empty output");
				}

				Ok(TaskStatus::Shutdown(task)) => {
					self.tasks_for_shutdown.push(task);
				}

				Ok(TaskStatus::Error(e)) => {
					cancel_pending_tasks(&*pending_running_tasks).await;

					return Some(Err(e));
				}

				Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
					cancel_pending_tasks(&*pending_running_tasks).await;

					return Some(Ok(ReturnStatus::Canceled));
				}

				Err(e) => {
					cancel_pending_tasks(&*pending_running_tasks).await;

					return Some(Err(e.into()));
				}
			}
		}

		None
	}

	async fn init_or_resume<OuterCtx: OuterContext>(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		ctx: &impl JobContext<OuterCtx>,
		dispatcher: &JobTaskDispatcher,
	) -> Result<(), indexer::Error> {
		// if we don't have any pending task, then this is a fresh job
		let updates = if self.pending_tasks_on_resume.is_empty() {
			let walker_root_path = Arc::new(
				get_full_path_from_sub_path(
					self.location.id,
					&self.sub_path,
					&*self.iso_file_path_factory.location_path,
					ctx.db(),
				)
				.await?,
			);

			pending_running_tasks.push(
				dispatcher
					.dispatch(WalkDirTask::new_deep(
						walker_root_path.as_ref(),
						Arc::clone(&walker_root_path),
						self.indexer_ruler.clone(),
						self.iso_file_path_factory.clone(),
						WalkerDBProxy {
							location_id: self.location.id,
							db: Arc::clone(ctx.db()),
						},
						dispatcher.clone(),
					)?)
					.await,
			);

			self.metadata.total_tasks = 1;

			let updates = vec![
				ProgressUpdate::TaskCount(self.metadata.total_tasks),
				ProgressUpdate::Message(format!("Indexing {}", walker_root_path.display())),
			];

			self.walker_root_path = Some(walker_root_path);

			updates
		} else {
			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));

			vec![
				ProgressUpdate::TaskCount(self.metadata.total_tasks),
				ProgressUpdate::Message("Resuming tasks".to_string()),
			]
		};

		ctx.progress(updates).await;

		Ok(())
	}

	async fn dispatch_last_save_and_update_tasks<OuterCtx: OuterContext>(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		ctx: &impl JobContext<OuterCtx>,
		dispatcher: &JobTaskDispatcher,
	) -> Option<Result<ReturnStatus, Error>> {
		if !self.to_create_buffer.is_empty() || !self.to_update_buffer.is_empty() {
			if !self.to_create_buffer.is_empty() {
				assert!(
					self.to_create_buffer.len() <= BATCH_SIZE,
					"last save task must be less than BATCH_SIZE paths"
				);

				self.metadata.total_paths += self.to_create_buffer.len() as u64;
				self.metadata.total_save_steps += 1;

				pending_running_tasks.push(
					dispatcher
						.dispatch(SaveTask::new_deep(
							self.location.id,
							self.location.pub_id.clone(),
							self.to_create_buffer.drain(..).collect(),
							Arc::clone(ctx.db()),
							Arc::clone(ctx.sync()),
						))
						.await,
				);
			}

			if !self.to_update_buffer.is_empty() {
				assert!(
					self.to_update_buffer.len() <= BATCH_SIZE,
					"last update task must be less than BATCH_SIZE paths"
				);

				self.metadata.total_updated_paths += self.to_update_buffer.len() as u64;
				self.metadata.total_update_steps += 1;

				pending_running_tasks.push(
					dispatcher
						.dispatch(UpdateTask::new_deep(
							self.to_update_buffer.drain(..).collect(),
							Arc::clone(ctx.db()),
							Arc::clone(ctx.sync()),
						))
						.await,
				);
			}

			self.process_handles(pending_running_tasks, ctx, dispatcher)
				.await
		} else {
			None
		}
	}

	async fn index_pending_ancestors<OuterCtx: OuterContext>(
		&mut self,
		pending_running_tasks: &mut FuturesUnordered<TaskHandle<Error>>,
		ctx: &impl JobContext<OuterCtx>,
		dispatcher: &JobTaskDispatcher,
	) -> Option<Result<ReturnStatus, Error>> {
		if self.ancestors_needing_indexing.is_empty() {
			return None;
		}

		let save_tasks = self
			.ancestors_needing_indexing
			.drain()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| {
				let chunked_saves = chunk.collect::<Vec<_>>();
				self.metadata.total_paths += chunked_saves.len() as u64;
				self.metadata.total_save_steps += 1;

				SaveTask::new_deep(
					self.location.id,
					self.location.pub_id.clone(),
					chunked_saves,
					Arc::clone(ctx.db()),
					Arc::clone(ctx.sync()),
				)
			})
			.collect::<Vec<_>>();

		pending_running_tasks.extend(dispatcher.dispatch_many(save_tasks).await);

		self.process_handles(pending_running_tasks, ctx, dispatcher)
			.await
	}

	fn prepare_save_and_update_tasks<OuterCtx: OuterContext>(
		&mut self,
		to_create: Vec<WalkedEntry>,
		to_update: Vec<WalkedEntry>,
		ctx: &impl JobContext<OuterCtx>,
	) -> (Vec<SaveTask>, Vec<UpdateTask>) {
		if self.processing_first_directory {
			// If we are processing the first directory, we dispatch shallow tasks with higher priority
			// this way we provide a faster feedback loop to the user
			self.processing_first_directory = false;

			let save_tasks = to_create
				.into_iter()
				.chunks(BATCH_SIZE)
				.into_iter()
				.map(|chunk| {
					let chunked_saves = chunk.collect::<Vec<_>>();
					self.metadata.total_paths += chunked_saves.len() as u64;
					self.metadata.total_save_steps += 1;

					SaveTask::new_shallow(
						self.location.id,
						self.location.pub_id.clone(),
						chunked_saves,
						Arc::clone(ctx.db()),
						Arc::clone(ctx.sync()),
					)
				})
				.collect::<Vec<_>>();

			let update_tasks = to_update
				.into_iter()
				.chunks(BATCH_SIZE)
				.into_iter()
				.map(|chunk| {
					let chunked_updates = chunk.collect::<Vec<_>>();
					self.metadata.total_updated_paths += chunked_updates.len() as u64;
					self.metadata.total_update_steps += 1;

					UpdateTask::new_shallow(
						chunked_updates,
						Arc::clone(ctx.db()),
						Arc::clone(ctx.sync()),
					)
				})
				.collect::<Vec<_>>();

			(save_tasks, update_tasks)
		} else {
			self.to_create_buffer.extend(to_create);

			let save_tasks = if self.to_create_buffer.len() > BATCH_SIZE {
				let chunks_count = self.to_create_buffer.len() / BATCH_SIZE;
				let mut save_tasks = Vec::with_capacity(chunks_count);

				for _ in 0..chunks_count {
					let chunked_saves = self
						.to_create_buffer
						.drain(..BATCH_SIZE)
						.collect::<Vec<_>>();

					self.metadata.total_paths += chunked_saves.len() as u64;
					self.metadata.total_save_steps += 1;

					save_tasks.push(SaveTask::new_deep(
						self.location.id,
						self.location.pub_id.clone(),
						chunked_saves,
						Arc::clone(ctx.db()),
						Arc::clone(ctx.sync()),
					));
				}
				save_tasks
			} else {
				trace!("Not enough entries to dispatch a new saver task");
				vec![]
			};

			self.to_update_buffer.extend(to_update);

			let update_tasks = if self.to_update_buffer.len() > BATCH_SIZE {
				let chunks_count = self.to_update_buffer.len() / BATCH_SIZE;
				let mut update_tasks = Vec::with_capacity(chunks_count);

				for _ in 0..chunks_count {
					let chunked_updates = self
						.to_update_buffer
						.drain(..BATCH_SIZE)
						.collect::<Vec<_>>();

					self.metadata.total_updated_paths += chunked_updates.len() as u64;
					self.metadata.total_update_steps += 1;

					update_tasks.push(UpdateTask::new_deep(
						chunked_updates,
						Arc::clone(ctx.db()),
						Arc::clone(ctx.sync()),
					));
				}
				update_tasks
			} else {
				trace!("Not enough entries to dispatch a new updater task");
				vec![]
			};

			(save_tasks, update_tasks)
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
	db_write_time: Duration,
	scan_read_time: Duration,
	total_tasks: u64,
	completed_tasks: u64,
	total_paths: u64,
	total_updated_paths: u64,
	total_save_steps: u64,
	total_update_steps: u64,
	indexed_count: u64,
	updated_count: u64,
	removed_count: u64,
}

impl From<Metadata> for Vec<ReportOutputMetadata> {
	fn from(
		Metadata {
			db_write_time,
			scan_read_time,
			total_tasks,
			completed_tasks,
			total_paths,
			total_updated_paths,
			total_save_steps,
			total_update_steps,
			indexed_count,
			updated_count,
			removed_count,
		}: Metadata,
	) -> Self {
		vec![
			ReportOutputMetadata::Indexer {
				total_paths: u64_to_frontend(total_paths),
			},
			ReportOutputMetadata::Metrics(HashMap::from([
				("db_write_time".into(), json!(db_write_time)),
				("scan_read_time".into(), json!(scan_read_time)),
				("total_tasks".into(), json!(total_tasks)),
				("completed_tasks".into(), json!(completed_tasks)),
				("total_paths".into(), json!(total_paths)),
				("total_updated_paths".into(), json!(total_updated_paths)),
				("total_save_steps".into(), json!(total_save_steps)),
				("total_update_steps".into(), json!(total_update_steps)),
				("indexed_count".into(), json!(indexed_count)),
				("updated_count".into(), json!(updated_count)),
				("removed_count".into(), json!(removed_count)),
			])),
		]
	}
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TaskKind {
	Walk,
	Save,
	Update,
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	location: location_with_indexer_rules::Data,
	sub_path: Option<PathBuf>,
	metadata: Metadata,

	iso_file_path_factory: IsoFilePathFactory,
	indexer_ruler: IndexerRuler,
	walker_root_path: Option<Arc<PathBuf>>,
	ancestors_needing_indexing: HashSet<WalkedEntry>,
	ancestors_already_indexed: HashSet<IsolatedFilePathData<'static>>,
	iso_paths_and_sizes: HashMap<IsolatedFilePathData<'static>, u64>,

	processing_first_directory: bool,

	to_create_buffer: VecDeque<WalkedEntry>,
	to_update_buffer: VecDeque<WalkedEntry>,

	errors: Vec<NonCriticalError>,

	tasks_for_shutdown_bytes: Option<SerializedTasks>,
}

impl<OuterCtx: OuterContext> SerializableJob<OuterCtx> for Indexer {
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let Self {
			location,
			sub_path,
			metadata,
			iso_file_path_factory,
			indexer_ruler,
			walker_root_path,
			ancestors_needing_indexing,
			ancestors_already_indexed,
			iso_paths_and_sizes,
			processing_first_directory,
			to_create_buffer,
			to_update_buffer,
			errors,
			tasks_for_shutdown,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			location,
			sub_path,
			metadata,
			iso_file_path_factory,
			indexer_ruler,
			walker_root_path,
			ancestors_needing_indexing,
			ancestors_already_indexed,
			iso_paths_and_sizes,
			processing_first_directory,
			to_create_buffer,
			to_update_buffer,
			errors,
			tasks_for_shutdown_bytes: Some(SerializedTasks(rmp_serde::to_vec_named(
				&tasks_for_shutdown
					.into_iter()
					.map(|task| async move {
						if task
							.is::<WalkDirTask<WalkerDBProxy, IsoFilePathFactory, JobTaskDispatcher>>(
							) {
							task
							.downcast::<WalkDirTask<WalkerDBProxy, IsoFilePathFactory, JobTaskDispatcher>>(
							)
							.expect("just checked")
							.serialize()
							.await
							.map(|bytes| (TaskKind::Walk, bytes))
						} else if task.is::<SaveTask>() {
							task.downcast::<SaveTask>()
								.expect("just checked")
								.serialize()
								.await
								.map(|bytes| (TaskKind::Save, bytes))
						} else if task.is::<UpdateTask>() {
							task.downcast::<UpdateTask>()
								.expect("just checked")
								.serialize()
								.await
								.map(|bytes| (TaskKind::Update, bytes))
						} else {
							unreachable!("Unexpected task type")
						}
					})
					.collect::<Vec<_>>()
					.try_join()
					.await?,
			)?)),
		})
		.map(Some)
	}

	async fn deserialize(
		serialized_job: &[u8],
		_: &OuterCtx,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let SaveState {
			location,
			sub_path,
			metadata,
			iso_file_path_factory,
			indexer_ruler,
			walker_root_path,
			ancestors_needing_indexing,
			ancestors_already_indexed,
			iso_paths_and_sizes,
			processing_first_directory,
			to_create_buffer,
			to_update_buffer,
			errors,
			tasks_for_shutdown_bytes,
		} = rmp_serde::from_slice::<SaveState>(serialized_job)?;

		Ok(Some((
			Self {
				location,
				sub_path,
				metadata,
				iso_file_path_factory,
				indexer_ruler,
				walker_root_path,
				ancestors_needing_indexing,
				ancestors_already_indexed,
				iso_paths_and_sizes,
				processing_first_directory,
				to_create_buffer,
				to_update_buffer,
				errors,
				pending_tasks_on_resume: Vec::new(),
				tasks_for_shutdown: Vec::new(),
			},
			tasks_for_shutdown_bytes,
		)))
	}
}

impl Hash for Indexer {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}
