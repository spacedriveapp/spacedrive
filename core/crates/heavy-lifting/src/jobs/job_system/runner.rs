use crate::{jobs::JobId, Error};

use sd_prisma::prisma::PrismaClient;
use sd_task_system::{Task, TaskDispatcher};
use sd_utils::error::FileIOError;

use std::{
	collections::{hash_map::Entry, HashMap},
	mem,
	path::Path,
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use chrono::Utc;
use futures::StreamExt;
use futures_concurrency::{future::TryJoin, stream::Merge};
use tokio::{
	fs,
	sync::oneshot,
	time::{interval_at, Instant},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{
	job::{DynJob, JobHandle, JobOutput, ReturnStatus},
	report,
	store::{StoredJob, StoredJobEntry},
	Command, JobSystemError,
};

const JOBS_INITIAL_CAPACITY: usize = 32;
const FIVE_MINUTES: Duration = Duration::from_secs(5 * 60);

pub(super) enum RunnerMessage {
	NewJob {
		id: JobId,
		dyn_job: Box<dyn DynJob>,
		db_id: Uuid,
		db: Arc<PrismaClient>,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	ResumeStoredJob {
		id: JobId,
		dyn_job: Box<dyn DynJob>,
		dyn_tasks: Vec<Box<dyn Task<Error>>>,
		db_id: Uuid,
		db: Arc<PrismaClient>,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	Command {
		id: JobId,
		command: Command,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	Shutdown,
}

pub(super) struct JobSystemRunner {
	dispatcher: TaskDispatcher<Error>,
	handles: HashMap<JobId, JobHandle>,
	job_hashes: HashMap<u64, JobId>,
	job_hashes_by_id: HashMap<JobId, u64>,
	dbs_by_job_id: HashMap<JobId, (Uuid, Arc<PrismaClient>)>,
	jobs_to_store_by_db_id: HashMap<Uuid, Vec<StoredJobEntry>>,
	job_return_status_tx: chan::Sender<(JobId, ReturnStatus)>,
	job_outputs_tx: chan::Sender<(JobId, Result<JobOutput, JobSystemError>)>,
}

impl JobSystemRunner {
	pub(super) fn new(
		dispatcher: TaskDispatcher<Error>,
		job_return_status_tx: chan::Sender<(JobId, ReturnStatus)>,
		job_outputs_tx: chan::Sender<(JobId, Result<JobOutput, JobSystemError>)>,
	) -> Self {
		Self {
			dispatcher,
			handles: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			job_hashes: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			job_hashes_by_id: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			dbs_by_job_id: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			jobs_to_store_by_db_id: HashMap::new(),
			job_return_status_tx,
			job_outputs_tx,
		}
	}

	async fn new_job(
		&mut self,
		id: JobId,
		dyn_job: Box<dyn DynJob>,
		maybe_existing_tasks: Option<Vec<Box<dyn Task<Error>>>>,
		(db_id, db): (Uuid, Arc<PrismaClient>),
	) -> Result<(), JobSystemError> {
		let Self {
			dispatcher,
			handles,
			job_hashes,
			job_hashes_by_id,
			dbs_by_job_id,
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

		let mut handle = if let Some(existing_tasks) = maybe_existing_tasks {
			dyn_job.resume(
				dispatcher.clone(),
				existing_tasks,
				job_return_status_tx.clone(),
			)
		} else {
			dyn_job.dispatch(dispatcher.clone(), job_return_status_tx.clone())
		};

		handle.report.status = report::Status::Running;
		if handle.report.started_at.is_none() {
			handle.report.started_at = Some(start_time);
		}

		// If the report doesn't have a created_at date, it's a new report
		if handle.report.created_at.is_none() {
			handle.report.create(&db).await?;
		} else {
			// Otherwise it can be a job being resumed or a children job that was already been created
			handle.report.update(&db).await?;
		}

		// Registering children jobs
		handle
			.next_jobs
			.iter_mut()
			.map(|dyn_job| dyn_job.report_mut())
			.map(|next_job_report| async {
				if next_job_report.created_at.is_none() {
					next_job_report.create(&db).await
				} else {
					Ok(())
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		handles.insert(id, handle);
		dbs_by_job_id.insert(id, (db_id, db));

		Ok(())
	}

	async fn process_command(&mut self, id: JobId, command: Command) -> Result<(), JobSystemError> {
		if let (Some(handle), Some((_, db))) =
			(self.handles.get_mut(&id), self.dbs_by_job_id.get(&id))
		{
			handle.send_command(command, db).await?;
			Ok(())
		} else {
			Err(JobSystemError::NotFound(id))
		}
	}

	fn is_empty(&self) -> bool {
		self.handles.is_empty()
			&& self.job_hashes.is_empty()
			&& self.job_hashes_by_id.is_empty()
			&& self.dbs_by_job_id.is_empty()
	}

	async fn process_return_status(&mut self, job_id: JobId, status: ReturnStatus) {
		let Self {
			handles,
			job_hashes,
			job_hashes_by_id,
			dbs_by_job_id,
			job_outputs_tx,
			job_return_status_tx,
			dispatcher,
			..
		} = self;

		let job_hash = job_hashes_by_id.remove(&job_id).expect("it must be here");
		let (db_id, db) = dbs_by_job_id.remove(&job_id).expect("it must be here");
		assert!(job_hashes.remove(&job_hash).is_some());
		let mut handle = handles.remove(&job_id).expect("it must be here");

		let res = match status {
			ReturnStatus::Completed(job_return) => {
				try_dispatch_next_job(
					&mut handle,
					dispatcher.clone(),
					(job_hashes, job_hashes_by_id),
					handles,
					job_return_status_tx.clone(),
					(db_id, &db, dbs_by_job_id),
				);

				handle.complete_job(job_return, &db).await
			}

			ReturnStatus::Failed(e) => handle
				.failed_job(&e, &db)
				.await
				.and_then(|()| Err(e.into())),

			ReturnStatus::Shutdown(Some(res)) => {
				let Ok(serialized_job) = res.map_err(|e| error!("Failed to serialize job: {e:#?}"))
				else {
					return;
				};

				let name = handle.report.name;

				let Ok(next_jobs) = handle
					.next_jobs
					.into_iter()
					.filter_map(|next_job| {
						let next_id = next_job.id();
						let next_name = next_job.job_name();
						next_job.serialize().map(|res| {
							res.map(|serialized_job| StoredJob {
								id: next_job.id(),
								name: next_job.job_name(),
								serialized_job,
							})
							.map_err(|e| {
								error!(
									"Failed to serialize next job: \
									<parent_id='{job_id}', parent_name='{name}', \
									next_id='{next_id}', next_name='{next_name}'>: {e:#?}"
								);
							})
						})
					})
					.collect::<Result<Vec<_>, _>>()
				else {
					return;
				};

				self.jobs_to_store_by_db_id
					.entry(db_id)
					.or_default()
					.push(StoredJobEntry {
						root_job: StoredJob {
							id: job_id,
							name,
							serialized_job,
						},
						next_jobs,
					});

				return;
			}

			ReturnStatus::Shutdown(None) => {
				debug!(
					"Job was shutdown but didn't returned any serialized data, \
					probably it isn't resumable job: <id='{job_id}'>"
				);

				return;
			}

			ReturnStatus::Canceled => handle
				.cancel_job(&db)
				.await
				.and_then(|()| Err(JobSystemError::Canceled(job_id))),
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

		if self.dbs_by_job_id.capacity() > JOBS_INITIAL_CAPACITY
			&& self.dbs_by_job_id.len() < JOBS_INITIAL_CAPACITY
		{
			self.dbs_by_job_id.shrink_to(JOBS_INITIAL_CAPACITY);
		}
	}

	async fn save_jobs(
		self,
		store_jobs_file: impl AsRef<Path> + Send,
	) -> Result<(), JobSystemError> {
		let store_jobs_file = store_jobs_file.as_ref();

		let Self {
			handles,
			job_hashes,
			job_hashes_by_id,
			dbs_by_job_id,
			jobs_to_store_by_db_id,
			..
		} = self;

		assert!(
			handles.is_empty()
				&& job_hashes.is_empty()
				&& job_hashes_by_id.is_empty()
				&& dbs_by_job_id.is_empty(),
			"All jobs must be completed before saving"
		);

		if jobs_to_store_by_db_id.is_empty() {
			info!("No jobs to store in disk for job system shutdown!");
			return Ok(());
		}

		fs::write(
			store_jobs_file,
			rmp_serde::to_vec_named(&jobs_to_store_by_db_id)?,
		)
		.await
		.map_err(|e| JobSystemError::StoredJobs(FileIOError::from((store_jobs_file, e))))
	}
}

type DbData<'db, 'dbs_by_job_id> = (
	Uuid,
	&'db Arc<PrismaClient>,
	&'dbs_by_job_id mut HashMap<JobId, (Uuid, Arc<PrismaClient>)>,
);

fn try_dispatch_next_job(
	handle: &mut JobHandle,
	dispatcher: TaskDispatcher<Error>,
	(job_hashes, job_hashes_by_id): (&mut HashMap<u64, JobId>, &mut HashMap<JobId, u64>),
	handles: &mut HashMap<JobId, JobHandle>,
	job_return_status_tx: chan::Sender<(JobId, ReturnStatus)>,
	(db_id, db, dbs_by_job_id): DbData<'_, '_>,
) {
	if let Some(next) = handle.next_jobs.pop_front() {
		let next_id = next.id();
		let next_hash = next.hash();
		if let Entry::Vacant(e) = job_hashes.entry(next_hash) {
			e.insert(next_id);
			job_hashes_by_id.insert(next_id, next_hash);
			let mut next_handle = next.dispatch(dispatcher, job_return_status_tx);

			assert!(
				next_handle.next_jobs.is_empty(),
				"Only the root job will have next jobs, the rest will be empty and \
							we will swap with remaining ones from the previous job"
			);

			next_handle.next_jobs = mem::take(&mut handle.next_jobs);

			handles.insert(next_id, next_handle);
			dbs_by_job_id.insert(next_id, (db_id, Arc::clone(db)));
		} else {
			warn!("Unexpectedly found a job with the same hash as the next job: <id='{next_id}', name='{}'>", next.job_name());
		}
	}
}

pub(super) async fn run(
	mut runner: JobSystemRunner,
	store_jobs_file: impl AsRef<Path> + Send,
	msgs_rx: chan::Receiver<RunnerMessage>,
	job_return_status_rx: chan::Receiver<(JobId, ReturnStatus)>,
) {
	enum StreamMessage {
		ReturnStatus((JobId, ReturnStatus)),
		RunnerMessage(RunnerMessage),
		CleanMemoryTick,
	}

	let memory_cleanup_interval = interval_at(Instant::now() + FIVE_MINUTES, FIVE_MINUTES);

	let job_return_status_rx_to_shutdown = job_return_status_rx.clone();

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
				db_id,
				db,
				ack_tx,
			}) => {
				ack_tx
					.send(runner.new_job(id, dyn_job, None, (db_id, db)).await)
					.expect("ack channel closed before sending new job response");
			}

			StreamMessage::RunnerMessage(RunnerMessage::ResumeStoredJob {
				id,
				dyn_job,
				dyn_tasks,
				db_id,
				db,
				ack_tx,
			}) => {
				ack_tx
					.send(
						runner
							.new_job(id, dyn_job, Some(dyn_tasks), (db_id, db))
							.await,
					)
					.expect("ack channel closed before sending resume job response");
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

			StreamMessage::RunnerMessage(RunnerMessage::Shutdown) => {
				// Consuming all pending return status messages
				loop {
					while let Ok((job_id, status)) = job_return_status_rx_to_shutdown.try_recv() {
						runner.process_return_status(job_id, status).await;
					}

					if runner.is_empty() {
						break;
					}

					debug!("Waiting for all jobs to complete before shutting down...");
				}

				// Now the runner can shutdown
				if let Err(e) = runner.save_jobs(store_jobs_file).await {
					error!("Failed to save jobs before shutting down: {e:#?}");
				}

				return;
			}

			// Memory cleanup tick
			StreamMessage::CleanMemoryTick => {
				runner.clean_memory();
			}
		}
	}
}
