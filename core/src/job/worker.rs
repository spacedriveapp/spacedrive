use crate::invalidate_query;
use crate::job::{DynJob, JobError, JobManager, JobReportUpdate, JobStatus};
use crate::library::Library;
use chrono::Utc;
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;
use tokio::{
	sync::{
		broadcast,
		mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
		Mutex,
	},
	time::Instant,
};
use tracing::{error, info, warn};

use super::{JobMetadata, JobReport};

const JOB_REPORT_UPDATE_INTERVAL: Duration = Duration::from_millis(1000 / 60);

// used to update the worker state from inside the worker thread
#[derive(Debug)]
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Completed(oneshot::Sender<()>, JobMetadata),
	Failed(oneshot::Sender<()>),
	Paused(Vec<u8>, oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct WorkerContext {
	pub library: Library,
	events_tx: UnboundedSender<WorkerEvent>,
	shutdown_tx: Arc<broadcast::Sender<()>>,
	// Used for debouncing
	last_event: Instant,
}

impl WorkerContext {
	pub fn progress(&self, updates: Vec<JobReportUpdate>) {
		self.events_tx
			.send(WorkerEvent::Progressed(updates))
			.expect("critical error: failed to send worker worker progress event updates");
	}

	pub fn progress_debounced(&mut self, updates: Vec<JobReportUpdate>) {
		let now = Instant::now();
		if self.last_event.duration_since(now) > JOB_REPORT_UPDATE_INTERVAL {
			self.last_event = now;

			self.events_tx
				.send(WorkerEvent::Progressed(updates))
				.expect("critical error: failed to send worker worker progress event updates");
		}
	}

	pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
		self.shutdown_tx.subscribe()
	}
}

// a worker is a dedicated thread that runs a single job
// once the job is complete the worker will exit
pub struct Worker {
	job: Option<Box<dyn DynJob>>,
	report: JobReport,
	worker_events_tx: UnboundedSender<WorkerEvent>,
	worker_events_rx: Option<UnboundedReceiver<WorkerEvent>>,
}

impl Worker {
	pub fn new(job: Box<dyn DynJob>, report: JobReport) -> Self {
		let (worker_events_tx, worker_events_rx) = unbounded_channel();

		Self {
			job: Some(job),
			report,
			worker_events_tx,
			worker_events_rx: Some(worker_events_rx),
		}
	}

	pub fn report(&self) -> JobReport {
		self.report.clone()
	}
	// spawns a thread and extracts channel sender to communicate with it
	pub async fn spawn(
		job_manager: Arc<JobManager>,
		worker_mutex: Arc<Mutex<Self>>,
		library: Library,
	) -> Result<(), JobError> {
		let mut worker = worker_mutex.lock().await;
		// we capture the worker receiver channel so state can be updated from inside the worker
		let worker_events_tx = worker.worker_events_tx.clone();
		let worker_events_rx = worker
			.worker_events_rx
			.take()
			.expect("critical error: missing worker events rx");

		let mut job = worker
			.job
			.take()
			.expect("critical error: missing job on worker");

		let job_hash = job.hash();
		let job_id = worker.report.id;

		worker.report.status = JobStatus::Running;
		if worker.report.started_at.is_none() {
			worker.report.started_at = Some(Utc::now());
		}

		// If the report doesn't have a created_at date, it's a new report
		if worker.report.created_at.is_none() {
			worker.report.create(&library).await?;
		} else {
			// Otherwise it can be a job being resumed or a children job that was already been created
			worker.report.update(&library).await?;
		}
		drop(worker);

		job.register_children(&library).await?;

		invalidate_query!(library, "jobs.getRunning");

		// spawn task to handle receiving events from the worker
		tokio::spawn(Worker::track_progress(
			Arc::clone(&worker_mutex),
			worker_events_rx,
			library.clone(),
		));

		// spawn task to handle running the job
		tokio::spawn(async move {
			let worker_ctx = WorkerContext {
				library: library.clone(),
				events_tx: worker_events_tx,
				shutdown_tx: job_manager.shutdown_tx(),
				last_event: (Instant::now()
					- (JOB_REPORT_UPDATE_INTERVAL + Duration::from_secs(1))), // So we don't miss the first event
			};

			let (done_tx, done_rx) = oneshot::channel();

			match job.run(job_manager.clone(), worker_ctx.clone()).await {
				Ok(metadata) => {
					// handle completion
					worker_ctx
						.events_tx
						.send(WorkerEvent::Completed(done_tx, metadata))
						.expect("critical error: failed to send worker complete event");
				}
				Err(JobError::Paused(state)) => {
					info!("Job <id='{job_id}'> paused, we will pause all children jobs");
					if let Err(e) = job.pause_children(&library).await {
						error!("Failed to pause children jobs: {e:#?}");
					}

					worker_ctx
						.events_tx
						.send(WorkerEvent::Paused(state, done_tx))
						.expect("critical error: failed to send worker pause event");
				}
				Err(e) => {
					error!("Job <id='{job_id}'> failed with error: {e:#?}; We will cancel all children jobs");
					if let Err(e) = job.cancel_children(&library).await {
						error!("Failed to cancel children jobs: {e:#?}");
					}

					worker_ctx
						.events_tx
						.send(WorkerEvent::Failed(done_tx))
						.expect("critical error: failed to send worker fail event");
				}
			}

			if let Err(e) = done_rx.await {
				error!("failed to wait for worker completion: {:#?}", e);
			}
			job_manager.complete(&library, job_id, job_hash).await;
		});

		Ok(())
	}

	async fn track_progress(
		worker: Arc<Mutex<Self>>,
		mut worker_events_rx: UnboundedReceiver<WorkerEvent>,
		library: Library,
	) {
		while let Some(command) = worker_events_rx.recv().await {
			let mut worker = worker.lock().await;

			match command {
				WorkerEvent::Progressed(updates) => {
					// protect against updates if job is not running
					if worker.report.status != JobStatus::Running {
						continue;
					};
					for update in updates {
						match update {
							JobReportUpdate::TaskCount(task_count) => {
								worker.report.task_count = task_count as i32;
							}
							JobReportUpdate::CompletedTaskCount(completed_task_count) => {
								worker.report.completed_task_count = completed_task_count as i32;
							}
							JobReportUpdate::Message(message) => {
								worker.report.message = message;
							}
						}
					}

					invalidate_query!(library, "jobs.getRunning");
				}
				WorkerEvent::Completed(done_tx, metadata) => {
					worker.report.status = JobStatus::Completed;
					worker.report.data = None;
					worker.report.metadata = metadata;
					worker.report.completed_at = Some(Utc::now());
					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					invalidate_query!(library, "jobs.getRunning");
					invalidate_query!(library, "jobs.getHistory");

					info!("{}", worker.report);

					done_tx
						.send(())
						.expect("critical error: failed to send worker completion");

					break;
				}
				WorkerEvent::Failed(done_tx) => {
					worker.report.status = JobStatus::Failed;
					worker.report.data = None;
					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					invalidate_query!(library, "library.list");

					warn!("{}", worker.report);

					done_tx
						.send(())
						.expect("critical error: failed to send worker completion");

					break;
				}
				WorkerEvent::Paused(state, done_tx) => {
					worker.report.status = JobStatus::Paused;
					worker.report.data = Some(state);
					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					info!("{}", worker.report);

					invalidate_query!(library, "jobs.getHistory");

					done_tx
						.send(())
						.expect("critical error: failed to send worker completion");

					break;
				}
			}
		}
	}
}
