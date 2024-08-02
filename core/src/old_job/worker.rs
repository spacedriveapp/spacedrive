use crate::{api::CoreEvent, invalidate_query, library::Library, Node};

use std::{
	fmt,
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use async_channel as chan;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use futures_concurrency::stream::Merge;
use serde::Serialize;
use serde_json::json;
use specta::Type;
use tokio::{
	spawn,
	sync::{oneshot, watch},
	task::JoinError,
	time::{interval, timeout, Instant, MissedTickBehavior},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

use super::{
	DynJob, JobError, JobIdentity, JobReportUpdate, JobRunErrors, JobRunOutput, JobStatus,
	OldJobReport, OldJobs,
};

const FIVE_SECS: Duration = Duration::from_secs(5);
const FIVE_MINUTES: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Serialize, Type)]
pub struct JobProgressEvent {
	pub id: Uuid,
	pub library_id: Uuid,
	pub task_count: i32,
	pub completed_task_count: i32,
	pub phase: String,
	pub message: String,
	pub info: String,
	pub estimated_completion: DateTime<Utc>,
}

/// used to update the worker state from inside the worker thread
#[derive(Debug)]
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Paused,
	Stop,
}

/// used to send commands to the worker thread from the manager
#[derive(Debug)]
pub enum WorkerCommand {
	Pause(Instant),
	Resume(Instant),
	IdentifyYourself(oneshot::Sender<JobIdentity>),
	Cancel(Instant, oneshot::Sender<()>),
	Shutdown(Instant, oneshot::Sender<()>),
	Timeout(Duration, oneshot::Sender<()>),
}

pub struct WorkerContext {
	pub library: Arc<Library>,
	pub node: Arc<Node>,
	pub(super) events_tx: chan::Sender<WorkerEvent>,
}

impl fmt::Debug for WorkerContext {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("WorkerContext").finish()
	}
}

impl Drop for WorkerContext {
	fn drop(&mut self) {
		// This send blocking is fine as this sender is unbounded
		if !self.events_tx.is_closed() && self.events_tx.send_blocking(WorkerEvent::Stop).is_err() {
			error!("Error sending worker context stop event");
		}
	}
}
impl WorkerContext {
	pub fn pause(&self) {
		if self.events_tx.send_blocking(WorkerEvent::Paused).is_err() {
			error!("Error sending worker context pause event");
		}
	}

	pub fn progress_msg(&self, msg: String) {
		self.progress(vec![JobReportUpdate::Message(msg)]);
	}

	pub fn progress(&self, updates: Vec<JobReportUpdate>) {
		if !self.events_tx.is_closed()
			&& self
				.events_tx
				// This send blocking is fine as this sender is unbounded
				.send_blocking(WorkerEvent::Progressed(updates))
				.is_err()
		{
			error!("Error sending worker context progress event");
		}
	}
}

// a worker is a dedicated task that runs a single job
// once the job is complete the worker will exit
pub struct Worker {
	pub(super) library_id: Uuid,
	commands_tx: chan::Sender<WorkerCommand>,
	report_watch_tx: Arc<watch::Sender<OldJobReport>>,
	report_watch_rx: watch::Receiver<OldJobReport>,
	paused: AtomicBool,
}

impl Worker {
	pub async fn new(
		id: Uuid,
		mut job: Box<dyn DynJob>,
		mut report: OldJobReport,
		library: Arc<Library>,
		node: Arc<Node>,
		job_manager: Arc<OldJobs>,
	) -> Result<Self, JobError> {
		let (commands_tx, commands_rx) = chan::bounded(8);

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

		let (report_watch_tx, report_watch_rx) = watch::channel(report.clone());
		let report_watch_tx = Arc::new(report_watch_tx);
		let library_id = library.id;

		// spawn task to handle running the job
		spawn(Self::do_work(
			id,
			JobWorkTable {
				job,
				manager: job_manager,
				hash: job_hash,
				report,
			},
			Arc::clone(&report_watch_tx),
			start_time,
			(commands_tx.clone(), commands_rx),
			library,
			node,
		));

		Ok(Self {
			library_id,
			commands_tx,
			report_watch_tx,
			report_watch_rx,
			paused: AtomicBool::new(false),
		})
	}

	pub async fn pause(&self) {
		if self.report_watch_rx.borrow().status == JobStatus::Running {
			self.paused.store(true, Ordering::Relaxed);
			if self
				.commands_tx
				.send(WorkerCommand::Pause(Instant::now()))
				.await
				.is_ok()
			{
				self.report_watch_tx
					.send_modify(|report| report.status = JobStatus::Paused);
			}
		}
	}

	pub async fn who_am_i(&self) -> Option<JobIdentity> {
		let (tx, rx) = oneshot::channel();
		if self
			.commands_tx
			.send(WorkerCommand::IdentifyYourself(tx))
			.await
			.is_err()
		{
			warn!("Failed to send identify yourself command to a job worker");
			return None;
		}

		rx.await
			.map_err(|_| warn!("Failed to receive identify yourself answer from a job worker"))
			.ok()
	}

	pub async fn resume(&self) {
		if self.report_watch_rx.borrow().status == JobStatus::Paused {
			self.paused.store(false, Ordering::Relaxed);
			if self
				.commands_tx
				.send(WorkerCommand::Resume(Instant::now()))
				.await
				.is_ok()
			{
				self.report_watch_tx
					.send_modify(|report| report.status = JobStatus::Running);
			}
		}
	}

	pub async fn cancel(&self) {
		if self.report_watch_rx.borrow().status != JobStatus::Canceled {
			let (tx, rx) = oneshot::channel();
			if self
				.commands_tx
				.send(WorkerCommand::Cancel(Instant::now(), tx))
				.await
				.is_ok()
			{
				self.report_watch_tx
					.send_modify(|report| report.status = JobStatus::Canceled);
				rx.await.ok();
			}
		}
	}

	pub async fn shutdown(&self) {
		let (tx, rx) = oneshot::channel();
		if self
			.commands_tx
			.send(WorkerCommand::Shutdown(Instant::now(), tx))
			.await
			.is_ok()
		{
			rx.await.ok();
		}
	}

	pub fn report(&self) -> OldJobReport {
		self.report_watch_rx.borrow().clone()
	}

	pub fn is_paused(&self) -> bool {
		self.paused.load(Ordering::Relaxed)
	}

	#[instrument(skip_all, fields(job_id = %report.id, job_name = %report.name))]
	fn track_progress(
		report: &mut OldJobReport,
		last_report_watch_update: &mut Instant,
		report_watch_tx: &watch::Sender<OldJobReport>,
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
					trace!(?message);
					report.message = message;
				}
				JobReportUpdate::Phase(phase) => {
					trace!(
						old_phase = %report.phase,
						new_phase = %phase,
						"Changing Job phase;",
					);
					report.phase = phase;
				}
				JobReportUpdate::Info(info) => {
					report.info = info;
				}
			}
		}

		// Calculate elapsed time
		let elapsed = Utc::now() - start_time;

		// Calculate remaining time
		let task_count = report.task_count as usize;
		let completed_task_count = report.completed_task_count as usize;
		let remaining_task_count = task_count.saturating_sub(completed_task_count);

		// To avoid division by zero
		let remaining_time_per_task = elapsed / (usize::max(completed_task_count, 1) as i32);
		let remaining_time = remaining_time_per_task * remaining_task_count as i32;

		// Update the report with estimated remaining time
		report.estimated_completion = Utc::now()
			.checked_add_signed(remaining_time)
			.unwrap_or(Utc::now());

		// updated the report watcher
		if last_report_watch_update.elapsed() > Duration::from_millis(500) {
			report_watch_tx.send_modify(|old| {
				old.task_count = report.task_count;
				old.completed_task_count = report.completed_task_count;
				old.estimated_completion = report.estimated_completion;
				old.message.clone_from(&report.message);
			});
			*last_report_watch_update = Instant::now();
		}

		// emit a CoreEvent
		library.emit(CoreEvent::JobProgress(JobProgressEvent {
			id: report.id,
			library_id: library.id,
			task_count: report.task_count,
			completed_task_count: report.completed_task_count,
			estimated_completion: report.estimated_completion,
			info: report.info.clone(),
			phase: report.phase.clone(),
			message: report.message.clone(),
		}));
	}

	#[instrument(
		skip_all,
		fields(
			job_id = %report.id,
			job_name = %report.name,
			worker_id = %worker_id,
			%start_time,
			library_id = %library.id,
		),
	)]
	async fn do_work(
		worker_id: Uuid,
		JobWorkTable {
			mut job,
			manager,
			hash,
			mut report,
		}: JobWorkTable,
		report_watch_tx: Arc<watch::Sender<OldJobReport>>,
		start_time: DateTime<Utc>,
		(commands_tx, commands_rx): (chan::Sender<WorkerCommand>, chan::Receiver<WorkerCommand>),
		library: Arc<Library>,
		node: Arc<Node>,
	) {
		let (events_tx, events_rx) = chan::unbounded();

		let mut timeout_checker = interval(FIVE_SECS);
		timeout_checker.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut last_update_received_at = Instant::now();

		let mut last_reporter_watch_update = Instant::now();
		invalidate_query!(library, "jobs.reports");

		let mut finalized_events_rx = pin!(events_rx.clone());

		let mut is_paused = false;

		let mut run_task = {
			let library = Arc::clone(&library);
			spawn(async move {
				let job_result = job
					.run(
						WorkerContext {
							library,
							node,
							events_tx,
						},
						commands_rx,
					)
					.await;

				(job, job_result)
			})
		};

		type RunOutput = (Box<dyn DynJob>, Result<JobRunOutput, JobError>);

		enum StreamMessage {
			JobResult(Result<RunOutput, JoinError>),
			NewEvent(WorkerEvent),
			Tick,
		}

		let mut msg_stream = pin!((
			stream::once(&mut run_task).map(StreamMessage::JobResult),
			events_rx.map(StreamMessage::NewEvent),
			IntervalStream::new(timeout_checker).map(|_| StreamMessage::Tick),
		)
			.merge());

		let mut events_ended = false;

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::JobResult(Err(join_error)) => {
					error!(?join_error, "Critical error;");
					break;
				}
				StreamMessage::JobResult(Ok((job, job_result))) => {
					if !events_ended {
						finalized_events_rx.close();
						// There are still some progress events to be processed so we postpone the job result
						while let Some(WorkerEvent::Progressed(updates)) =
							finalized_events_rx.next().await
						{
							Self::track_progress(
								&mut report,
								&mut last_reporter_watch_update,
								&report_watch_tx,
								start_time,
								updates,
								&library,
							);
						}
					}

					let next_job =
						Self::process_job_output(job, job_result, &mut report, &library).await;

					report_watch_tx.send(report.clone()).ok();

					debug!("Worker completed Job");

					return manager.complete(&library, worker_id, hash, next_job).await;
				}
				StreamMessage::NewEvent(WorkerEvent::Progressed(updates)) => {
					is_paused = false;
					last_update_received_at = Instant::now();
					Self::track_progress(
						&mut report,
						&mut last_reporter_watch_update,
						&report_watch_tx,
						start_time,
						updates,
						&library,
					);
				}
				StreamMessage::NewEvent(WorkerEvent::Paused) => {
					is_paused = true;
				}
				StreamMessage::NewEvent(WorkerEvent::Stop) => {
					events_ended = true;
				}
				StreamMessage::Tick => {
					if !is_paused {
						let elapsed = last_update_received_at.elapsed();
						if elapsed > FIVE_MINUTES {
							error!(
								time_without_updates = ?elapsed,
								"Worker has not received any updates;",
							);

							let (tx, rx) = oneshot::channel();
							if commands_tx
								.send(WorkerCommand::Timeout(elapsed, tx))
								.await
								.is_err()
							{
								error!("Failed to send timeout step command to a running job");
							} else if timeout(FIVE_SECS, rx).await.is_err() {
								error!("Failed to receive timeout step answer from a running job");
							}

							// As we already sent a timeout command, we can safely join as the job is over
							let Ok((job, job_result)) = run_task
								.await
								.map_err(|join_error| error!(?join_error, "Had a critical error;"))
							else {
								break;
							};

							Self::process_job_output(job, job_result, &mut report, &library).await;

							report_watch_tx.send(report.clone()).ok();

							error!("Timed out");

							break;
						}
					}
				}
			}
		}

		manager.complete(&library, worker_id, hash, None).await
	}

	async fn process_job_output(
		mut job: Box<dyn DynJob>,
		job_result: Result<JobRunOutput, JobError>,
		report: &mut OldJobReport,
		library: &Library,
	) -> Option<Box<dyn DynJob>> {
		// Run the job and handle the result
		match job_result {
			// -> Job completed successfully
			Ok(JobRunOutput {
				metadata,
				errors: JobRunErrors(errors),
				next_job,
			}) if errors.is_empty() => {
				report.status = JobStatus::Completed;
				report.data = None;
				report.metadata = match (report.metadata.take(), metadata) {
					(Some(mut current_metadata), Some(new_metadata)) => {
						current_metadata["output"] = new_metadata;
						Some(current_metadata)
					}
					(None, Some(new_metadata)) => Some(json!({ "output": new_metadata })),
					(Some(current_metadata), None) => Some(current_metadata),
					_ => None,
				};
				report.completed_at = Some(Utc::now());
				if let Err(e) = report.update(library).await {
					error!(?e, "Failed to update job report;");
				}

				debug!(?report);

				invalidate_queries(library);

				return next_job;
			}
			// -> Job completed with errors
			Ok(JobRunOutput {
				metadata,
				errors: JobRunErrors(errors),
				next_job,
			}) => {
				warn!("Completed with errors");
				report.status = JobStatus::CompletedWithErrors;
				report.errors_text = errors;
				report.data = None;
				report.metadata = match (report.metadata.take(), metadata) {
					(Some(mut current_metadata), Some(new_metadata)) => {
						current_metadata["output"] = new_metadata;
						Some(current_metadata)
					}
					(None, Some(new_metadata)) => Some(json!({ "output": new_metadata })),
					(Some(current_metadata), None) => Some(current_metadata),
					_ => None,
				};
				report.completed_at = Some(Utc::now());
				if let Err(e) = report.update(library).await {
					error!(?e, "Failed to update job report;");
				}

				debug!(?report);

				invalidate_queries(library);

				return next_job;
			}
			// -> Job paused
			Err(JobError::Paused(state, signal_tx)) => {
				info!("Job paused, we will pause all children jobs");
				if let Err(e) = job.pause_children(library).await {
					error!(?e, "Failed to pause children jobs;");
				}

				debug!("Setting worker status to paused");

				report.status = JobStatus::Paused;
				report.data = Some(state);

				if let Err(e) = report.update(library).await {
					error!(?e, "Failed to update job report;");
				}

				debug!(?report);

				invalidate_queries(library);

				signal_tx.send(()).ok();
			}
			// -> Job canceled
			Err(JobError::Canceled(signal_tx)) => {
				info!("Job canceled, we will cancel all children jobs");
				if let Err(e) = job.cancel_children(library).await {
					error!(?e, "Failed to pause children jobs;");
				}

				debug!("Setting worker status to paused");

				report.status = JobStatus::Canceled;
				report.data = None;

				if let Err(e) = report.update(library).await {
					error!(?e, "Failed to update job report;");
				}

				debug!(?report);

				invalidate_queries(library);

				signal_tx.send(()).ok();
			}
			// -> Job failed
			Err(e) => {
				error!(?e, "Job failed with error;");
				if let Err(e) = job.cancel_children(library).await {
					error!(?e, "Failed to cancel children jobs;");
				}

				report.status = JobStatus::Failed;
				report.data = None;
				if let Err(e) = report.update(library).await {
					error!(?e, "Failed to update job report;");
				}

				warn!(?report);

				invalidate_queries(library);
			}
		}

		None
	}
}

struct JobWorkTable {
	job: Box<dyn DynJob>,
	manager: Arc<OldJobs>,
	hash: u64,
	report: OldJobReport,
}

fn invalidate_queries(library: &Library) {
	invalidate_query!(library, "jobs.isActive");
	invalidate_query!(library, "jobs.reports");
}
