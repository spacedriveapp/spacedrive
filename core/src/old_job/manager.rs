use crate::{
	library::Library,
	object::{
		fs::{
			old_copy::OldFileCopierJobInit, old_cut::OldFileCutterJobInit,
			old_delete::OldFileDeleterJobInit, old_erase::OldFileEraserJobInit,
		},
		validation::old_validator_job::OldObjectValidatorJobInit,
	},
	old_job::{worker::Worker, DynJob, JobError, OldJob},
	Node,
};

use sd_prisma::prisma::job;

use std::{
	collections::{HashMap, HashSet, VecDeque},
	sync::Arc,
};

use futures::future::join_all;
use prisma_client_rust::operator::or;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use super::{JobIdentity, JobManagerError, JobStatus, OldJobReport, StatefulJob};

const MAX_WORKERS: usize = 5;

pub enum JobManagerEvent {
	IngestJob(Arc<Library>, Box<dyn DynJob>),
	Shutdown(oneshot::Sender<()>, Arc<OldJobs>),
}

#[must_use = "'job::manager::Actor::start' must be called to start the actor"]
pub struct Actor {
	jobs: Arc<OldJobs>,
	internal_receiver: mpsc::UnboundedReceiver<JobManagerEvent>,
}

impl Actor {
	pub fn start(mut self, node: Arc<Node>) {
		tokio::spawn(async move {
			// FIXME: if this task crashes, the entire application is unusable
			while let Some(event) = self.internal_receiver.recv().await {
				match event {
					JobManagerEvent::IngestJob(library, job) => {
						self.jobs.clone().dispatch(&node, &library, job).await
					}
					// When the app shuts down, we need to gracefully shutdown all
					// active workers and preserve their state
					JobManagerEvent::Shutdown(signal_tx, this) => {
						info!("Shutting down job manager");
						let running_workers = this.running_workers.read().await;
						join_all(running_workers.values().map(|worker| worker.shutdown())).await;

						signal_tx.send(()).ok();
					}
				}
			}
		});
	}
}

/// JobManager handles queueing and executing jobs using the [`DynJob`]
/// Handling persisting JobReports to the database, pause/resuming
pub struct OldJobs {
	current_jobs_hashes: RwLock<HashSet<u64>>,
	job_queue: RwLock<VecDeque<Box<dyn DynJob>>>,
	running_workers: RwLock<HashMap<Uuid, Worker>>,
	internal_sender: mpsc::UnboundedSender<JobManagerEvent>,
}

impl OldJobs {
	/// Initializes the JobManager and spawns the internal event loop to listen for ingest.
	pub fn new() -> (Arc<Self>, Actor) {
		// allow the job manager to control its workers
		let (internal_sender, internal_receiver) = mpsc::unbounded_channel();
		let this = Arc::new(Self {
			current_jobs_hashes: RwLock::new(HashSet::new()),
			job_queue: RwLock::new(VecDeque::new()),
			running_workers: RwLock::new(HashMap::new()),
			internal_sender,
		});

		(
			this.clone(),
			Actor {
				jobs: this,
				internal_receiver,
			},
		)
	}

	#[instrument(
		skip_all,
		fields(library_id = %library.id, job_name = %job.name(), job_hash = %job.hash()),
		err,
	)]
	/// Ingests a new job and dispatches it if possible, queues it otherwise.
	pub async fn ingest(
		self: Arc<Self>,
		node: &Arc<Node>,
		library: &Arc<Library>,
		job: Box<OldJob<impl StatefulJob>>,
	) -> Result<(), JobManagerError> {
		let job_hash = job.hash();

		if self.current_jobs_hashes.read().await.contains(&job_hash) {
			return Err(JobManagerError::AlreadyRunningJob {
				name: job.name(),
				hash: job_hash,
			});
		}

		debug!("Ingesting job;");

		self.current_jobs_hashes.write().await.insert(job_hash);
		self.dispatch(node, library, job).await;
		Ok(())
	}

	#[instrument(
		skip_all,
		fields(library_id = %library.id, job_name = %job.name(), job_hash = %job.hash()),
	)]
	/// Dispatches a job to a worker if under MAX_WORKERS limit, queues it otherwise.
	async fn dispatch(
		self: Arc<Self>,
		node: &Arc<Node>,
		library: &Arc<Library>,
		mut job: Box<dyn DynJob>,
	) {
		let mut running_workers = self.running_workers.write().await;
		let mut job_report = job
			.report_mut()
			.take()
			.expect("critical error: missing job on worker");

		if running_workers.len() < MAX_WORKERS {
			info!("Running job");

			let worker_id = job_report.parent_id.unwrap_or(job_report.id);

			Worker::new(
				worker_id,
				job,
				job_report,
				library.clone(),
				node.clone(),
				self.clone(),
			)
			.await
			.map_or_else(
				|e| {
					error!(?e, "Error spawning worker;");
				},
				|worker| {
					running_workers.insert(worker_id, worker);
				},
			);
		} else {
			debug!("Queueing job");
			if let Err(e) = job_report.create(library).await {
				// It's alright to just log here, as will try to create the report on run if it wasn't created before
				error!(?e, "Error creating job report;");
			}

			// Put the report back, or it will be lost forever
			*job.report_mut() = Some(job_report);

			self.job_queue.write().await.push_back(job);
		}
	}

	pub async fn complete(
		self: Arc<Self>,
		library: &Arc<Library>,
		worker_id: Uuid,
		job_hash: u64,
		next_job: Option<Box<dyn DynJob>>,
	) {
		// remove worker from running workers and from current jobs hashes
		self.current_jobs_hashes.write().await.remove(&job_hash);
		self.running_workers.write().await.remove(&worker_id);
		// continue queue
		let job = if next_job.is_some() {
			next_job
		} else {
			self.job_queue.write().await.pop_front()
		};

		if let Some(job) = job {
			// We can't directly execute `self.ingest` here because it would cause an async cycle.
			self.internal_sender
				.send(JobManagerEvent::IngestJob(library.clone(), job))
				.unwrap_or_else(|_| {
					error!("Failed to ingest job!");
				});
		}
	}

	/// Shutdown the job manager, signaled by core on shutdown.
	pub async fn shutdown(self: &Arc<Self>) {
		let (tx, rx) = oneshot::channel();
		self.internal_sender
			.send(JobManagerEvent::Shutdown(tx, self.clone()))
			.unwrap_or_else(|_| {
				error!("Failed to send shutdown event to job manager!");
			});

		rx.await.unwrap_or_else(|_| {
			error!("Failed to receive shutdown event response from job manager!");
		});
	}

	#[instrument(skip(self))]
	/// Pause a specific job.
	pub async fn pause(&self, job_id: Uuid) -> Result<(), JobManagerError> {
		// Look up the worker for the given job ID.
		if let Some(worker) = self.running_workers.read().await.get(&job_id) {
			debug!(report = ?worker.report(), "Pausing job;");

			// Set the pause signal in the worker.
			worker.pause().await;

			Ok(())
		} else {
			Err(JobManagerError::NotFound(job_id))
		}
	}
	/// Resume a specific job.
	pub async fn resume(&self, job_id: Uuid) -> Result<(), JobManagerError> {
		// Look up the worker for the given job ID.
		if let Some(worker) = self.running_workers.read().await.get(&job_id) {
			debug!(report = ?worker.report(), "Resuming job;");

			// Set the pause signal in the worker.
			worker.resume().await;

			Ok(())
		} else {
			Err(JobManagerError::NotFound(job_id))
		}
	}

	/// Cancel a specific job.
	pub async fn cancel(&self, job_id: Uuid) -> Result<(), JobManagerError> {
		// Look up the worker for the given job ID.
		if let Some(worker) = self.running_workers.read().await.get(&job_id) {
			debug!(report = ?worker.report(), "Canceling job;");

			// Set the cancel signal in the worker.
			worker.cancel().await;

			Ok(())
		} else {
			Err(JobManagerError::NotFound(job_id))
		}
	}

	/// This is called at startup to resume all paused jobs or jobs that were running
	/// when the core was shut down.
	/// - It will resume jobs that contain data and cancel jobs that do not.
	/// - Prevents jobs from being stuck in a paused/running state
	pub async fn cold_resume(
		self: Arc<Self>,
		node: &Arc<Node>,
		library: &Arc<Library>,
	) -> Result<(), JobManagerError> {
		// Include the Queued status in the initial find condition
		let find_condition = vec![or(vec![
			job::status::equals(Some(JobStatus::Paused as i32)),
			job::status::equals(Some(JobStatus::Running as i32)),
			job::status::equals(Some(JobStatus::Queued as i32)),
		])];

		let all_jobs = library
			.db
			.job()
			.find_many(find_condition)
			.exec()
			.await?
			.into_iter()
			.map(OldJobReport::try_from);

		for job in all_jobs {
			let job = job?;

			match initialize_resumable_job(job.clone(), None) {
				Ok(resumable_job) => {
					info!(%job.name, %job.id, "Resuming job;");
					Arc::clone(&self)
						.dispatch(node, library, resumable_job)
						.await;
				}
				Err(JobError::UnknownJobName(_, job_name))
					if matches!(
						job_name.as_str(),
						"indexer" | "file_identifier" | "media_processor"
					) =>
				{
					debug!(%job_name, "Moved to new job system");
				}
				Err(e) => {
					warn!(
						%job.name,
						%job.id,
						?e,
						"Failed to initialize job;",
					);

					info!(%job.name, %job.id, "Cancelling job;");

					library
						.db
						.job()
						.update(
							job::id::equals(job.id.as_bytes().to_vec()),
							vec![job::status::set(Some(JobStatus::Canceled as i32))],
						)
						.select(job::select!({ id }))
						.exec()
						.await?;
				}
			}
		}
		Ok(())
	}

	// get all active jobs, including paused jobs organized by job id
	pub async fn get_active_reports_with_id(&self) -> HashMap<Uuid, OldJobReport> {
		self.running_workers
			.read()
			.await
			.values()
			.map(|worker| {
				let report = worker.report();
				(report.id, report)
			})
			.collect()
	}

	// get all running jobs, excluding paused jobs organized by action
	pub async fn get_running_reports(&self) -> HashMap<String, OldJobReport> {
		self.running_workers
			.read()
			.await
			.values()
			.filter(|&worker| !worker.is_paused())
			.map(|worker| {
				let report = worker.report();
				(report.get_meta().0, report)
			})
			.collect()
	}

	/// Check if the manager currently has some active workers.
	pub async fn has_active_workers(&self, library_id: Uuid) -> bool {
		self.running_workers
			.read()
			.await
			.values()
			.any(|worker| worker.library_id == library_id && !worker.is_paused())
	}

	pub async fn has_job_running(&self, predicate: impl Fn(JobIdentity) -> bool) -> bool {
		for worker in self.running_workers.read().await.values() {
			if worker.who_am_i().await.map(&predicate).unwrap_or(false) {
				return true;
			}
		}
		false
	}
}

#[macro_use]
mod macros {
	macro_rules! dispatch_call_to_job_by_name {
        ($job_name:expr, T -> $call:expr, default = $default:block, jobs = [ $($job:ty),+ $(,)?]) => {{
            match $job_name {
                $(<$job as $crate::old_job::StatefulJob>::NAME => {
                    type T = $job;
                    $call
                },)+
                _ => $default
            }
        }};
    }
}
/// This function is used to initialize a  DynJob from a job report.
fn initialize_resumable_job(
	job_report: OldJobReport,
	next_jobs: Option<VecDeque<Box<dyn DynJob>>>,
) -> Result<Box<dyn DynJob>, JobError> {
	dispatch_call_to_job_by_name!(
		job_report.name.as_str(),
		T -> OldJob::<T>::new_from_report(job_report, next_jobs),
		default = {
			error!(
				%job_report.name,
				%job_report.id,
				"Unknown job type;",
			);
			Err(JobError::UnknownJobName(job_report.id, job_report.name))
		},
		jobs = [
			OldObjectValidatorJobInit,
			OldFileCutterJobInit,
			OldFileCopierJobInit,
			OldFileDeleterJobInit,
			OldFileEraserJobInit,
		]
	)
}
