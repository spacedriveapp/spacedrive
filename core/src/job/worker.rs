use crate::invalidate_query;
use crate::job::{DynJob, JobError, JobManager, JobReportUpdate, JobStatus};
use crate::library::Library;
use chrono::{DateTime, Utc};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar};
use tokio::select;
use tokio::sync::{
	broadcast,
	mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
	Mutex,
};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

use super::{JobMetadata, JobReport, JobRunErrors};

// used to update the worker state from inside the worker thread
#[derive(Debug)]
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Completed(oneshot::Sender<()>, JobMetadata),
	CompletedWithErrors(oneshot::Sender<()>, JobMetadata, JobRunErrors),
	Failed(oneshot::Sender<()>),
	Paused(Vec<u8>, oneshot::Sender<()>),
}

// used to send commands to the worker thread from the manager
#[derive(Debug)]
pub enum WorkerCommand {
	Pause,
	Resume,
	Cancel,
}

#[derive(Clone)]
pub struct WorkerContext {
	pub library: Library,
	events_tx: UnboundedSender<WorkerEvent>,
	command_rx: Arc<Mutex<UnboundedReceiver<WorkerCommand>>>,
}

impl WorkerContext {
	pub fn progress(&self, updates: Vec<JobReportUpdate>) {
		self.events_tx
			.send(WorkerEvent::Progressed(updates))
			.expect("critical error: failed to send worker worker progress event updates");
	}
}

// a worker is a dedicated thread that runs a single job
// once the job is complete the worker will exit
pub struct Worker {
	job: Option<Box<dyn DynJob>>,
	report: JobReport,
	events_tx: UnboundedSender<WorkerEvent>,
	events_rx: Option<UnboundedReceiver<WorkerEvent>>,
	command_tx: Option<UnboundedSender<WorkerCommand>>,
	start_time: Option<DateTime<Utc>>,
}

impl Worker {
	pub fn new(job: Box<dyn DynJob>, report: JobReport) -> Self {
		let (events_tx, events_rx) = unbounded_channel();

		Self {
			job: Some(job),
			report,
			events_tx,
			events_rx: Some(events_rx),
			command_tx: None,
			start_time: None,
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
		let events_tx = worker.events_tx.clone();
		let events_rx = worker
			.events_rx
			.take()
			.expect("critical error: missing worker events rx");

		// create command channel to send commands to the worker
		let (command_tx, command_rx) = unbounded_channel();
		let command_rx = Arc::new(Mutex::new(command_rx));
		worker.command_tx = Some(command_tx);

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

		worker.start_time = Some(Utc::now());

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
		invalidate_query!(library, "jobs.getHistory");

		// spawn task to handle receiving events from the worker
		tokio::spawn(Worker::track_progress(
			Arc::clone(&worker_mutex),
			events_rx,
			library.clone(),
		));

		// spawn task to handle running the job
		tokio::spawn(async move {
			let worker_ctx = WorkerContext {
				library: library.clone(),
				events_tx,
				command_rx,
			};

			let (done_tx, done_rx) = oneshot::channel::<()>();
			let done_tx = Arc::new(Mutex::new(Some(done_tx)));

			'outer: loop {
				let mut command_rx = worker_ctx.command_rx.lock().await;

				select! {
					command = command_rx.recv() => {
						if let Some(command) = command {
							println!("Worker received command: {:?}", command);
							match command {
								WorkerCommand::Pause => {
									println!("Worker handled pause command");

									'pause: loop {
										if let Some(command) = command_rx.recv().await {
											match command {
												WorkerCommand::Resume => {
													println!("Worker handled resume command");
													break 'pause;
												}
												WorkerCommand::Cancel => {
													println!("Worker handled cancel command");
													break 'outer;
												}
												_ => (),
											}
										}
									}
								}
								WorkerCommand::Cancel => {
									println!("Worker received cancel command");
									break 'outer;
								}
								_ => (),
							}
						}
					}
					_ = async {
						println!("Worker running job: {:?}", job_hash);

						match job.run(job_manager.clone(), worker_ctx.clone()).await {
							Ok((metadata, errors)) if errors.is_empty() => {
								let local_done_tx = done_tx.lock().await.take();
								if let Some(tx) = local_done_tx {
									worker_ctx
										.events_tx
										.send(WorkerEvent::Completed(tx, metadata))
										.expect("critical error: failed to send worker complete event");
								}

							}
							Ok((metadata, errors)) => {
								warn!("Job<id'{job_id}'> completed with errors");
								let local_done_tx = done_tx.lock().await.take();
								if let Some(tx) = local_done_tx {
									worker_ctx
										.events_tx
										.send(WorkerEvent::CompletedWithErrors(tx, metadata, errors))
										.expect("critical error: failed to send worker complete event");
								}

							}
							Err(JobError::Paused(state)) => {
								info!("Job<id='{job_id}'> paused, we will pause all children jobs");
								if let Err(e) = job.pause_children(&library).await {
									error!("Failed to pause children jobs: {e:#?}");
								}

								let local_done_tx = done_tx.lock().await.take();
								if let Some(tx) = local_done_tx {
									worker_ctx
										.events_tx
										.send(WorkerEvent::Paused(state, tx))
										.expect("critical error: failed to send worker pause event");
								}
							}
							Err(e) => {
								error!("Job<id='{job_id}'> failed with error: {e:#?}; We will cancel all children jobs");
								if let Err(e) = job.cancel_children(&library).await {
									error!("Failed to cancel children jobs: {e:#?}");
								}

								let local_done_tx = done_tx.lock().await.take();
								if let Some(tx) = local_done_tx {
									worker_ctx
										.events_tx
										.send(WorkerEvent::Failed(tx))
										.expect("critical error: failed to send worker fail event");
								}

							}
						}
					} => {}
				}
			}

			if let Err(e) = done_rx.await {
				error!("failed to wait for worker completion: {:#?}", e);
			}

			job_manager.complete(&library, job_id, job_hash).await;
		});

		Ok(())
	}

	// send command to worker from job manager
	pub fn command(&self, command: WorkerCommand) -> Result<(), JobError> {
		info!("Sending command to worker: {:#?}", command);
		if let Some(tx) = &self.command_tx {
			let tx = tx.clone();
			tx.send(command)
				.map_err(|_| JobError::WorkerCommandSendFailed)
		} else {
			Err(JobError::WorkerCommandSendFailed)
		}
	}

	async fn track_progress(
		worker: Arc<Mutex<Self>>,
		mut events_rx: UnboundedReceiver<WorkerEvent>,
		library: Library,
	) {
		while let Some(command) = events_rx.recv().await {
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
					// Calculate elapsed time
					if let Some(start_time) = worker.start_time {
						let elapsed = Utc::now() - start_time;

						// Calculate remaining time
						let task_count = worker.report.task_count as usize;
						let completed_task_count = worker.report.completed_task_count as usize;
						let remaining_task_count = task_count.saturating_sub(completed_task_count);
						let remaining_time_per_task = elapsed / (completed_task_count + 1) as i32; // Adding 1 to avoid division by zero
						let remaining_time = remaining_time_per_task * remaining_task_count as i32;

						// Update the report with estimated remaining time
						worker.report.estimated_completion = Utc::now()
							.checked_add_signed(remaining_time)
							.unwrap_or(Utc::now());
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
				WorkerEvent::CompletedWithErrors(done_tx, metadata, errors) => {
					worker.report.status = JobStatus::CompletedWithErrors;
					worker.report.errors_text = errors;
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
					invalidate_query!(library, "jobs.getRunning");
					invalidate_query!(library, "jobs.getHistory");

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

					invalidate_query!(library, "jobs.getRunning");
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
