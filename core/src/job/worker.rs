use crate::{api::CoreEvent, invalidate_query, library::Library};

use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use specta::Type;
use tokio::{
	select,
	sync::{mpsc, oneshot},
};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::{DynJob, JobError, JobManager, JobReport, JobReportUpdate, JobStatus};

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
	Stop,
}

// used to send commands to the worker thread from the manager
#[derive(Debug)]
pub enum WorkerCommand {
	Pause,
	Resume,
	Cancel(oneshot::Sender<()>),
	Shutdown(oneshot::Sender<()>),
}

pub struct WorkerContext {
	pub library: Library,
	events_tx: mpsc::UnboundedSender<WorkerEvent>,
}

impl Drop for WorkerContext {
	fn drop(&mut self) {
		self.events_tx
			.send(WorkerEvent::Stop)
			.expect("critical error: failed to send worker stop event");
	}
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
	commands_tx: mpsc::Sender<WorkerCommand>,
	request_report_tx: mpsc::Sender<oneshot::Sender<JobReport>>,
	paused: AtomicBool,
}

impl Worker {
	pub async fn new(
		mut job: Box<dyn DynJob>,
		mut report: JobReport,
		library: Library,
		job_manager: Arc<JobManager>,
	) -> Result<Self, JobError> {
		let (commands_tx, commands_rx) = mpsc::channel(8);
		let (request_report_tx, request_report_rx) = mpsc::channel(8);

		let job_hash = job.hash();

		let start_time = Utc::now();

		report.status = JobStatus::Running;
		if report.started_at.is_none() {
			report.started_at = Some(start_time);
		}

		// If the report doesn't have a created_at date, it's a new report
		if report.created_at.is_none() {
			report.create(&library).await?;
		} else {
			// Otherwise it can be a job being resumed or a children job that was already been created
			report.update(&library).await?;
		}

		job.register_children(&library).await?;

		invalidate_queries(&library);

		// spawn task to handle running the job
		tokio::spawn(Self::do_work(
			JobWorkTable {
				job,
				manager: job_manager,
				hash: job_hash,
				report,
			},
			request_report_rx,
			start_time,
			commands_rx,
			library,
		));

		Ok(Self {
			commands_tx,
			request_report_tx,
			paused: AtomicBool::new(false),
		})
	}

	pub async fn pause(&self) {
		self.paused.store(true, Ordering::Relaxed);
		self.commands_tx.send(WorkerCommand::Pause).await.ok();
	}

	pub async fn resume(&self) {
		self.paused.store(false, Ordering::Relaxed);
		self.commands_tx.send(WorkerCommand::Resume).await.ok();
	}

	pub async fn cancel(&self) {
		let (tx, rx) = oneshot::channel();
		self.commands_tx.send(WorkerCommand::Cancel(tx)).await.ok();
		rx.await.ok();
	}

	pub async fn shutdown(&self) {
		let (tx, rx) = oneshot::channel();
		self.commands_tx
			.send(WorkerCommand::Shutdown(tx))
			.await
			.ok();
		rx.await.ok();
	}

	pub async fn report(&self) -> JobReport {
		let (tx, rx) = oneshot::channel();

		self.request_report_tx
			.send(tx)
			.await
			.expect("critical error: failed to send worker request for job report");

		rx.await
			.expect("critical error: failed to receive worker response for job report")
	}

	pub fn is_paused(&self) -> bool {
		self.paused.load(Ordering::Relaxed)
	}

	fn track_progress(
		report: &mut JobReport,
		start_time: DateTime<Utc>,
		updates: Vec<JobReportUpdate>,
		library: &Library,
	) {
		// protect against updates if job is not running
		if report.status != JobStatus::Running {
			return;
		};

		for update in updates {
			match update {
				JobReportUpdate::TaskCount(task_count) => {
					report.task_count = task_count as i32;
				}
				JobReportUpdate::CompletedTaskCount(completed_task_count) => {
					report.completed_task_count = completed_task_count as i32;
				}

				JobReportUpdate::Message(message) => {
					trace!("job {} message: {}", report.id, message);
					report.message = message;
				}
			}
		}

		// Calculate elapsed time

		let elapsed = Utc::now() - start_time;

		// Calculate remaining time
		let task_count = report.task_count as usize;
		let completed_task_count = report.completed_task_count as usize;
		let remaining_task_count = task_count.saturating_sub(completed_task_count);
		let remaining_time_per_task = elapsed / (completed_task_count + 1) as i32; // Adding 1 to avoid division by zero
		let remaining_time = remaining_time_per_task * remaining_task_count as i32;

		// Update the report with estimated remaining time
		report.estimated_completion = Utc::now()
			.checked_add_signed(remaining_time)
			.unwrap_or(Utc::now());

		// emit a CoreEvent
		library.emit(CoreEvent::JobProgress(JobProgressEvent {
			id: report.id,
			task_count: report.task_count,
			completed_task_count: report.completed_task_count,
			estimated_completion: report.estimated_completion,
			message: report.message.clone(),
		}));
	}

	async fn do_work(
		JobWorkTable {
			mut job,
			manager,
			hash,
			mut report,
		}: JobWorkTable,
		mut request_report_rx: mpsc::Receiver<oneshot::Sender<JobReport>>,
		start_time: DateTime<Utc>,
		commands_rx: mpsc::Receiver<WorkerCommand>,
		library: Library,
	) {
		let (events_tx, mut events_rx) = mpsc::unbounded_channel();

		let mut job_future = job.run(
			Arc::clone(&manager),
			WorkerContext {
				library: library.clone(),
				events_tx,
			},
			commands_rx,
		);

		let mut events_ended = false;
		let job_result = 'job: loop {
			select! {
				job_result = &mut job_future => {
					if !events_ended {
						// There are still some progress events to be processed so we postpone the job result
						loop {
							select!{
								Some(event) = events_rx.recv() => {
									match event {
										WorkerEvent::Progressed(updates) => {
											Self::track_progress(
												&mut report,
												start_time,
												updates,
												&library
											);
										}
										WorkerEvent::Stop => {
											break 'job job_result;
										},
									}
								},
								// When someone requests a report, send a copy of the current report
								Some(response_tx) = request_report_rx.recv() => {
									response_tx.send(report.clone()).ok();
								},
							}
						}
					} else {
						break 'job job_result;
					}
				},

				// When someone requests a report, send a copy of the current report
				Some(response_tx) = request_report_rx.recv() => {
					response_tx.send(report.clone()).ok();
				},

				Some(event) = events_rx.recv() => {
					match event {
						WorkerEvent::Progressed(updates) => {
							Self::track_progress(&mut report, start_time, updates, &library)
						}
						WorkerEvent::Stop => {events_ended = true;},
					}
				}
			}
		};

		// Need this drop here to sinalize to borrowchecker that we're done with our `&mut job` borrow for `run` method
		drop(job_future);

		Self::process_job_output(job, job_result, &mut report, &library).await;

		debug!(
			"Worker completed Job<id='{}', name='{}'>",
			report.id, report.name
		);

		manager.complete(&library, report.id, hash).await;
	}

	async fn process_job_output(
		mut job: Box<dyn DynJob>,
		job_result: Result<(Option<Value>, Vec<String>), JobError>,
		report: &mut JobReport,
		library: &Library,
	) {
		// Run the job and handle the result
		match job_result {
			// -> Job completed successfully
			Ok((metadata, errors)) if errors.is_empty() => {
				report.status = JobStatus::Completed;
				report.data = None;
				report.metadata = metadata;
				report.completed_at = Some(Utc::now());
				if let Err(e) = report.update(library).await {
					error!("failed to update job report: {:#?}", e);
				}

				debug!("{report}");

				invalidate_queries(library);
			}
			// -> Job completed with errors
			Ok((metadata, errors)) => {
				warn!(
					"Job<id='{}', name='{}'> completed with errors",
					report.id, report.name
				);
				report.status = JobStatus::CompletedWithErrors;
				report.errors_text = errors;
				report.data = None;
				report.metadata = metadata;
				report.completed_at = Some(Utc::now());
				if let Err(e) = report.update(library).await {
					error!("failed to update job report: {:#?}", e);
				}

				debug!("{report}");

				invalidate_queries(library);
			}
			// -> Job paused
			Err(JobError::Paused(state, signal_tx)) => {
				info!(
					"Job<id='{}', name='{}'> paused, we will pause all children jobs",
					report.id, report.name
				);
				if let Err(e) = job.pause_children(library).await {
					error!("Failed to pause children jobs: {e:#?}");
				}

				debug!("Setting worker status to paused");

				report.status = JobStatus::Paused;
				report.data = Some(state);

				if let Err(e) = report.update(library).await {
					error!("failed to update job report: {:#?}", e);
				}

				debug!("{report}");

				invalidate_queries(library);

				signal_tx.send(()).ok();
			}
			// -> Job paused
			Err(JobError::Canceled(signal_tx)) => {
				info!(
					"Job<id='{}', name='{}'> canceled, we will cancel all children jobs",
					report.id, report.name
				);
				if let Err(e) = job.cancel_children(library).await {
					error!("Failed to pause children jobs: {e:#?}");
				}

				debug!("Setting worker status to paused");

				report.status = JobStatus::Canceled;
				report.data = None;

				if let Err(e) = report.update(library).await {
					error!("failed to update job report: {:#?}", e);
				}

				debug!("{report}");

				invalidate_queries(library);

				signal_tx.send(()).ok();
			}
			// -> Job failed
			Err(e) => {
				error!(
					"Job<id='{}', name='{}'> failed with error: {e:#?};",
					report.id, report.name
				);
				if let Err(e) = job.cancel_children(library).await {
					error!("Failed to cancel children jobs: {e:#?}");
				}

				report.status = JobStatus::Failed;
				report.data = None;
				if let Err(e) = report.update(library).await {
					error!("failed to update job report: {:#?}", e);
				}

				warn!("{report}");

				invalidate_queries(library);
			}
		}
	}
}

struct JobWorkTable {
	job: Box<dyn DynJob>,
	manager: Arc<JobManager>,
	hash: u64,
	report: JobReport,
}

fn invalidate_queries(library: &Library) {
	invalidate_query!(library, "jobs.isActive");
	invalidate_query!(library, "jobs.reports");
}
