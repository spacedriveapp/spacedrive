use std::{
	collections::VecDeque,
	hash::Hash,
	marker::PhantomData,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_prisma::prisma::{file_path, location};
use sd_utils::error::FileIOError;

use heavy_lifting::{
	job::{
		Job, JobContext, JobError, JobName, JobTaskDispatcher, OuterContext, ReturnStatus,
		SerializableJob, SerializedTasks,
	},
	report::{ReportInputMetadata, ReportOutputMetadata},
	task::{Task, TaskHandle, TaskId, TaskStatus},
	Error,
};

use futures::{future::try_join_all, stream::FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};

use super::tasks::{CopyTask, CreateDirsTask, batch::BatchedCopy};

#[derive(Debug)]
pub struct CopyJob<C> {
	sources: Vec<PathBuf>,
	target_dir: PathBuf,
	pending_tasks: Option<Vec<TaskHandle<Error>>>,
	shutdown_tasks: Option<Vec<Box<dyn Task<Error = Error>>>>,
	accumulative_errors: Option<Vec<Error>>,
	_context: PhantomData<C>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CopyState {
	sources: Vec<PathBuf>,
	target_dir: PathBuf,
	shutdown_tasks: Option<SerializedTasks>,
	accumulative_errors: Option<Vec<Error>>,
}

impl<C> Hash for CopyJob<C> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.sources.hash(state);
		self.target_dir.hash(state);
	}
}

impl<C> CopyJob<C> {
	pub fn new(sources: Vec<PathBuf>, target_dir: impl Into<PathBuf>) -> Self {
		Self {
			sources,
			target_dir: target_dir.into(),
			pending_tasks: None,
			shutdown_tasks: None,
			accumulative_errors: None,
			_context: PhantomData,
		}
	}

	async fn create_directory_tasks(
		&self,
		_ctx: &impl JobContext<C>,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, JobError> {
		let mut tasks: Vec<Box<dyn Task<Error = Error>>> = Vec::new();

		// Create target directory if it doesn't exist
		if !self.target_dir.exists() {
			tasks.push(Box::new(CreateDirsTask::new(
				self.target_dir.clone(),
				self.target_dir.clone(),
			)));
		}

		// Create subdirectories for each source directory
		for source in &self.sources {
			let target_path = self.target_dir.join(source.file_name().unwrap());
			if source.is_dir() {
				tasks.push(Box::new(CreateDirsTask::new(source.clone(), target_path)));
			}
		}

		Ok(tasks)
	}

	async fn create_copy_tasks(
		&self,
		_ctx: &impl JobContext<C>,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, JobError> {
		let mut tasks: Vec<Box<dyn Task<Error = Error>>> = Vec::new();

		// Process each source
		for source in &self.sources {
			let target = self.target_dir.join(source.file_name().unwrap());
			tasks.push(Box::new(CopyTask::new(source.clone(), target).await?));
		}

		Ok(tasks)
	}
}

#[async_trait::async_trait]
impl<C> Job<C> for CopyJob<C> {
	const NAME: JobName = JobName::Copy;

	async fn run(
		mut self,
		dispatcher: JobTaskDispatcher,
		ctx: &impl JobContext<C>,
	) -> Result<ReturnStatus, Error> {
		// First create all necessary directories
		let mut tasks = self.create_directory_tasks(ctx).await?;

		// Then create copy tasks for files
		tasks.extend(self.create_copy_tasks(ctx).await?);

		let mut tasks =
			FuturesUnordered::from_iter(tasks.into_iter().map(|task| dispatcher.dispatch(task)));
		let mut return_status = None;

		while let Some(result) = tasks.next().await {
			match result {
				Ok(task_status) => {
					if let TaskStatus::Shutdown(task) = task_status {
						if self.shutdown_tasks.is_none() {
							self.shutdown_tasks = Some(Vec::new());
						}
						self.shutdown_tasks.as_mut().unwrap().push(task);
					}
				}
				Err(e) => {
					if self.accumulative_errors.is_none() {
						self.accumulative_errors = Some(Vec::new());
					}
					self.accumulative_errors.as_mut().unwrap().push(e);
				}
			}
		}

		if let Some(errors) = self.accumulative_errors {
			if errors.is_empty() {
				Ok(ReturnStatus::Complete)
			} else {
				Err(Error::NonCritical(errors))
			}
		} else {
			Ok(ReturnStatus::Complete)
		}
	}

	fn metadata(&self) -> ReportInputMetadata {
		ReportInputMetadata::Copier {
			location_id: None,
			sources: self.sources.clone(),
			target_dir: self.target_dir.clone(),
		}
	}
}

#[async_trait::async_trait]
impl<OuterCtx: OuterContext> SerializableJob<OuterCtx> for CopyJob<OuterCtx> {
	async fn serialize(mut self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		let serialized_shutdown_tasks = try_join_all(
			self.shutdown_tasks
				.take()
				.unwrap_or_default()
				.into_iter()
				.map(|task| async move {
					task.downcast::<CopyTask>()
						.expect("it's known because of the bound in the impl block")
						.serialize()
						.await
				}),
		)
		.await
		.unwrap();

		let serialized_tasks_bytes = rmp_serde::to_vec_named(&serialized_shutdown_tasks)
			.map(SerializedTasks)
			.unwrap();

		rmp_serde::to_vec_named(&CopyState {
			sources: self.sources,
			target_dir: self.target_dir,
			shutdown_tasks: Some(serialized_tasks_bytes),
			accumulative_errors: self.accumulative_errors,
		})
		.map(Some)
	}

	async fn deserialize(
		serialized_job: &[u8],
		_: &OuterCtx,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		let mut job = rmp_serde::from_slice::<CopyState>(serialized_job)?;
		let tasks = job.shutdown_tasks.take();

		let job = Self {
			sources: job.sources,
			target_dir: job.target_dir,
			accumulative_errors: job.accumulative_errors,
			shutdown_tasks: None,
			pending_tasks: None,
			_context: PhantomData,
		};
		Ok(Some((job, tasks)))
	}
}
