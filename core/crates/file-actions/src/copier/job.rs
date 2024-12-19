/**
 *
 * This file contains the core file copying functionality for Spacedrive.
 * It implements a robust copy job system that handles both local and
 * cross-device file transfers with detailed progress tracking.
 *
 */
use super::tasks::{CopyTask, CreateDirsTask};
use futures::{future::try_join_all, stream::FuturesUnordered, StreamExt};
use sd_core_heavy_lifting::{
	job_system::{
		job::{Job, JobContext, JobName, JobTaskDispatcher, OuterContext, ReturnStatus},
		report::{ReportInputMetadata, ReportOutputMetadata},
		SerializableJob, SerializedTasks,
	},
	Error,
};
use sd_core_shared_types::{sd_path::SdPath, CopyOperation, CopyStats};
use sd_task_system::{Task, TaskHandle, TaskStatus};
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, time::Instant};

/// Internal state of a copy job that can be serialized
#[derive(Debug, Serialize, Deserialize)]
pub struct CopyState {
	/// List of source paths to copy
	pub sources: Vec<SdPath>,
	/// Target directory to copy into
	pub target_dir: SdPath,
	/// Tasks that were interrupted and need to be resumed
	pub shutdown_tasks: Option<SerializedTasks>,
	/// Current statistics of the copy operation
	pub stats: CopyStats,
	/// When the operation started
	pub start_time: Option<Instant>,
}

/// The main copy job that handles copying files and directories.
/// Supports both local and cross-device copies, with detailed progress tracking.
#[derive(Debug)]
pub struct CopyJob<C> {
	/// Source paths to copy
	sources: Vec<SdPath>,
	/// Target directory to copy into
	target_dir: SdPath,
	/// Tasks currently in progress
	pending_tasks: Option<Vec<TaskHandle<Error>>>,
	/// Tasks that were shutdown and need to be resumed
	shutdown_tasks: Option<Vec<Box<dyn Task<Error>>>>,
	/// Accumulated errors during the operation
	accumulative_errors: Option<Vec<Error>>,
	/// Current statistics of the copy operation
	stats: CopyStats,
	/// When the operation started
	start_time: Option<Instant>,
	/// Type parameter for the job context
	_context: PhantomData<C>,
}

impl<C> CopyJob<C> {
	pub fn new(sources: Vec<SdPath>, target_dir: SdPath) -> Self {
		Self {
			sources,
			target_dir,
			pending_tasks: None,
			shutdown_tasks: None,
			accumulative_errors: None,
			stats: CopyStats::default(),
			start_time: None,
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
		&mut self,
		_ctx: &impl JobContext<C>,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, JobError> {
		let mut tasks: Vec<Box<dyn Task<Error = Error>>> = Vec::new();

		// Count total files and bytes
		let mut total_files = 0u32;
		let mut total_bytes = 0u64;

		// Process each source
		for source in &self.sources {
			let target = self.target_dir.join(source.file_name().unwrap());
			let metadata = source.metadata().await?;

			total_files += 1;
			total_bytes += metadata.len();

			tasks.push(Box::new(CopyTask::new(source.clone(), target).await?));
		}

		// Update stats with totals
		self.stats.total_files = total_files;
		self.stats.total_bytes = total_bytes;

		Ok(tasks)
	}

	fn record_success(&mut self, stats: CopyStats) {
		let duration = stats.duration;
		self.stats.completed_files += 1;
		self.stats.completed_bytes += stats.bytes_copied;

		self.stats.operations.push(CopyOperation {
			source: stats.source,
			target: stats.target,
			size: stats.bytes_copied,
			cross_device: stats.cross_device,
			duration_ms: Some(duration.as_millis() as u64),
			error: None,
		});
	}

	fn record_failure(&mut self, source: SdPath, target: SdPath, error: Error) {
		self.stats.operations.push(CopyOperation {
			source,
			target,
			size: 0,
			cross_device: source.device() != target.device(),
			duration_ms: None,
			error: Some(error.to_string()),
		});
	}

	fn finalize_stats(&mut self) {
		if let Some(start_time) = self.start_time {
			let duration = start_time.elapsed();
			self.stats.duration_ms = duration.as_millis() as u64;

			// Calculate average speed
			if duration.as_secs() > 0 {
				self.stats.speed = self.stats.completed_bytes / duration.as_secs();
			} else if self.stats.completed_bytes > 0 {
				// If duration is less than a second but we copied something, use bytes as speed
				self.stats.speed = self.stats.completed_bytes;
			}
		}
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
		self.start_time = Some(Instant::now());

		// First create all necessary directories
		let mut tasks = self.create_directory_tasks(ctx).await?;

		// Then create copy tasks for files
		tasks.extend(self.create_copy_tasks(ctx).await?);

		let mut tasks =
			FuturesUnordered::from_iter(tasks.into_iter().map(|task| dispatcher.dispatch(task)));

		while let Some(result) = tasks.next().await {
			match result {
				Ok(task_status) => match task_status {
					TaskStatus::Complete(stats) => self.record_success(stats),
					TaskStatus::Shutdown(task) => {
						if self.shutdown_tasks.is_none() {
							self.shutdown_tasks = Some(Vec::new());
						}
						self.shutdown_tasks.as_mut().unwrap().push(task);
					}
					_ => {}
				},
				Err(e) => {
					if let Some((source, target)) = e.get_paths() {
						self.record_failure(source, target, e);
					}

					if self.accumulative_errors.is_none() {
						self.accumulative_errors = Some(Vec::new());
					}
					self.accumulative_errors.as_mut().unwrap().push(e);
				}
			}
		}

		self.finalize_stats();

		if let Some(errors) = self.accumulative_errors {
			if errors.is_empty() {
				Ok(ReturnStatus::Completed(
					JobReturn::builder()
						.with_metadata(vec![ReportOutputMetadata::Copier(self.stats.clone())])
						.build(),
				))
			} else {
				Err(Error::NonCritical(errors))
			}
		} else {
			Ok(ReturnStatus::Completed(
				JobReturn::builder()
					.with_metadata(vec![ReportOutputMetadata::Copier(self.stats.clone())])
					.build(),
			))
		}
	}

	// fn metadata(&self) -> ReportInputMetadata {
	// 	ReportInputMetadata::Copier {
	// 		sources: self.sources.clone(),
	// 		target_dir: self.target_dir.clone(),
	// 	}
	// }
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
			stats: self.stats,
			start_time: self.start_time,
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
			stats: job.stats,
			start_time: job.start_time,
			shutdown_tasks: None,
			pending_tasks: None,
			accumulative_errors: None,
			_context: PhantomData,
		};
		Ok(Some((job, tasks)))
	}
}

// #[async_trait::async_trait]
// impl<OuterCtx: OuterContext> UndoableJob<OuterCtx> for CopyJob<OuterCtx> {
// 	async fn create_undo_job(
// 		&self,
// 		ctx: &impl JobContext<OuterCtx>,
// 	) -> Result<Box<dyn SerializableJob<OuterCtx>>, Error> {
// 		// Get the report for this job
// 		let report = ctx.report().await;

// 		// Get output metadata to know which files were successfully copied
// 		let source_target_pairs = report
// 			.metadata
// 			.iter()
// 			.find_map(|m| {
// 				if let ReportMetadata::Output(ReportOutputMetadata::Copier {
// 					source_target_pairs,
// 					..
// 				}) = m
// 				{
// 					Some(source_target_pairs.clone())
// 				} else {
// 					None
// 				}
// 			})
// 			.ok_or_else(|| Error::JobError("No source-target pairs found in job report".into()))?;

// 		// Create a move job to move files back to their original locations
// 		Ok(Box::new(MoveJob::new(source_target_pairs)))
// 	}

// 	fn is_undo(&self) -> bool {
// 		false // This is not an undo operation
// 	}
// }
