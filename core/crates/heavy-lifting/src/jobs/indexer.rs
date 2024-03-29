use crate::{
	tasks::indexer::{
		saver::{SaveTask, SaveTaskOutput},
		updater::{UpdateTask, UpdateTaskOutput},
		walker::{self, WalkDirTask, WalkTaskOutput},
		IndexerError, NonCriticalIndexerError,
	},
	Error, NonCriticalJobError,
};

use sd_core_file_path_helper::{
	ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	FilePathError, IsolatedFilePathData,
};
use sd_core_indexer_rules::{IndexerRule, IndexerRuler};
use sd_core_prisma_helpers::{
	file_path_pub_and_cas_ids, file_path_walker, location_with_indexer_rules,
};

use sd_prisma::{
	prisma::{file_path, location, PrismaClient, SortOrder},
	prisma_sync,
};
use sd_task_system::{
	AnyTaskOutput, IntoTask, SerializableTask, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskOutput, TaskStatus,
};
use sd_utils::db::maybe_missing;

use std::{
	collections::{HashMap, HashSet},
	hash::{Hash, Hasher},
	mem,
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use prisma_client_rust::operator::or;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::warn;

use super::{
	cancel_pending_tasks,
	job_system::{
		job::{Job, JobContext, JobName, JobReturn, JobTaskDispatcher, ReturnStatus},
		SerializableJob, SerializedTasks,
	},
};

/// `BATCH_SIZE` is the number of files to index at each task, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
	db_write_time: Duration,
	scan_read_time: Duration,
	total_paths: u64,
	total_updated_paths: u64,
	total_save_steps: u64,
	total_update_steps: u64,
	indexed_count: u64,
	updated_count: u64,
	removed_count: u64,
	paths_and_sizes: HashMap<PathBuf, u64>,
}

#[derive(Debug)]
pub struct IndexerJob {
	location: Arc<location_with_indexer_rules::Data>,
	sub_path: Option<PathBuf>,
	metadata: Metadata,

	iso_file_path_factory: IsoFilePathFactory,
	indexer_ruler: IndexerRuler,
	walker_root_path: Option<Arc<PathBuf>>,

	errors: Vec<NonCriticalJobError>,

	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Box<dyn Task<Error>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TaskKind {
	Walk,
	Save,
	Update,
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	location: Arc<location_with_indexer_rules::Data>,
	sub_path: Option<PathBuf>,
	metadata: Metadata,

	iso_file_path_factory: IsoFilePathFactory,
	indexer_ruler_bytes: Vec<u8>,
	walker_root_path: Option<Arc<PathBuf>>,

	errors: Vec<NonCriticalJobError>,

	tasks_for_shutdown_bytes: Option<SerializedTasks>,
}

impl SerializableJob for IndexerJob {
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		rmp_serde::to_vec_named(&SaveState {
			location: self.location,
			sub_path: self.sub_path,
			metadata: self.metadata,
			iso_file_path_factory: self.iso_file_path_factory,
			indexer_ruler_bytes: self.indexer_ruler.serialize().await?,
			walker_root_path: self.walker_root_path,
			tasks_for_shutdown_bytes: Some(SerializedTasks(rmp_serde::to_vec_named(
				&self
					.tasks_for_shutdown
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
			errors: self.errors,
		})
		.map(Some)
	}

	async fn deserialize(
		serialized_job: &[u8],
		_: &impl JobContext,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let SaveState {
			location,
			sub_path,
			metadata,
			iso_file_path_factory,
			indexer_ruler_bytes,
			walker_root_path,
			errors,
			tasks_for_shutdown_bytes,
		} = rmp_serde::from_slice::<SaveState>(serialized_job)?;

		let indexer_ruler = IndexerRuler::deserialize(&indexer_ruler_bytes)?;

		Ok(Some((
			Self {
				location,
				sub_path,
				metadata,
				iso_file_path_factory,
				indexer_ruler,
				walker_root_path,
				errors,
				pending_tasks_on_resume: Vec::new(),
				tasks_for_shutdown: Vec::new(),
			},
			tasks_for_shutdown_bytes,
		)))
	}
}

impl Hash for IndexerJob {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

impl IndexerJob {
	pub fn new(
		location: location_with_indexer_rules::Data,
		sub_path: Option<PathBuf>,
	) -> Result<Self, IndexerError> {
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
			location: Arc::new(location),
			sub_path,
			metadata: Metadata::default(),
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
	async fn process_task_output(
		&mut self,
		task_id: TaskId,
		any_task_output: Box<dyn AnyTaskOutput>,
		job_ctx: &impl JobContext,
		dispatcher: &JobTaskDispatcher,
	) -> Result<Vec<TaskHandle<Error>>, IndexerError> {
		if any_task_output.is::<WalkTaskOutput>() {
			return self
				.process_walk_output(
					*any_task_output
						.downcast::<WalkTaskOutput>()
						.expect("just checked"),
					job_ctx,
					dispatcher,
				)
				.await;
		} else if any_task_output.is::<SaveTaskOutput>() {
			self.process_save_output(
				*any_task_output
					.downcast::<SaveTaskOutput>()
					.expect("just checked"),
			);
		} else if any_task_output.is::<UpdateTaskOutput>() {
			self.process_update_output(
				*any_task_output
					.downcast::<UpdateTaskOutput>()
					.expect("just checked"),
			);
		} else {
			unreachable!("Unexpected task output type: <id='{task_id}'>");
		}

		Ok(Vec::new())
	}

	async fn process_walk_output(
		&mut self,
		WalkTaskOutput {
			to_create,
			to_update,
			to_remove,
			accepted_ancestors,
			errors,
			directory,
			total_size,
			maybe_parent,
			mut handles,
			scan_time,
		}: WalkTaskOutput,
		job_ctx: &impl JobContext,
		dispatcher: &JobTaskDispatcher,
	) -> Result<Vec<TaskHandle<Error>>, IndexerError> {
		self.metadata.scan_read_time += scan_time;

		*self.metadata.paths_and_sizes.entry(directory).or_default() += total_size;
		if let Some(parent) = maybe_parent {
			*self.metadata.paths_and_sizes.entry(parent).or_default() += total_size;
		}

		self.errors.extend(errors);

		// TODO: Figure out how to handle the accepted ancestors

		let db_delete_time = Instant::now();
		self.metadata.removed_count +=
			remove_non_existing_file_paths(to_remove, job_ctx.db(), job_ctx.sync()).await?;
		self.metadata.db_write_time += db_delete_time.elapsed();

		let save_tasks = to_create
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| {
				let chunked_saves = chunk.collect::<Vec<_>>();
				self.metadata.total_paths += chunked_saves.len() as u64;
				self.metadata.total_save_steps += 1;

				SaveTask::new(
					Arc::clone(&self.location),
					chunked_saves,
					Arc::clone(job_ctx.db()),
					Arc::clone(job_ctx.sync()),
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

				UpdateTask::new(
					chunked_updates,
					Arc::clone(job_ctx.db()),
					Arc::clone(job_ctx.sync()),
				)
			})
			.collect::<Vec<_>>();

		handles.extend(dispatcher.dispatch_many(save_tasks).await);
		handles.extend(dispatcher.dispatch_many(update_tasks).await);

		// TODO: Report progress

		Ok(handles)
	}

	fn process_save_output(
		&mut self,
		SaveTaskOutput {
			saved_count,
			save_duration,
		}: SaveTaskOutput,
	) {
		self.metadata.indexed_count += saved_count;
		self.metadata.db_write_time += save_duration;

		// TODO: Report progress
	}

	fn process_update_output(
		&mut self,
		UpdateTaskOutput {
			updated_count,
			update_duration,
		}: UpdateTaskOutput,
	) {
		self.metadata.updated_count += updated_count;
		self.metadata.db_write_time += update_duration;

		// TODO: Report progress
	}
}

impl Job for IndexerJob {
	const NAME: JobName = JobName::Indexer;

	async fn run(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: impl JobContext,
	) -> Result<ReturnStatus, Error> {
		let mut pending_running_tasks = FuturesUnordered::new();

		// if we don't have any pending task, then this is a fresh job
		if self.pending_tasks_on_resume.is_empty() {
			let walker_root_path = Arc::new(
				determine_initial_walk_path(
					self.location.id,
					&self.sub_path,
					&self.iso_file_path_factory.location_path,
					ctx.db(),
				)
				.await?,
			);

			pending_running_tasks.push(
				dispatcher
					.dispatch(WalkDirTask::new(
						walker_root_path.as_ref(),
						Arc::clone(&walker_root_path),
						self.indexer_ruler.clone(),
						self.iso_file_path_factory.clone(),
						WalkerDBProxy {
							location_id: self.location.id,
							db: Arc::clone(ctx.db()),
						},
						Some(dispatcher.clone()),
					)?)
					.await,
			);

			self.walker_root_path = Some(walker_root_path);
		} else {
			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));
		}

		while let Some(task) = pending_running_tasks.next().await {
			match task {
				Ok(TaskStatus::Done((task_id, TaskOutput::Out(out)))) => {
					let more_handles = match self
						.process_task_output(task_id, out, &ctx, &dispatcher)
						.await
					{
						Ok(more_handles) => more_handles,
						Err(e) => {
							cancel_pending_tasks(&pending_running_tasks).await;

							return Err(e.into());
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

		if self.tasks_for_shutdown.is_empty() {
			Ok(ReturnStatus::Completed(JobReturn::default()))
		} else {
			Ok(ReturnStatus::Shutdown(self.serialize().await))
		}
	}

	async fn resume_tasks(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext,
		SerializedTasks(serialized_tasks): SerializedTasks,
	) -> Result<(), Error> {
		let location_id = self.location.id;

		self.pending_tasks_on_resume = dispatcher
			.dispatch_many_boxed(
				rmp_serde::from_slice::<Vec<(TaskKind, Vec<u8>)>>(&serialized_tasks)
					.map_err(IndexerError::from)?
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
					.map_err(IndexerError::from)?,
			)
			.await;

		Ok(())
	}
}

async fn determine_initial_walk_path(
	location_id: location::id::Type,
	sub_path: &Option<PathBuf>,
	location_path: &Path,
	db: &PrismaClient,
) -> Result<PathBuf, IndexerError> {
	match sub_path {
		Some(sub_path) if sub_path != Path::new("") => {
			let full_path = ensure_sub_path_is_in_location(location_path, sub_path)
				.await
				.map_err(IndexerError::from)?;
			ensure_sub_path_is_directory(location_path, sub_path)
				.await
				.map_err(IndexerError::from)?;

			ensure_file_path_exists(
				sub_path,
				&IsolatedFilePathData::new(location_id, location_path, &full_path, true)
					.map_err(IndexerError::from)?,
				db,
				IndexerError::SubPathNotFound,
			)
			.await?;

			Ok(full_path)
		}
		_ => Ok(location_path.to_path_buf()),
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IsoFilePathFactory {
	pub location_id: location::id::Type,
	pub location_path: Arc<PathBuf>,
}

impl walker::IsoFilePathFactory for IsoFilePathFactory {
	fn build(
		&self,
		path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<IsolatedFilePathData<'static>, FilePathError> {
		IsolatedFilePathData::new(self.location_id, self.location_path.as_ref(), path, is_dir)
	}
}

#[derive(Debug, Clone)]
struct WalkerDBProxy {
	location_id: location::id::Type,
	db: Arc<PrismaClient>,
}

impl walker::WalkerDBProxy for WalkerDBProxy {
	async fn fetch_file_paths(
		&self,
		found_paths: Vec<file_path::WhereParam>,
	) -> Result<Vec<file_path_walker::Data>, IndexerError> {
		// Each found path is a AND with 4 terms, and SQLite has a expression tree limit of 1000 terms
		// so we will use chunks of 200 just to be safe
		self.db
			._batch(
				found_paths
					.into_iter()
					.chunks(200)
					.into_iter()
					.map(|founds| {
						self.db
							.file_path()
							.find_many(vec![or(founds.collect::<Vec<_>>())])
							.select(file_path_walker::select())
					})
					.collect::<Vec<_>>(),
			)
			.await
			.map(|fetched| fetched.into_iter().flatten().collect::<Vec<_>>())
			.map_err(Into::into)
	}

	async fn fetch_file_paths_to_remove(
		&self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		unique_location_id_materialized_path_name_extension_params: Vec<file_path::WhereParam>,
	) -> Result<Vec<file_path_pub_and_cas_ids::Data>, NonCriticalIndexerError> {
		// NOTE: This batch size can be increased if we wish to trade memory for more performance
		const BATCH_SIZE: i64 = 1000;

		let founds_ids = self
			.db
			._batch(
				unique_location_id_materialized_path_name_extension_params
					.into_iter()
					.chunks(200)
					.into_iter()
					.map(|unique_params| {
						self.db
							.file_path()
							.find_many(vec![or(unique_params.collect())])
							.select(file_path::select!({ id }))
					})
					.collect::<Vec<_>>(),
			)
			.await
			.map(|founds_chunk| {
				founds_chunk
					.into_iter()
					.flat_map(|file_paths| file_paths.into_iter().map(|file_path| file_path.id))
					.collect::<HashSet<_>>()
			})
			.map_err(|e| NonCriticalIndexerError::FetchAlreadyExistingFilePathIds(e.to_string()))?;

		let mut to_remove = vec![];
		let mut cursor = 1;

		loop {
			let found = self
				.db
				.file_path()
				.find_many(vec![
					file_path::location_id::equals(Some(self.location_id)),
					file_path::materialized_path::equals(Some(
						parent_iso_file_path
							.materialized_path_for_children()
							.expect("the received isolated file path must be from a directory"),
					)),
				])
				.order_by(file_path::id::order(SortOrder::Asc))
				.take(BATCH_SIZE)
				.cursor(file_path::id::equals(cursor))
				.select(file_path::select!({ id pub_id cas_id }))
				.exec()
				.await
				.map_err(|e| NonCriticalIndexerError::FetchFilePathsToRemove(e.to_string()))?;

			#[allow(clippy::cast_possible_truncation)] // Safe because we are using a constant
			let should_stop = found.len() < BATCH_SIZE as usize;

			if let Some(last) = found.last() {
				cursor = last.id;
			} else {
				break;
			}

			to_remove.extend(
				found
					.into_iter()
					.filter(|file_path| !founds_ids.contains(&file_path.id))
					.map(|file_path| file_path_pub_and_cas_ids::Data {
						id: file_path.id,
						pub_id: file_path.pub_id,
						cas_id: file_path.cas_id,
					}),
			);

			if should_stop {
				break;
			}
		}

		Ok(to_remove)
	}
}

async fn remove_non_existing_file_paths(
	to_remove: Vec<file_path_pub_and_cas_ids::Data>,
	db: &PrismaClient,
	sync: &sd_core_sync::Manager,
) -> Result<u64, IndexerError> {
	use sd_sync::OperationFactory;
	#[allow(clippy::cast_sign_loss)]
	let (sync_params, db_params): (Vec<_>, Vec<_>) = to_remove
		.into_iter()
		.map(|file_path| {
			(
				sync.shared_delete(prisma_sync::file_path::SyncId {
					pub_id: file_path.pub_id,
				}),
				file_path.id,
			)
		})
		.unzip();

	sync.write_ops(
		db,
		(
			sync_params,
			db.file_path()
				.delete_many(vec![file_path::id::in_vec(db_params)]),
		),
	)
	.await
	.map(
		#[allow(clippy::cast_sign_loss)]
		|count| count as u64,
	)
	.map_err(Into::into)
}
