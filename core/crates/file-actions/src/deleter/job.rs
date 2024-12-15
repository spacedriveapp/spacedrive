use std::{
	hash::Hash,
	marker::PhantomData,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
};

use futures::{stream::FuturesUnordered, StreamExt};

use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_heavy_lifting::{
	job_system::{
		job::{Job, JobReturn, JobTaskDispatcher, ReturnStatus},
		utils::cancel_pending_tasks,
		SerializableJob, SerializedTasks,
	},
	Error, JobContext, JobName, NonCriticalError, OuterContext, ProgressUpdate,
};
use sd_core_prisma_helpers::file_path_with_object;
use sd_prisma::prisma::{file_path, location, PrismaClient};
use sd_task_system::{SerializableTask, Task, TaskDispatcher, TaskHandle, TaskStatus};
use serde::{Deserialize, Serialize};

use super::{tasks, DeleteBehavior, FileData};

#[derive(Debug)]
pub struct DeleterJob<B, C> {
	paths: Vec<PathBuf>,
	use_trash: bool,
	check_index: bool,

	pending_tasks: Option<Vec<TaskHandle<Error>>>,
	shutdown_tasks: Option<Vec<Box<dyn Task<Error>>>>,
	accumulative_errors: Option<Vec<Error>>,

	behavior: PhantomData<fn(B) -> B>, // variance: invariant, inherent Send + Sync
	_context: PhantomData<C>,
}

enum InnerTaskType {
	Delete,
	MoveToTrash,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleterState {
	paths: Vec<PathBuf>,
	use_trash: bool,
	check_index: bool,

	shutdown_tasks: Option<SerializedTasks>,
	accumulative_errors: Option<Vec<NonCriticalError>>,
}

impl<B: DeleteBehavior, C> Hash for DeleterJob<B, C> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.paths.hash(state);
		self.use_trash.hash(state);
		self.check_index.hash(state);
	}
}

impl<B: DeleteBehavior + Hash, C> DeleterJob<B, C> {
	pub const fn new(paths: Vec<PathBuf>, use_trash: bool, check_index: bool) -> Self {
		Self {
			paths,
			use_trash,
			check_index,

			behavior: PhantomData,

			accumulative_errors: None,
			pending_tasks: None,
			shutdown_tasks: None,
			_context: PhantomData,
		}
	}
}

impl<B: DeleteBehavior + Hash + Send + 'static, C> Job<C> for DeleterJob<B, C> {
	const NAME: JobName = JobName::Delete;

	async fn run<OuterCtx: OuterContext>(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: &impl JobContext<C>,
	) -> Result<ReturnStatus, Error> {
		self.check_index_compatibility(ctx).await?;

		let tasks = self.create_delete_tasks(ctx).await?;

		let mut tasks = FuturesUnordered::from_iter(tasks);

		let mut return_status = None;

		while let Some(result) = tasks.next().await {
			match result {
				Ok(TaskStatus::Done(_)) => {}
				Ok(TaskStatus::Shutdown(task)) => {
					self.shutdown_tasks.get_or_insert_with(Vec::new).push(task);
				}
				Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
					cancel_pending_tasks(&mut tasks).await;
					let _return = ReturnStatus::Canceled(
						JobReturn::builder()
							// .with_non_critical_errors()
							.build(),
					);
					return_status = Some(Ok(_return));
					break;
				}
				Ok(TaskStatus::Error(error)) => {
					cancel_pending_tasks(&mut tasks).await;
					self.accumulative_errors.get_or_insert_default().push(error);
					break;
				}

				Err(error) => {
					cancel_pending_tasks(&mut tasks).await;
					return_status = Some(Err(error));
					break;
				}
			}
		}

		match return_status {
			Some(status) => Ok(status?),
			None => {
				Ok(ReturnStatus::Completed(
					JobReturn::builder()
						// .with_non_critical_errors(errors)
						.build(),
				))
			}
		}
	}
}

impl<OuterCtx, B, C> SerializableJob<OuterCtx> for DeleterJob<B, C>
where
	OuterCtx: OuterContext,
	B: DeleteBehavior + Send + Hash + 'static,
	C: Send + Sync + 'static,
	tasks::RemoveTask<B>: SerializableTask<Error>,
{
	async fn serialize(mut self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let serialized_shutdown_tasks = self
			.shutdown_tasks
			.take()
			.unwrap_or_default()
			.into_iter()
			.map(|task| async move {
				task.downcast::<tasks::RemoveTask<B>>()
					.expect("it's known because of the bound in the impl block")
					.serialize()
					.await
			})
			.collect::<Vec<_>>()
			.try_join()
			.await
			.unwrap();

		let serialized_tasks_bytes = rmp_serde::to_vec_named(&serialized_shutdown_tasks)
			.map(SerializedTasks)
			.unwrap();

		rmp_serde::to_vec_named(&DeleterState {
			paths: self.paths,
			use_trash: self.use_trash,
			check_index: self.check_index,
			shutdown_tasks: Some(serialized_tasks_bytes),
			// TODO(matheus-consoli):
			accumulative_errors: None,
		})
		.map(Some)
	}

	async fn deserialize(
		serialized_job: &[u8],
		_: &OuterCtx,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let mut job = rmp_serde::from_slice::<DeleterState>(serialized_job)?;
		let tasks = job.shutdown_tasks.take();

		let job = Self {
			paths: job.paths,
			use_trash: job.use_trash,
			check_index: job.check_index,
			accumulative_errors: None, //  TODO(matheus-consoli):  job.accumulative_errors
			shutdown_tasks: None,
			pending_tasks: None,
			behavior: PhantomData,
			_context: PhantomData,
		};
		Ok(Some((job, tasks)))
	}
}

async fn get_location_path_from_location_id(
	db: &PrismaClient,
	location_id: file_path::id::Type,
) -> Result<PathBuf, sd_utils::error::FileIOError> {
	db.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await
		.map_err(Into::into)
		.and_then(|maybe_location| {
			maybe_location
				.ok_or_else(|| sd_utils::error::FileIOError::LocationIdNotFound(location_id))
				.and_then(|location| {
					location.path.map(PathBuf::from).ok_or_else(|| {
						sd_utils::error::FileIOError::LocationMissingPath(location_id)
					})
				})
		})
}

impl<B: DeleteBehavior + Hash + Send + 'static, C> DeleterJob<B, C> {
	async fn check_index_compatibility(&self, ctx: &impl JobContext<C>) -> Result<(), Error> {
		if !self.check_index {
			return Ok(());
		}

		if let Some(location_id) = self.location_id {
			for path in &self.paths {
				if let Ok(Some(file_path)) = ctx
					.db()
					.file_path()
					.find_first(vec![
						file_path::location_id::equals(Some(location_id)),
						file_path::materialized_path::equals(path.to_string_lossy().to_string()),
					])
					.exec()
					.await
				{
					if !path.exists() {
						return Err(Error::InvalidInput(format!(
							"File {} exists in index but not on current OS",
							path.display()
						)));
					}
				}
			}
		}

		Ok(())
	}

	async fn create_delete_tasks(
		&self,
		ctx: &impl JobContext<C>,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, Error> {
		let mut tasks: Vec<Box<dyn Task<Error = Error>>> = Vec::new();

		let mut files = Vec::new();
		let mut dirs = Vec::new();

		for path in &self.paths {
			if path.is_file() {
				files.push(path.clone());
			} else if path.is_dir() {
				dirs.push(path.clone());
			}
		}

		if self.use_trash {
			if !self.paths.is_empty() {
				tasks.push(Box::new(tasks::MoveToTrashTask::new(self.paths.clone())));
			}
		} else {
			if !files.is_empty() {
				tasks.push(Box::new(tasks::RemoveTask::new(files, false)));
			}

			if !dirs.is_empty() {
				tasks.push(Box::new(tasks::RemoveTask::new(dirs, true)));
			}
		}

		Ok(tasks)
	}
}
