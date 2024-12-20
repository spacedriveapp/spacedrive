#![recursion_limit = "256"]
#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use async_channel as chan;
use futures::Stream;
use futures_concurrency::future::{Join, TryJoin};
use sd_core_job_errors::system::JobSystemError;
use sd_core_job_errors::Error;
use sd_core_shared_types::{jobs::JobName, thumb_key::ThumbKey};
use sd_prisma::prisma::file_path;
use sd_task_system::BaseTaskDispatcher;
use sd_utils::error::FileIOError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{cell::RefCell, collections::hash_map::HashMap, panic, path::Path, sync::Arc};
use tokio::{fs, spawn, sync::oneshot, task::JoinHandle};
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

pub mod job;
pub mod report;
pub mod reversible;
mod runner;
mod store;
pub mod utils;

use crate::job::JobContext;
use job::{IntoJob, Job, JobOutput, OuterContext};
use report::Report;
use runner::{run, JobSystemRunner, RunnerMessage};
use store::{load_jobs, StoredJobEntry};
pub use store::{SerializableJob, SerializedTasks};

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum LocationScanState {
	Pending = 0,
	Indexed = 1,
	FilesIdentified = 2,
	Completed = 3,
}

#[derive(Debug, Serialize, Type)]
pub enum UpdateEvent {
	NewThumbnail {
		thumb_key: ThumbKey,
	},
	NewIdentifiedObjects {
		file_path_ids: Vec<file_path::id::Type>,
	},
}

const PENDING_JOBS_FILE: &str = "pending_jobs.bin";

pub type JobId = Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Command {
	Pause,
	Resume,
	Cancel,
	Shutdown,
}

/// The central unit that orchestrates all the Jobs in the system
///
/// It is responsible for running the jobs and orchestrating how the job queue is allocated
/// in which thread
pub struct JobSystem<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> {
	msgs_tx: chan::Sender<RunnerMessage<OuterCtx, JobCtx>>,
	job_outputs_rx: chan::Receiver<(JobId, Result<JobOutput, Error>)>,
	store_jobs_file: Arc<Path>,
	runner_handle: RefCell<Option<JoinHandle<()>>>,
}

impl<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> JobSystem<OuterCtx, JobCtx> {
	/// Spawn the job system
	pub fn new(
		base_dispatcher: BaseTaskDispatcher<Error>,
		data_directory: impl AsRef<Path>,
	) -> Self {
		let (job_outputs_tx, job_outputs_rx) = chan::unbounded();
		let (job_done_tx, job_done_rx) = chan::bounded(16);
		let (msgs_tx, msgs_rx) = chan::bounded(8);

		let store_jobs_file = Arc::<Path>::from(data_directory.as_ref().join(PENDING_JOBS_FILE));

		let runner_handle = RefCell::new(Some(spawn({
			let store_jobs_file = Arc::clone(&store_jobs_file);
			async move {
				trace!("Job System Runner starting...");
				// keep trying to spawn the job system (tokio) task until succeed
				while let Err(e) = spawn({
					let store_jobs_file = Arc::clone(&store_jobs_file);
					let base_dispatcher = base_dispatcher.clone();
					let job_return_status_tx = job_done_tx.clone();
					let job_done_rx = job_done_rx.clone();
					let job_outputs_tx = job_outputs_tx.clone();
					let msgs_rx = msgs_rx.clone();

					async move {
						run(
							JobSystemRunner::new(
								base_dispatcher,
								job_return_status_tx,
								job_outputs_tx,
							),
							store_jobs_file.as_ref(),
							msgs_rx,
							job_done_rx,
						)
						.await;
					}
				})
				.await
				{
					if e.is_panic() {
						error!(?e, "Job system panicked;");
					} else {
						trace!("JobSystemRunner received shutdown signal and will exit...");
						break;
					}
					trace!("Restarting JobSystemRunner processing task...");
				}

				info!("JobSystemRunner gracefully shutdown");
			}
		})));

		Self {
			msgs_tx,
			job_outputs_rx,
			store_jobs_file,
			runner_handle,
		}
	}

	pub async fn init(
		&self,
		previously_existing_contexts: &HashMap<Uuid, OuterCtx>,
	) -> Result<(), JobSystemError> {
		load_stored_job_entries(
			&*self.store_jobs_file,
			previously_existing_contexts,
			&self.msgs_tx,
		)
		.await
	}

	/// Get a map of all active reports with their respective job ids
	///
	/// # Panics
	///
	/// Panics only happen if internal channels are unexpectedly closed
	pub async fn get_active_reports(&self) -> HashMap<JobId, Report> {
		let (ack_tx, ack_rx) = oneshot::channel();
		self.msgs_tx
			.send(RunnerMessage::GetActiveReports { ack_tx })
			.await
			.expect("runner msgs channel unexpectedly closed on get active reports request");

		ack_rx
			.await
			.expect("ack channel closed before receiving get active reports response")
	}

	/// Checks if *any* of the desired jobs is running for the desired location
	///
	/// # Panics
	///
	/// Panics only happen if internal channels are unexpectedly closed
	pub async fn check_running_jobs(&self, job_names: Vec<JobName>) -> bool {
		let (ack_tx, ack_rx) = oneshot::channel();

		self.msgs_tx
			.send(RunnerMessage::CheckIfJobsAreRunning { job_names, ack_tx })
			.await
			.expect("runner msgs channel unexpectedly closed on check running job request");

		ack_rx
			.await
			.expect("ack channel closed before receiving check running job response")
	}

	/// Shutdown the job system
	///
	/// # Panics
	///
	/// Panics only happen if internal channels are unexpectedly closed
	pub async fn shutdown(&self) {
		if let Some(handle) = self
			.runner_handle
			.try_borrow_mut()
			.ok()
			.and_then(|mut maybe_handle| maybe_handle.take())
		{
			self.msgs_tx
				.send(RunnerMessage::Shutdown)
				.await
				.expect("runner msgs channel unexpectedly closed on shutdown request");

			if let Err(e) = handle.await {
				if e.is_panic() {
					error!(?e, "JobSystem panicked;");
				}
			}
			info!("JobSystem gracefully shutdown");
		} else {
			warn!("JobSystem already shutdown");
		}
	}

	/// Dispatch a new job to the system
	///
	/// # Panics
	///
	/// Panics only happen if internal channels are unexpectedly closed
	pub async fn dispatch<J: Job + SerializableJob<OuterCtx>>(
		&self,
		job: impl IntoJob<J, OuterCtx, JobCtx> + Send,
		ctx: OuterCtx,
	) -> Result<JobId, JobSystemError> {
		let dyn_job = job.into_job();
		let id = dyn_job.id();

		let (ack_tx, ack_rx) = oneshot::channel();
		self.msgs_tx
			.send(RunnerMessage::NewJob {
				job_id: id,
				dyn_job,
				ctx,
				ack_tx,
			})
			.await
			.expect("runner msgs channel unexpectedly closed on new job request");

		ack_rx
			.await
			.expect("ack channel closed before receiving new job request")
			.map(|()| id)
	}

	/// Check if there are any active jobs for the desired [`OuterContext`]
	///
	/// # Panics
	///
	/// Panics only happen if internal channels are unexpectedly closed
	pub async fn has_active_jobs(&self, ctx: OuterCtx) -> bool {
		let ctx_id = ctx.id();

		let (ack_tx, ack_rx) = oneshot::channel();
		self.msgs_tx
			.send(RunnerMessage::HasActiveJobs { ctx_id, ack_tx })
			.await
			.expect("runner msgs channel unexpectedly closed on has active jobs request");

		ack_rx
			.await
			.expect("ack channel closed before receiving has active jobs response")
	}

	pub fn receive_job_outputs(&self) -> impl Stream<Item = (JobId, Result<JobOutput, Error>)> {
		self.job_outputs_rx.clone()
	}

	#[instrument(skip(self), err)]
	async fn send_command(&self, job_id: JobId, command: Command) -> Result<(), JobSystemError> {
		let (ack_tx, ack_rx) = oneshot::channel();
		self.msgs_tx
			.send(RunnerMessage::Command {
				job_id,
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

	pub async fn pause(&self, job_id: JobId) -> Result<(), JobSystemError> {
		self.send_command(job_id, Command::Pause).await
	}

	pub async fn resume(&self, job_id: JobId) -> Result<(), JobSystemError> {
		self.send_command(job_id, Command::Resume).await
	}

	pub async fn cancel(&self, job_id: JobId) -> Result<(), JobSystemError> {
		self.send_command(job_id, Command::Cancel).await
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> Sync
	for JobSystem<OuterCtx, JobCtx>
{
}

async fn load_stored_job_entries<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	store_jobs_file: impl AsRef<Path> + Send,
	previously_existing_job_contexts: &HashMap<Uuid, OuterCtx>,
	msgs_tx: &chan::Sender<RunnerMessage<OuterCtx, JobCtx>>,
) -> Result<(), JobSystemError> {
	let store_jobs_file = store_jobs_file.as_ref();

	let stores_jobs_by_db = rmp_serde::from_slice::<HashMap<Uuid, Vec<StoredJobEntry>>>(
		&match fs::read(store_jobs_file).await {
			Ok(bytes) => bytes,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				debug!("No pending jobs found on disk");
				return Ok(());
			}
			Err(e) => {
				return Err(JobSystemError::StoredJobs(FileIOError::from((
					store_jobs_file,
					e,
					"Failed to load jobs from disk",
				))))
			}
		},
	)?;

	stores_jobs_by_db
		.into_iter()
		.filter_map(|(ctx_id, entries)| {
			previously_existing_job_contexts.get(&ctx_id).map_or_else(
				|| {
					warn!(%ctx_id, "Found stored jobs for a database that doesn't exist anymore;");
					None
				},
				|ctx| Some((entries, ctx.clone())),
			)
		})
		.map(|(entries, ctx)| async move {
			load_jobs(entries, &ctx)
				.await
				.map(|stored_jobs| (stored_jobs, ctx))
		})
		.collect::<Vec<_>>()
		.join()
		.await
		.into_iter()
		.filter_map(|res| {
			res.map_err(|e| error!(?e, "Failed to load stored jobs;"))
				.ok()
		})
		.flat_map(|(stored_jobs, ctx)| {
			stored_jobs
				.into_iter()
				.map(move |(dyn_job, serialized_tasks)| {
					let ctx = ctx.clone();
					async move {
						let (ack_tx, ack_rx) = oneshot::channel();

						msgs_tx
							.send(RunnerMessage::ResumeStoredJob {
								job_id: dyn_job.id(),

								dyn_job,
								ctx,
								serialized_tasks,
								ack_tx,
							})
							.await
							.expect("runner msgs channel unexpectedly closed on stored job resume");

						ack_rx.await.expect(
							"ack channel closed before receiving stored job resume response",
						)
					}
				})
		})
		.collect::<Vec<_>>()
		.try_join()
		.await?;

	fs::remove_file(store_jobs_file).await.map_err(|e| {
		JobSystemError::StoredJobs(FileIOError::from((
			store_jobs_file,
			e,
			"Failed to clean stored jobs file",
		)))
	})
}
