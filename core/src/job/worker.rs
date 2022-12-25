use crate::invalidate_query;
use crate::job::{DynJob, JobError, JobManager, JobReportUpdate, JobStatus};
use crate::library::LibraryContext;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

use super::JobReport;

pub struct WorkerContext {
	report: JobReport,
	pub library_ctx: LibraryContext,
	shutdown_tx: Arc<broadcast::Sender<()>>,
}

impl WorkerContext {
	pub fn progress(&mut self, updates: Vec<JobReportUpdate>) {
		self.progress_inner(updates, false);
	}

	pub fn progress_debounced(&mut self, updates: Vec<JobReportUpdate>) {
		self.progress_inner(updates, true);
	}

	fn progress_inner(&mut self, updates: Vec<JobReportUpdate>, debounce: bool) {
		// protect against updates if job is not running
		if self.report.status != JobStatus::Running {
			warn!("Attempted to update job progress while job is not running");
			return;
		};
		for update in updates {
			match update {
				JobReportUpdate::TaskCount(task_count) => {
					self.report.task_count = task_count as i32;
				}
				JobReportUpdate::CompletedTaskCount(completed_task_count) => {
					self.report.completed_task_count = completed_task_count as i32;
				}
				JobReportUpdate::Message(message) => {
					self.report.message = message;
				}
				JobReportUpdate::SecondsElapsed(seconds) => {
					self.report.seconds_elapsed += seconds as i32;
				}
			}
		}

		// TODO: Copy the prototype sender level debounce onto this invalidate_query call to respect argument.

		// TODO: invalidate query without library context and just reference to channel
		invalidate_query!(self.library_ctx, "jobs.getRunning");
	}

	pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
		self.shutdown_tx.subscribe()
	}
}

// a worker is a dedicated thread that runs a single job
// once the job is complete the worker will exit
pub struct Worker {
	job: Option<Box<dyn DynJob>>,
	worker_ctx: WorkerContext,
}

impl Worker {
	pub fn new(
		job: Box<dyn DynJob>,
		report: JobReport,
		library_ctx: LibraryContext,
		shutdown_tx: Arc<broadcast::Sender<()>>,
	) -> Self {
		Self {
			job: Some(job),
			worker_ctx: WorkerContext {
				report,
				library_ctx,
				shutdown_tx,
			},
		}
	}

	pub fn report(&self) -> JobReport {
		self.worker_ctx.report.clone()
	}

	// spawns a thread and extracts channel sender to communicate with it
	pub async fn spawn(
		job_manager: Arc<JobManager>,
		worker_mutex: Arc<Mutex<Self>>,
		library_ctx: LibraryContext,
	) -> Result<(), JobError> {
		let mut worker = worker_mutex.lock().await;
		let mut job = worker
			.job
			.take()
			.expect("critical error: missing job on worker");

		let job_hash = job.hash();
		let job_id = worker.worker_ctx.report.id;
		let old_status = worker.worker_ctx.report.status;

		worker.worker_ctx.report.status = JobStatus::Running;

		if matches!(old_status, JobStatus::Queued) {
			worker.worker_ctx.report.create(&library_ctx).await?;
		} else {
			worker.worker_ctx.report.update(&library_ctx).await?;
		}
		drop(worker);

		invalidate_query!(library_ctx, "jobs.isRunning");

		// spawn task to handle running the job
		tokio::spawn(async move {
			let mut worker = worker_mutex.lock().await; // TODO: Is holding this lock gonna cause problems???
			match job.run(&mut worker.worker_ctx).await {
				Ok(metadata) => {
					// handle completion
					worker.worker_ctx.report.status = JobStatus::Completed;
					worker.worker_ctx.report.data = None;
					worker.worker_ctx.report.metadata = metadata;
					if let Err(e) = worker
						.worker_ctx
						.report
						.update(&worker.worker_ctx.library_ctx)
						.await
					{
						error!("failed to update job report: {:#?}", e);
					}
					info!("{}", worker.worker_ctx.report);

					// TODO: Invalidate without `LibraryCtx`
					invalidate_query!(worker.worker_ctx.library_ctx, "jobs.isRunning");
					invalidate_query!(worker.worker_ctx.library_ctx, "jobs.getRunning");
					invalidate_query!(worker.worker_ctx.library_ctx, "jobs.getHistory");
				}
				Err(JobError::Paused(state)) => {
					worker.worker_ctx.report.status = JobStatus::Paused;
					worker.worker_ctx.report.data = Some(state);
					if let Err(e) = worker
						.worker_ctx
						.report
						.update(&worker.worker_ctx.library_ctx)
						.await
					{
						error!("failed to update job report: {:#?}", e);
					}
					info!("{}", worker.worker_ctx.report);

					invalidate_query!(worker.worker_ctx.library_ctx, "jobs.getHistory");
				}
				Err(e) => {
					error!("job '{}' failed with error: {:#?}", job_id, e);

					worker.worker_ctx.report.status = JobStatus::Failed;
					worker.worker_ctx.report.data = None;
					if let Err(e) = worker
						.worker_ctx
						.report
						.update(&worker.worker_ctx.library_ctx)
						.await
					{
						error!("failed to update job report: {:#?}", e);
					}
					warn!("{}", worker.worker_ctx.report);

					invalidate_query!(worker.worker_ctx.library_ctx, "library.list");
				}
			}

			job_manager.complete(&library_ctx, job_id, job_hash).await;
		});

		Ok(())
	}
}
