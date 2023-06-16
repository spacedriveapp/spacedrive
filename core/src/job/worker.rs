use super::JobReport;
use crate::api::CoreEvent;
use crate::invalidate_query;
use crate::job::{DynJob, JobError, JobManager, JobReportUpdate, JobStatus};
use crate::library::Library;
use chrono::{DateTime, Utc};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{
	mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
	Mutex,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Type)]
pub struct JobProgressEvent {
	pub id: Uuid,
	pub task_count: i32,
	pub completed_task_count: i32,
	pub message: String,
	pub estimated_completion: DateTime<Utc>,
}

// used to update the worker state from inside the worker thread
#[derive(Debug)]
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Paused(Option<Vec<u8>>),
}

// used to send commands to the worker thread from the manager
#[derive(Debug)]
pub enum WorkerCommand {
	Cancel,
	Shutdown,
}

pub struct WorkerContext {
	pub library: Library,
	events_tx: UnboundedSender<WorkerEvent>,
	pub command_rx: Arc<Mutex<UnboundedReceiver<WorkerCommand>>>,
	pub paused: Arc<AtomicBool>,
}

impl WorkerContext {
	pub fn progress(&self, updates: Vec<JobReportUpdate>) {
		self.events_tx
			.send(WorkerEvent::Progressed(updates))
			.expect("critical error: failed to send worker worker progress event updates");
	}
	pub fn preserve_state(&self, state: Vec<u8>) {
		self.events_tx
			.send(WorkerEvent::Paused(Some(state)))
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
	// external_event_tx: UnboundedSender<JobManagerUpdate>,
	start_time: Option<DateTime<Utc>>,
	paused: Arc<AtomicBool>,
}

impl Worker {
	pub fn new(
		job: Box<dyn DynJob>,
		report: JobReport,
		// external_event_tx: UnboundedSender<JobManagerUpdate>,
	) -> Self {
		let (events_tx, events_rx) = unbounded_channel();

		Self {
			job: Some(job),
			report,
			events_tx,
			events_rx: Some(events_rx),
			command_tx: None,
			// external_event_tx,
			start_time: None,
			paused: Arc::new(AtomicBool::new(false)),
		}
	}

	pub fn pause(&self) {
		self.paused.store(true, Ordering::Relaxed);
	}

	pub fn resume(&self) {
		self.paused.store(false, Ordering::Relaxed);
	}

	pub fn report(&self) -> JobReport {
		self.report.clone()
	}

	pub fn is_paused(&self) -> bool {
		self.paused.load(Ordering::Relaxed)
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

		invalidate_queries(&library);

		// spawn task to handle receiving events from the worker
		tokio::spawn(Worker::track_progress(
			Arc::clone(&worker_mutex),
			events_rx,
			library.clone(),
		));

		let paused = Arc::clone(&worker_mutex.lock().await.paused);

		let worker = Arc::clone(&worker_mutex);

		// spawn task to handle running the job
		tokio::spawn(async move {
			let mut worker_ctx = WorkerContext {
				library: library.clone(),
				events_tx,
				command_rx,
				paused,
			};

			// This oneshot is used to signal job completion, whether successful, failed, or paused,
			// back to the task that's monitoring job execution.
			// let (done_tx, done_rx) = oneshot::channel::<()>();

			// Run the job and handle the result
			match job.run(job_manager.clone(), &mut worker_ctx).await {
				// -> Job completed successfully
				Ok((metadata, errors)) if errors.is_empty() => {
					// worker_ctx
					// 	.events_tx
					// 	.send(WorkerEvent::Completed(done_tx, metadata))
					// 	.expect("critical error: failed to send worker complete event");

					let mut worker = worker.lock().await;

					worker.report.status = JobStatus::Completed;
					worker.report.data = None;
					worker.report.metadata = metadata;
					worker.report.completed_at = Some(Utc::now());
					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					invalidate_queries(&library);
					info!("{}", worker.report);
				}
				// -> Job completed with errors
				Ok((metadata, errors)) => {
					warn!("Job<id'{job_id}'> completed with errors");
					// worker_ctx
					// 	.events_tx
					// 	.send(WorkerEvent::CompletedWithErrors(done_tx, metadata, errors))
					// 	.expect("critical error: failed to send worker complete event");

					let mut worker = worker.lock().await;

					worker.report.status = JobStatus::CompletedWithErrors;
					worker.report.errors_text = errors;
					worker.report.data = None;
					worker.report.metadata = metadata;
					worker.report.completed_at = Some(Utc::now());
					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					invalidate_queries(&library);
					info!("{}", worker.report);
				}
				// -> Job paused
				Err(JobError::Paused(state)) => {
					info!("Job<id='{job_id}'> paused, we will pause all children jobs");
					// if let Err(e) = job.pause_children(&library).await {
					// 	error!("Failed to pause children jobs: {e:#?}");
					// }

					debug!("Setting worker status to paused");

					let mut worker = worker.lock().await;

					worker.report.status = JobStatus::Paused;
					worker.report.data = Some(state);

					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					info!("{}", worker.report);

					invalidate_queries(&library);
				}
				// -> Job failed
				Err(e) => {
					error!("Job<id='{job_id}'> failed with error: {e:#?};");
					// if let Err(e) = job.cancel_children(&library).await {
					// 	error!("Failed to cancel children jobs: {e:#?}");
					// }

					let mut worker = worker.lock().await;

					worker.report.status = JobStatus::Failed;
					worker.report.data = None;
					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					invalidate_queries(&library);
					warn!("{}", worker.report);
				}
			}

			println!("Worker completed job: {:?}", job_hash);

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
		while let Some(event) = events_rx.recv().await {
			let mut worker = worker.lock().await;

			match event {
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

						let report = worker.report.clone();
						// emit a CoreEvent
						library.emit(CoreEvent::JobProgress(JobProgressEvent {
							id: report.id,
							task_count: report.task_count,
							completed_task_count: report.completed_task_count,
							estimated_completion: report.estimated_completion,
							message: report.message,
						}));
					}
				}
				// WorkerEvent::Completed(done_tx, metadata) => {
				// 	worker.report.status = JobStatus::Completed;
				// 	worker.report.data = None;
				// 	worker.report.metadata = metadata;
				// 	worker.report.completed_at = Some(Utc::now());
				// 	if let Err(e) = worker.report.update(&library).await {
				// 		error!("failed to update job report: {:#?}", e);
				// 	}

				// 	invalidate_query!(library, "jobs.reports");

				// 	info!("{}", worker.report);

				// 	done_tx
				// 		.send(())
				// 		.expect("critical error: failed to send worker completion");

				// 	break;
				// }
				// WorkerEvent::CompletedWithErrors(done_tx, metadata, errors) => {
				// 	worker.report.status = JobStatus::CompletedWithErrors;
				// 	worker.report.errors_text = errors;
				// 	worker.report.data = None;
				// 	worker.report.metadata = metadata;
				// 	worker.report.completed_at = Some(Utc::now());
				// 	if let Err(e) = worker.report.update(&library).await {
				// 		error!("failed to update job report: {:#?}", e);
				// 	}

				// 	invalidate_query!(library, "jobs.reports");

				// 	info!("{}", worker.report);

				// 	done_tx
				// 		.send(())
				// 		.expect("critical error: failed to send worker completion");

				// 	break;
				// }
				// WorkerEvent::Failed(done_tx) => {
				// 	worker.report.status = JobStatus::Failed;
				// 	worker.report.data = None;
				// 	if let Err(e) = worker.report.update(&library).await {
				// 		error!("failed to update job report: {:#?}", e);
				// 	}

				// 	invalidate_query!(library, "library.list");
				// 	invalidate_query!(library, "jobs.reports");

				// 	warn!("{}", worker.report);

				// 	done_tx
				// 		.send(())
				// 		.expect("critical error: failed to send worker completion");

				// 	break;
				// }
				WorkerEvent::Paused(state) => {
					debug!("Setting worker status to paused");

					worker.report.status = JobStatus::Paused;
					worker.report.data = state;

					if let Err(e) = worker.report.update(&library).await {
						error!("failed to update job report: {:#?}", e);
					}

					info!("{}", worker.report);

					invalidate_query!(library, "jobs.reports");

					break;
				}
			}
		}
	}
}

fn invalidate_queries(library: &Library) {
	invalidate_query!(library, "jobs.isActive");
	invalidate_query!(library, "jobs.reports");
}
