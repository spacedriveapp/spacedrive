use crate::job::{DynJob, JobError, JobManager, JobReportUpdate, JobStatus};
use crate::library::LibraryContext;
use crate::{api::LibraryArgs, invalidate_query};
use std::{sync::Arc, time::Duration};
use tokio::{
	sync::{
		broadcast,
		mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
		Mutex,
	},
	time::{interval_at, Instant},
};
use tracing::{error, info, warn};

use super::JobReport;

// used to update the worker state from inside the worker thread
#[derive(Debug)]
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Completed,
	Failed,
	Paused(Vec<u8>),
}

#[derive(Clone)]
pub struct WorkerContext {
	library_ctx: LibraryContext,
	events_tx: UnboundedSender<WorkerEvent>,
	shutdown_tx: Arc<broadcast::Sender<()>>,
}

impl WorkerContext {
	pub fn progress(&self, updates: Vec<JobReportUpdate>) {
		self.events_tx
			.send(WorkerEvent::Progressed(updates))
			.expect("critical error: failed to send worker worker progress event updates");
	}

	pub fn library_ctx(&self) -> LibraryContext {
		self.library_ctx.clone()
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
		ctx: LibraryContext,
	) {
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

		let job_id = worker.report.id;
		let old_status = worker.report.status;
		worker.report.status = JobStatus::Running;
		if matches!(old_status, JobStatus::Queued) {
			worker.report.create(&ctx).await.unwrap();
		}
		drop(worker);

		// spawn task to handle receiving events from the worker
		let library_ctx = ctx.clone();
		tokio::spawn(Worker::track_progress(
			Arc::clone(&worker_mutex),
			worker_events_rx,
			library_ctx.clone(),
		));

		// spawn task to handle running the job
		tokio::spawn(async move {
			let worker_ctx = WorkerContext {
				library_ctx,
				events_tx: worker_events_tx,
				shutdown_tx: job_manager.shutdown_tx(),
			};

			// track time
			let events_tx = worker_ctx.events_tx.clone();
			tokio::spawn(async move {
				let mut interval = interval_at(
					Instant::now() + Duration::from_millis(1000),
					Duration::from_millis(1000),
				);
				loop {
					interval.tick().await;
					if events_tx
						.send(WorkerEvent::Progressed(vec![
							JobReportUpdate::SecondsElapsed(1),
						]))
						.is_err() && events_tx.is_closed()
					{
						break;
					}
				}
			});

			if let Err(e) = job.run(worker_ctx.clone()).await {
				if let JobError::Paused(state) = e {
					worker_ctx
						.events_tx
						.send(WorkerEvent::Paused(state))
						.expect("critical error: failed to send worker pause event");
				} else {
					error!("job '{}' failed with error: {:#?}", job_id, e);
					worker_ctx
						.events_tx
						.send(WorkerEvent::Failed)
						.expect("critical error: failed to send worker fail event");
				}
			} else {
				// handle completion
				worker_ctx
					.events_tx
					.send(WorkerEvent::Completed)
					.expect("critical error: failed to send worker complete event");
			}

			job_manager.complete(&ctx, job_id).await;
		});
	}

	async fn track_progress(
		worker: Arc<Mutex<Self>>,
		mut worker_events_rx: UnboundedReceiver<WorkerEvent>,
		library: LibraryContext,
	) {
		while let Some(command) = worker_events_rx.recv().await {
			let mut worker = worker.lock().await;

			match command {
				WorkerEvent::Progressed(changes) => {
					// protect against updates if job is not running
					if worker.report.status != JobStatus::Running {
						continue;
					};
					for change in changes {
						match change {
							JobReportUpdate::TaskCount(task_count) => {
								worker.report.task_count = task_count as i32;
							}
							JobReportUpdate::CompletedTaskCount(completed_task_count) => {
								worker.report.completed_task_count = completed_task_count as i32;
							}
							JobReportUpdate::Message(message) => {
								worker.report.message = message;
							}
							JobReportUpdate::SecondsElapsed(seconds) => {
								worker.report.seconds_elapsed += seconds as i32;
							}
						}
					}

					invalidate_query!(
						library,
						"jobs.getRunning": LibraryArgs<()>,
						LibraryArgs {
							library_id: library.id,
							arg: ()
						}
					);
				}
				WorkerEvent::Completed => {
					worker.report.status = JobStatus::Completed;
					worker.report.data = None;
					worker
						.report
						.update(&library)
						.await
						.expect("critical error: failed to update job report");

					invalidate_query!(
						library,
						"jobs.getRunning": LibraryArgs<()>,
						LibraryArgs {
							library_id: library.id,
							arg: ()
						}
					);

					invalidate_query!(
						library,
						"jobs.getHistory": LibraryArgs<()>,
						LibraryArgs {
							library_id: library.id,
							arg: ()
						}
					);

					info!("{}", worker.report);

					break;
				}
				WorkerEvent::Failed => {
					worker.report.status = JobStatus::Failed;
					worker.report.data = None;
					worker
						.report
						.update(&library)
						.await
						.expect("critical error: failed to update job report");

					invalidate_query!(library, "library.get": (), ());

					warn!("{}", worker.report);

					break;
				}
				WorkerEvent::Paused(state) => {
					worker.report.status = JobStatus::Paused;
					worker.report.data = Some(state);
					worker
						.report
						.update(&library)
						.await
						.expect("critical error: failed to update job report");
					info!("{}", worker.report);

					invalidate_query!(
						library,
						"jobs.getHistory": LibraryArgs<()>,
						LibraryArgs {
							library_id: library.id,
							arg: ()
						}
					);

					break;
				}
			}
		}
	}
}
