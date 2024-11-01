use crate::{Error, JobContext};

use sd_prisma::prisma::location;
use sd_task_system::BaseTaskDispatcher;
use sd_utils::error::FileIOError;

use std::{
	collections::{hash_map::Entry, HashMap, HashSet},
	mem,
	path::Path,
	pin::pin,
	time::Duration,
};

use async_channel as chan;
use chrono::Utc;
use futures::StreamExt;
use futures_concurrency::{
	future::{Join, TryJoin},
	stream::Merge,
};
use serde_json::json;
use tokio::{
	fs,
	sync::oneshot,
	time::{interval_at, Instant},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

use super::{
	job::{DynJob, JobHandle, JobName, JobOutput, OuterContext, ReturnStatus},
	report::{self, ReportOutputMetadata},
	store::{StoredJob, StoredJobEntry},
	Command, JobId, JobSystemError, SerializedTasks,
};

const JOBS_INITIAL_CAPACITY: usize = 32;
const FIVE_MINUTES: Duration = Duration::from_secs(5 * 60);

pub(super) enum RunnerMessage<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> {
	NewJob {
		job_id: JobId,
		location_id: location::id::Type,
		dyn_job: Box<dyn DynJob<OuterCtx, JobCtx>>,
		ctx: OuterCtx,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	ResumeStoredJob {
		job_id: JobId,
		location_id: location::id::Type,
		dyn_job: Box<dyn DynJob<OuterCtx, JobCtx>>,
		ctx: OuterCtx,
		serialized_tasks: Option<SerializedTasks>,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	Command {
		job_id: JobId,
		command: Command,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	},
	GetActiveReports {
		ack_tx: oneshot::Sender<HashMap<JobId, report::Report>>,
	},
	CheckIfJobsAreRunning {
		job_names: Vec<JobName>,
		location_id: location::id::Type,
		ack_tx: oneshot::Sender<bool>,
	},
	Shutdown,
	HasActiveJobs {
		ctx_id: Uuid,
		ack_tx: oneshot::Sender<bool>,
	},
}

struct JobsWorktables {
	job_hashes: HashMap<u64, JobId>,
	job_hashes_by_id: HashMap<JobId, u64>,
	running_jobs_by_job_id: HashMap<JobId, (JobName, location::id::Type)>,
	running_jobs_set: HashSet<(JobName, location::id::Type)>,
	jobs_to_store_by_ctx_id: HashMap<Uuid, Vec<StoredJobEntry>>,
}

pub(super) struct JobSystemRunner<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> {
	on_shutdown_mode: bool,
	base_dispatcher: BaseTaskDispatcher<Error>,
	handles: HashMap<JobId, JobHandle<OuterCtx, JobCtx>>,
	worktables: JobsWorktables,
	job_return_status_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
	job_outputs_tx: chan::Sender<(JobId, Result<JobOutput, Error>)>,
}

impl<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> JobSystemRunner<OuterCtx, JobCtx> {
	pub(super) fn new(
		base_dispatcher: BaseTaskDispatcher<Error>,
		job_return_status_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
		job_outputs_tx: chan::Sender<(JobId, Result<JobOutput, Error>)>,
	) -> Self {
		Self {
			on_shutdown_mode: false,
			base_dispatcher,
			handles: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
			worktables: JobsWorktables {
				job_hashes: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
				job_hashes_by_id: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
				running_jobs_by_job_id: HashMap::with_capacity(JOBS_INITIAL_CAPACITY),
				running_jobs_set: HashSet::with_capacity(JOBS_INITIAL_CAPACITY),
				jobs_to_store_by_ctx_id: HashMap::new(),
			},
			job_return_status_tx,
			job_outputs_tx,
		}
	}

	async fn new_job(
		&mut self,
		job_id: JobId,
		location_id: location::id::Type,
		dyn_job: Box<dyn DynJob<OuterCtx, JobCtx>>,
		ctx: OuterCtx,
		maybe_existing_tasks: Option<SerializedTasks>,
	) -> Result<(), JobSystemError> {
		let Self {
			base_dispatcher,
			handles,
			worktables:
				JobsWorktables {
					job_hashes,
					job_hashes_by_id,
					running_jobs_by_job_id,
					running_jobs_set,
					..
				},
			job_return_status_tx,
			..
		} = self;

		let job_name = dyn_job.job_name();

		let job_hash = dyn_job.hash();
		if let Some(&already_running_id) = job_hashes.get(&job_hash) {
			return Err(JobSystemError::AlreadyRunning {
				new_id: job_id,
				already_running_id,
				job_name,
			});
		}

		running_jobs_by_job_id.insert(job_id, (job_name, location_id));
		running_jobs_set.insert((job_name, location_id));

		job_hashes.insert(job_hash, job_id);
		job_hashes_by_id.insert(job_id, job_hash);

		let mut handle = if maybe_existing_tasks.is_some() {
			dyn_job.resume(
				base_dispatcher.clone(),
				ctx.clone(),
				maybe_existing_tasks,
				job_return_status_tx.clone(),
			)
		} else {
			dyn_job.dispatch(
				base_dispatcher.clone(),
				ctx.clone(),
				job_return_status_tx.clone(),
			)
		};

		handle.register_start(Utc::now()).await?;

		handles.insert(job_id, handle);

		Ok(())
	}

	async fn get_active_reports(&self) -> HashMap<JobId, report::Report> {
		self.handles
			.iter()
			.map(|(job_id, handle)| async { (*job_id, handle.ctx.report().await.clone()) })
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.collect()
	}

	async fn process_command(
		&mut self,
		job_id: JobId,
		command: Command,
		ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	) {
		if let Some(handle) = self.handles.get_mut(&job_id) {
			match (command, handle.is_running) {
				(Command::Pause, false) => {
					warn!("Tried to pause a job already paused");
					return ack_tx.send(Ok(())).expect(
						"ack channel closed before sending response to already paused job",
					);
				}
				(Command::Resume, true) => {
					warn!("Tried to resume a job already running");
					return ack_tx.send(Ok(())).expect(
						"ack channel closed before sending response to already running job",
					);
				}
				_ => {}
			}
			match command {
				Command::Pause | Command::Cancel | Command::Shutdown => {
					handle.is_running = false;
				}
				Command::Resume => {
					handle.is_running = true;
				}
			}
			handle.send_command(command, ack_tx).await;
			handle.ctx.invalidate_query("jobs.isActive");
			handle.ctx.invalidate_query("jobs.reports");
		} else {
			error!("Job not found");
			ack_tx
				.send(Err(JobSystemError::NotFound(job_id)))
				.unwrap_or_else(|_| {
					panic!("ack channel closed before sending {command:?} response")
				});
		}
	}

	fn is_empty(&self) -> bool {
		self.handles.is_empty()
			&& self.worktables.job_hashes.is_empty()
			&& self.worktables.job_hashes_by_id.is_empty()
	}

	fn total_jobs(&self) -> usize {
		self.handles.len()
	}

	fn check_if_jobs_are_running(
		&self,
		job_names: Vec<JobName>,
		location_id: location::id::Type,
	) -> bool {
		job_names.into_iter().any(|job_name| {
			self.worktables
				.running_jobs_set
				.contains(&(job_name, location_id))
		})
	}

	#[instrument(skip_all, fields(%job_id))]
	async fn process_return_status(
		&mut self,
		job_id: JobId,
		status: Result<ReturnStatus, Error>,
	) -> Result<(), JobSystemError> {
		let Self {
			on_shutdown_mode,
			handles,
			worktables,
			job_outputs_tx,
			job_return_status_tx,
			base_dispatcher,
			..
		} = self;

		let job_hash = worktables
			.job_hashes_by_id
			.remove(&job_id)
			.expect("it must be here");

		let (job_name, location_id) = worktables
			.running_jobs_by_job_id
			.remove(&job_id)
			.expect("a JobName and location_id must've been inserted in the map with the job id");

		assert!(worktables.running_jobs_set.remove(&(job_name, location_id)));
		assert!(worktables.job_hashes.remove(&job_hash).is_some());

		let mut handle = handles.remove(&job_id).expect("it must be here");
		handle.run_time += handle.start_time.elapsed();

		handle
			.ctx
			.report_mut()
			.await
			.push_metadata(ReportOutputMetadata::Metrics(HashMap::from([(
				"job_run_time".into(),
				json!(handle.run_time),
			)])));

		let res = match status {
			Ok(ReturnStatus::Completed(job_return)) => {
				try_dispatch_next_job(
					&mut handle,
					location_id,
					base_dispatcher.clone(),
					worktables,
					handles,
					job_return_status_tx.clone(),
				)
				.await?;

				handle.complete_job(job_return).await.map_err(Into::into)
			}

			Ok(ReturnStatus::Shutdown(res)) => {
				match res {
					Ok(Some(serialized_job)) => {
						let name = {
							let db = handle.ctx.db();
							let report = handle.ctx.report().await;
							if let Err(e) = report.update(db).await {
								error!(?e, "Failed to update report on job shutdown;");
							}
							report.name
						};

						worktables
							.jobs_to_store_by_ctx_id
							.entry(handle.ctx.id())
							.or_default()
							.push(StoredJobEntry {
								location_id,
								root_job: StoredJob {
									id: job_id,
									run_time: handle.start_time.elapsed(),
									name,
									serialized_job,
								},
								next_jobs: serialize_next_jobs_to_shutdown(
									job_id,
									job_name,
									handle.next_jobs,
								)
								.await
								.unwrap_or_default(),
							});

						debug!(%name, "Job was shutdown and serialized;");
					}

					Ok(None) => {
						debug!(
							"Job was shutdown but didn't returned any serialized data, \
							probably it isn't resumable job"
						);
					}

					Err(e) => {
						error!(?e, "Failed to serialize job;");
					}
				}

				if *on_shutdown_mode && handles.is_empty() {
					// Job system is empty and in shutdown mode so we close this channel to finish the shutdown process
					job_return_status_tx.close();
				}

				return Ok(());
			}

			Ok(ReturnStatus::Canceled(job_return)) => {
				handle.cancel_job(job_return).await.map_err(Into::into)
			}
			Err(e) => handle
				.failed_job(&e)
				.await
				.map_err(Into::into)
				.and_then(|()| Err(e)),
		};

		job_outputs_tx
			.send((job_id, res))
			.await
			.expect("job outputs channel unexpectedly closed on job completion");

		handle.ctx.invalidate_query("jobs.isActive");
		handle.ctx.invalidate_query("jobs.reports");

		Ok(())
	}

	fn clean_memory(&mut self) {
		if self.handles.capacity() > JOBS_INITIAL_CAPACITY
			&& self.handles.len() < JOBS_INITIAL_CAPACITY
		{
			self.handles.shrink_to(JOBS_INITIAL_CAPACITY);
		}

		if self.worktables.job_hashes.capacity() > JOBS_INITIAL_CAPACITY
			&& self.worktables.job_hashes.len() < JOBS_INITIAL_CAPACITY
		{
			self.worktables.job_hashes.shrink_to(JOBS_INITIAL_CAPACITY);
		}

		if self.worktables.job_hashes_by_id.capacity() > JOBS_INITIAL_CAPACITY
			&& self.worktables.job_hashes_by_id.len() < JOBS_INITIAL_CAPACITY
		{
			self.worktables
				.job_hashes_by_id
				.shrink_to(JOBS_INITIAL_CAPACITY);
		}

		if self.worktables.running_jobs_by_job_id.capacity() > JOBS_INITIAL_CAPACITY
			&& self.worktables.running_jobs_by_job_id.len() < JOBS_INITIAL_CAPACITY
		{
			self.worktables
				.running_jobs_by_job_id
				.shrink_to(JOBS_INITIAL_CAPACITY);
		}

		if self.worktables.running_jobs_set.capacity() > JOBS_INITIAL_CAPACITY
			&& self.worktables.running_jobs_set.len() < JOBS_INITIAL_CAPACITY
		{
			self.worktables
				.running_jobs_set
				.shrink_to(JOBS_INITIAL_CAPACITY);
		}
	}

	async fn save_jobs(
		self,
		store_jobs_file: impl AsRef<Path> + Send,
	) -> Result<(), JobSystemError> {
		let store_jobs_file = store_jobs_file.as_ref();

		let Self {
			handles,
			worktables:
				JobsWorktables {
					job_hashes,
					job_hashes_by_id,
					jobs_to_store_by_ctx_id,
					..
				},
			..
		} = self;

		assert!(
			handles.is_empty() && job_hashes.is_empty() && job_hashes_by_id.is_empty(),
			"All jobs must be completed before saving"
		);

		if jobs_to_store_by_ctx_id.is_empty() {
			info!("No jobs to store in disk for job system shutdown!");
			return Ok(());
		}

		fs::write(
			store_jobs_file,
			rmp_serde::to_vec_named(&jobs_to_store_by_ctx_id)?,
		)
		.await
		.map_err(|e| JobSystemError::StoredJobs(FileIOError::from((store_jobs_file, e))))
	}

	fn has_active_jobs(&self, ctx_id: Uuid) -> bool {
		self.handles
			.values()
			.any(|handle| handle.ctx.id() == ctx_id && handle.is_running)
	}

	async fn dispatch_shutdown_command_to_jobs(&mut self) {
		self.handles
			.values_mut()
			.map(|handle| async move {
				let (tx, rx) = oneshot::channel();

				handle.send_command(Command::Shutdown, tx).await;

				rx.await.expect("Worker failed to ack shutdown request")
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.for_each(|res| {
				if let Err(e) = res {
					error!(?e, "Failed to shutdown job;");
				}
			});
	}
}

#[instrument(skip(next_jobs))]
async fn serialize_next_jobs_to_shutdown<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	parent_job_id: JobId,
	parent_job_name: JobName,
	next_jobs: impl IntoIterator<Item = Box<dyn DynJob<OuterCtx, JobCtx>>> + Send,
) -> Option<Vec<StoredJob>> {
	next_jobs
		.into_iter()
		.map(|next_job| async move {
			let next_id = next_job.id();
			let next_name = next_job.job_name();
			next_job
				.serialize()
				.await
				.map(|maybe_serialized_job| {
					maybe_serialized_job.map(|serialized_job| StoredJob {
						id: next_id,
						run_time: Duration::ZERO,
						name: next_name,
						serialized_job,
					})
				})
				.map_err(|e| {
					error!(%next_id, %next_name, ?e, "Failed to serialize next job;");
				})
		})
		.collect::<Vec<_>>()
		.try_join()
		.await
		.map(|maybe_serialized_next_jobs| {
			maybe_serialized_next_jobs.into_iter().flatten().collect()
		})
		.ok()
}

#[instrument(
	skip_all,
	fields(
		job_id = %handle.id,
		next_jobs_count = handle.next_jobs.len(),
		location_id = %location_id,
		total_running_jobs = handles.len(),
	)
)]
async fn try_dispatch_next_job<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	handle: &mut JobHandle<OuterCtx, JobCtx>,
	location_id: location::id::Type,
	base_dispatcher: BaseTaskDispatcher<Error>,
	JobsWorktables {
		job_hashes,
		job_hashes_by_id,
		running_jobs_by_job_id,
		running_jobs_set,
		..
	}: &mut JobsWorktables,
	handles: &mut HashMap<JobId, JobHandle<OuterCtx, JobCtx>>,
	job_return_status_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
) -> Result<(), JobSystemError> {
	if let Some(next) = handle.next_jobs.pop_front() {
		let next_id = next.id();
		let next_hash = next.hash();
		let next_name = next.job_name();

		if let Entry::Vacant(e) = job_hashes.entry(next_hash) {
			e.insert(next_id);
			trace!(%next_id, %next_name, "Dispatching next job;");

			job_hashes_by_id.insert(next_id, next_hash);
			running_jobs_by_job_id.insert(next_id, (next_name, location_id));
			running_jobs_set.insert((next_name, location_id));

			let mut next_handle = next.dispatch(
				base_dispatcher,
				handle.ctx.get_outer_ctx(),
				job_return_status_tx,
			);

			next_handle.register_start(Utc::now()).await?;

			assert!(
				next_handle.next_jobs.is_empty(),
				"Only the root job will have next jobs, the rest will be empty and \
				we will swap with remaining ones from the previous job"
			);

			next_handle.next_jobs = mem::take(&mut handle.next_jobs);

			handles.insert(next_id, next_handle);
		} else {
			warn!(%next_id, %next_name, "Unexpectedly found a job with the same hash as the next job;");
		}
	} else {
		trace!("No next jobs to dispatch");
	}

	Ok(())
}

pub(super) async fn run<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	mut runner: JobSystemRunner<OuterCtx, JobCtx>,
	store_jobs_file: impl AsRef<Path> + Send,
	msgs_rx: chan::Receiver<RunnerMessage<OuterCtx, JobCtx>>,
	job_done_rx: chan::Receiver<(JobId, Result<ReturnStatus, Error>)>,
) {
	enum StreamMessage<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> {
		ReturnStatus((JobId, Result<ReturnStatus, Error>)),
		RunnerMessage(RunnerMessage<OuterCtx, JobCtx>),
		CleanMemoryTick,
	}

	let memory_cleanup_interval = interval_at(Instant::now() + FIVE_MINUTES, FIVE_MINUTES);

	let job_return_status_rx_to_shutdown = job_done_rx.clone();

	let mut msg_stream = pin!((
		msgs_rx.map(StreamMessage::RunnerMessage),
		job_done_rx.map(StreamMessage::ReturnStatus),
		IntervalStream::new(memory_cleanup_interval).map(|_| StreamMessage::CleanMemoryTick),
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			// Job return status messages
			StreamMessage::ReturnStatus((job_id, status)) => {
				if let Err(e) = runner.process_return_status(job_id, status).await {
					error!(?e, "Failed to process return status;");
				}
			}

			// Runner messages
			StreamMessage::RunnerMessage(RunnerMessage::NewJob {
				job_id,
				location_id,
				dyn_job,
				ctx,
				ack_tx,
			}) => {
				ack_tx
					.send(
						runner
							.new_job(job_id, location_id, dyn_job, ctx, None)
							.await,
					)
					.expect("ack channel closed before sending new job response");
			}

			StreamMessage::RunnerMessage(RunnerMessage::HasActiveJobs { ctx_id, ack_tx }) => {
				ack_tx
					.send(runner.has_active_jobs(ctx_id))
					.expect("ack channel closed before sending has active jobs response");
			}

			StreamMessage::RunnerMessage(RunnerMessage::GetActiveReports { ack_tx }) => {
				ack_tx
					.send(runner.get_active_reports().await)
					.expect("ack channel closed before sending active reports response");
			}
			StreamMessage::RunnerMessage(RunnerMessage::ResumeStoredJob {
				job_id,
				location_id,
				dyn_job,
				ctx,
				serialized_tasks,
				ack_tx,
			}) => {
				ack_tx
					.send(
						runner
							.new_job(job_id, location_id, dyn_job, ctx, serialized_tasks)
							.await,
					)
					.expect("ack channel closed before sending resume job response");
			}

			StreamMessage::RunnerMessage(RunnerMessage::Command {
				job_id: id,
				command,
				ack_tx,
			}) => runner.process_command(id, command, ack_tx).await,

			StreamMessage::RunnerMessage(RunnerMessage::Shutdown) => {
				runner.on_shutdown_mode = true;
				// Consuming all pending return status messages
				if !runner.is_empty() {
					let mut job_return_status_stream = pin!(job_return_status_rx_to_shutdown);

					runner.dispatch_shutdown_command_to_jobs().await;

					debug!(
						total_jobs = runner.total_jobs(),
						"Waiting for jobs to shutdown before shutting down the job system...;",
					);

					while let Some((job_id, status)) = job_return_status_stream.next().await {
						if let Err(e) = runner.process_return_status(job_id, status).await {
							error!(?e, "Failed to process return status before shutting down;");
						}
					}

					// Now the runner can shutdown
					if let Err(e) = runner.save_jobs(store_jobs_file).await {
						error!(?e, "Failed to save jobs before shutting down;");
					}
				}

				return;
			}

			StreamMessage::RunnerMessage(RunnerMessage::CheckIfJobsAreRunning {
				job_names,
				location_id,
				ack_tx,
			}) => {
				ack_tx
					.send(runner.check_if_jobs_are_running(job_names, location_id))
					.expect("ack channel closed before sending resume job response");
			}

			// Memory cleanup tick
			StreamMessage::CleanMemoryTick => runner.clean_memory(),
		}
	}
}
