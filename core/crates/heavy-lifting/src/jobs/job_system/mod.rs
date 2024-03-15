use crate::Error;

use sd_prisma::prisma::PrismaClient;
use sd_task_system::TaskDispatcher;

use std::{
	cell::RefCell,
	collections::hash_map::{Entry, HashMap},
	mem,
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use chrono::Utc;
use futures::{Stream, StreamExt};
use futures_concurrency::{future::TryJoin, stream::Merge};
use tokio::{
	spawn,
	sync::oneshot,
	task::JoinHandle,
	time::{interval_at, Instant},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{error, info, trace, warn};

use self::report::ReportError;

use super::JobId;

pub(crate) mod job;
pub(crate) mod report;

use job::{DynJob, IntoJob, Job, JobHandle, JobOutput, ReturnStatus};

const JOBS_INITIAL_CAPACITY: usize = 32;
const FIVE_MINUTES: Duration = Duration::from_secs(5 * 60);

#[derive(thiserror::Error, Debug)]
pub enum JobSystemError {
	#[error("job not found: <id='{0}'>")]
	NotFound(JobId),
	#[error("job already running: <new_id='{new_id}', name='{job_name}', already_running_id='{already_running_id}'>")]
	AlreadyRunning {
		new_id: JobId,
		job_name: &'static str,
		already_running_id: JobId,
	},

	#[error("job canceled: <id='{0}'>")]
	Canceled(JobId),

	#[error(transparent)]
	Report(#[from] ReportError),

	#[error(transparent)]
	Processing(#[from] Error),
}

impl From<JobSystemError> for rspc::Error {
	fn from(e: JobSystemError) -> Self {
		match e {
			JobSystemError::NotFound(_) => {
				Self::with_cause(rspc::ErrorCode::NotFound, e.to_string(), e)
			}
			JobSystemError::AlreadyRunning { .. } => {
				Self::with_cause(rspc::ErrorCode::Conflict, e.to_string(), e)
			}

			JobSystemError::Canceled(_) => {
				Self::with_cause(rspc::ErrorCode::ClientClosedRequest, e.to_string(), e)
			}
			JobSystemError::Processing(e) => e.into(),
			JobSystemError::Report(e) => e.into(),
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
	Pause,
	Resume,
	Cancel,
}

pub(crate) enum RunnerMessage {
	NewJob {
		id: JobId,
		dyn_job: Box<dyn DynJob>,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	Command {
		id: JobId,
		command: Command,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
}

pub struct JobSystem {
	msgs_tx: chan::Sender<RunnerMessage>,
	job_outputs_rx: chan::Receiver<(JobId, Result<JobOutput, JobSystemError>)>,
	runner_handle: RefCell<Option<JoinHandle<()>>>,
}

impl JobSystem {
	#[must_use]
	pub fn new(dispatcher: TaskDispatcher<Error>, db: Arc<PrismaClient>) -> Self {
		let (job_outputs_tx, job_outputs_rx) = chan::unbounded();
		let (job_return_status_tx, job_return_status_rx) = chan::bounded(16);
		let (msgs_tx, msgs_rx) = chan::bounded(8);

		let runner_handle = RefCell::new(Some(spawn(async move {
			trace!("Job System Runner starting...");
			while let Err(e) = spawn(run(
				JobSystemRunner::new(
					dispatcher.clone(),
					Arc::clone(&db),
					job_return_status_tx.clone(),
					job_outputs_tx.clone(),
				),
				msgs_rx.clone(),
				job_return_status_rx.clone(),
			))
			.await
			{
				if e.is_panic() {
					error!("Job system panicked: {e:#?}");
				} else {
					trace!("JobSystemRunner received shutdown signal and will exit...");
					break;
				}
				trace!("Restarting JobSystemRunner processing task...");
			}

			info!("JobSystemRunner gracefully shutdown");
		})));

		Self {
			msgs_tx,
			job_outputs_rx,
			runner_handle,
		}
	}

	pub async fn shutdown(&self) {
		if let Some(handle) = self
			.runner_handle
			.try_borrow_mut()
			.ok()
			.and_then(|mut maybe_handle| maybe_handle.take())
		{
			if let Err(e) = handle.await {
				if e.is_panic() {
					error!("JobSystem panicked: {e:#?}");
				}
			}
			info!("JobSystem gracefully shutdown");
		} else {
			warn!("JobSystem already shutdown");
		}
	}

	/// Dispatch a new job to the system
	/// # Panics
	/// Panics only happen if internal channels are unexpectedly closed
	pub async fn dispatch<J: Job>(
		&mut self,
		job: impl IntoJob<J> + Send,
	) -> Result<JobId, JobSystemError> {
		let dyn_job = job.into_job();
		let id = dyn_job.id();

		let (ack_tx, ack_rx) = oneshot::channel();
		self.msgs_tx
			.send(RunnerMessage::NewJob {
				id,
				dyn_job,
				ack_tx,
			})
			.await
			.expect("runner msgs channel unexpectedly closed on new job request");

		ack_rx
			.await
			.expect("ack channel closed before receiving new job request")
			.map(|()| id)
	}

	pub fn receive_job_outputs(
		&self,
	) -> impl Stream<Item = (JobId, Result<JobOutput, JobSystemError>)> {
		self.job_outputs_rx.clone()
	}

	async fn send_command(&self, id: JobId, command: Command) -> Result<(), JobSystemError> {
		let (ack_tx, ack_rx) = oneshot::channel();
		self.msgs_tx
			.send(RunnerMessage::Command {
				id,
				command,
				ack_tx,
			})
			.await
			.unwrap_or_else(|_| {
				panic!("runner msgs channel unexpectedly closed on {command:?} request")
			});

		ack_rx
			.await
			.unwrap_or_else(|_| panic!("ack channel closed before receiving {command:?} response"))
	}

	pub async fn pause(&self, id: JobId) -> Result<(), JobSystemError> {
		self.send_command(id, Command::Pause).await
	}

	pub async fn resume(&self, id: JobId) -> Result<(), JobSystemError> {
		self.send_command(id, Command::Resume).await
	}

	pub async fn cancel(&self, id: JobId) -> Result<(), JobSystemError> {
		self.send_command(id, Command::Cancel).await
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl Sync for JobSystem {}

pub(crate) struct JobSystemRunner {
	dispatcher: TaskDispatcher<Error>,
	db: Arc<PrismaClient>,
	handles: HashMap<JobId, JobHandle>,
	job_hashes: HashMap<u64, JobId>,
	job_hashes_by_id: HashMap<JobId, u64>,
	job_return_status_tx: chan::Sender<(JobId, ReturnStatus)>,
	job_outputs_tx: chan::Sender<(JobId, Result<JobOutput, JobSystemError>)>,
}

impl JobSystemRunner {
	pub(crate) fn new(
		dispatcher: TaskDispatcher<Error>,
		db: Arc<PrismaClient>,
		job_return_status_tx: chan::Sender<(JobId, ReturnStatus)>,
		job_outputs_tx: chan::Sender<(JobId, Result<JobOutput, JobSystemError>)>,
	) -> Self {
		Self {
			dispatcher,
			db,
			handles: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			job_hashes: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			job_hashes_by_id: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			job_return_status_tx,
			job_outputs_tx,
		}
	}

	async fn new_job(&mut self, id: JobId, dyn_job: Box<dyn DynJob>) -> Result<(), JobSystemError> {
		let Self {
			dispatcher,
			db,
			handles,
			job_hashes,
			job_hashes_by_id,
			job_return_status_tx,
			..
		} = self;

		let job_hash = dyn_job.hash();
		if let Some(&already_running_id) = job_hashes.get(&job_hash) {
			return Err(JobSystemError::AlreadyRunning {
				new_id: id,
				already_running_id,
				job_name: dyn_job.job_name(),
			});
		}

		job_hashes.insert(job_hash, id);
		job_hashes_by_id.insert(id, job_hash);

		let start_time = Utc::now();

		let mut handle = dyn_job.dispatch(dispatcher.clone(), job_return_status_tx.clone());

		handle.report.status = report::Status::Running;
		if handle.report.started_at.is_none() {
			handle.report.started_at = Some(start_time);
		}

		// If the report doesn't have a created_at date, it's a new report
		if handle.report.created_at.is_none() {
			handle.report.create(db).await?;
		} else {
			// Otherwise it can be a job being resumed or a children job that was already been created
			handle.report.update(db).await?;
		}

		// Registering children jobs
		handle
			.next_jobs
			.iter_mut()
			.map(|dyn_job| dyn_job.report_mut())
			.map(|next_job_report| async {
				if next_job_report.created_at.is_none() {
					next_job_report.create(db).await
				} else {
					Ok(())
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		handles.insert(id, handle);

		Ok(())
	}

	async fn process_command(&mut self, id: JobId, command: Command) -> Result<(), JobSystemError> {
		if let Some(handle) = self.handles.get_mut(&id) {
			handle.send_command(command, &self.db).await?;
			Ok(())
		} else {
			Err(JobSystemError::NotFound(id))
		}
	}

	async fn process_return_status(&mut self, job_id: uuid::Uuid, status: ReturnStatus) {
		let Self {
			db,
			handles,
			job_hashes,
			job_hashes_by_id,
			job_outputs_tx,
			job_return_status_tx,
			dispatcher,
			..
		} = self;

		let job_hash = job_hashes_by_id.remove(&job_id).expect("it must be here");
		assert!(job_hashes.remove(&job_hash).is_some());
		let mut handle = handles.remove(&job_id).expect("it must be here");

		let res = match status {
			ReturnStatus::Completed(output) => {
				if let Some(next) = handle.next_jobs.pop_front() {
					let next_id = next.id();
					let next_hash = next.hash();
					if let Entry::Vacant(e) = job_hashes.entry(next_hash) {
						e.insert(next_id);
						job_hashes_by_id.insert(next_id, next_hash);
						let mut next_handle =
							next.dispatch(dispatcher.clone(), job_return_status_tx.clone());

						assert!(
							next_handle.next_jobs.is_empty(),
							"Only the root job will have next jobs, the rest will be empty and \
							we will swap with remaining ones from the previous job"
						);

						next_handle.next_jobs = mem::take(&mut handle.next_jobs);

						handles.insert(next_id, next_handle);
					} else {
						warn!("Unexpectedly found a job with the same hash as the next job: <id='{next_id}', name='{}'>", next.job_name());
					}
				}

				handle.complete(&output, db).await.map(|()| output)
			}
			ReturnStatus::Failed(e) => {
				// TODO: update report on db

				Err(e.into())
			}
			ReturnStatus::Shutdown(_) => {
				// TODO

				return;
			}
			ReturnStatus::Canceled => {
				// TODO: update report on db

				Err(JobSystemError::Canceled(job_id))
			}
		};

		job_outputs_tx
			.send((job_id, res))
			.await
			.expect("job outputs channel unexpectedly closed on job completion");
	}

	fn clean_memory(&mut self) {
		if self.handles.capacity() > JOBS_INITIAL_CAPACITY
			&& self.handles.len() < JOBS_INITIAL_CAPACITY
		{
			self.handles.shrink_to(JOBS_INITIAL_CAPACITY);
		}

		if self.job_hashes.capacity() > JOBS_INITIAL_CAPACITY
			&& self.job_hashes.len() < JOBS_INITIAL_CAPACITY
		{
			self.job_hashes.shrink_to(JOBS_INITIAL_CAPACITY);
		}

		if self.job_hashes_by_id.capacity() > JOBS_INITIAL_CAPACITY
			&& self.job_hashes_by_id.len() < JOBS_INITIAL_CAPACITY
		{
			self.job_hashes_by_id.shrink_to(JOBS_INITIAL_CAPACITY);
		}
	}
}

async fn run(
	mut runner: JobSystemRunner,
	msgs_rx: chan::Receiver<RunnerMessage>,
	job_return_status_rx: chan::Receiver<(JobId, ReturnStatus)>,
) {
	enum StreamMessage {
		ReturnStatus((JobId, ReturnStatus)),
		RunnerMessage(RunnerMessage),
		CleanMemoryTick,
	}

	let memory_cleanup_interval = interval_at(Instant::now() + FIVE_MINUTES, FIVE_MINUTES);

	let mut msg_stream = pin!((
		msgs_rx.map(StreamMessage::RunnerMessage),
		job_return_status_rx.map(StreamMessage::ReturnStatus),
		IntervalStream::new(memory_cleanup_interval).map(|_| StreamMessage::CleanMemoryTick),
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			// Job return status messages
			StreamMessage::ReturnStatus((job_id, status)) => {
				runner.process_return_status(job_id, status).await;
			}

			// Runner messages
			StreamMessage::RunnerMessage(RunnerMessage::NewJob {
				id,
				dyn_job,
				ack_tx: ack,
			}) => {
				ack.send(runner.new_job(id, dyn_job).await)
					.expect("ack channel closed before sending new job response");
			}

			StreamMessage::RunnerMessage(RunnerMessage::Command {
				id,
				command,
				ack_tx,
			}) => {
				ack_tx
					.send(runner.process_command(id, command).await)
					.unwrap_or_else(|_| {
						panic!("ack channel closed before sending {command:?} response")
					});
			}

			// Memory cleanup tick
			StreamMessage::CleanMemoryTick => {
				runner.clean_memory();
			}
		}
	}
}
