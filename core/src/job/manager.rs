use crate::{
	job::{worker::Worker, DynJob, Job, JobError},
	library::LoadedLibrary,
	location::indexer::indexer_job::IndexerJobInit,
	object::{
		file_identifier::file_identifier_job::FileIdentifierJobInit,
		fs::{
			copy::FileCopierJobInit, cut::FileCutterJobInit, delete::FileDeleterJobInit,
			erase::FileEraserJobInit,
		},
		preview::thumbnailer_job::ThumbnailerJobInit,
		validation::validator_job::ObjectValidatorJobInit,
	},
	prisma::job,
	Node,
};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	sync::Arc,
};

use futures::future::join_all;
use prisma_client_rust::operator::or;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{JobManagerError, JobReport, JobStatus, StatefulJob};

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

pub enum JobManagerEvent {
	IngestJob(Arc<LoadedLibrary>, Box<dyn DynJob>),
	Shutdown(oneshot::Sender<()>, Arc<JobManager>),
}

#[must_use = "'JobManagerActor::start' must be called to start the actor"]
pub struct JobManagerActor {
	job_manager: Arc<JobManager>,
	internal_receiver: mpsc::UnboundedReceiver<JobManagerEvent>,
}

impl JobManagerActor {
	pub fn start(mut self, node: Arc<Node>) {
		tokio::spawn(async move {
			// FIXME: if this task crashes, the entire application is unusable
			while let Some(event) = self.internal_receiver.recv().await {
				match event {
					JobManagerEvent::IngestJob(library, job) => {
						self.job_manager
							.clone()
							.dispatch(&node, &library, job)
							.await
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

/// JobManager handles queueing and executing jobs using the `DynJob`
/// Handling persisting JobReports to the database, pause/resuming, and
///
pub struct JobManager {
	current_jobs_hashes: RwLock<HashSet<u64>>,
	job_queue: RwLock<VecDeque<Box<dyn DynJob>>>,
	running_workers: RwLock<HashMap<Uuid, Worker>>,
	internal_sender: mpsc::UnboundedSender<JobManagerEvent>,
}

impl JobManager {
	/// Initializes the JobManager and spawns the internal event loop to listen for ingest.
	pub fn new() -> (Arc<Self>, JobManagerActor) {
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
			JobManagerActor {
				job_manager: this,
				internal_receiver,
			},
		)
	}

	/// Ingests a new job and dispatches it if possible, queues it otherwise.
	pub async fn ingest(
		self: Arc<Self>,
		node: &Arc<Node>,
		library: &Arc<LoadedLibrary>,
		job: Box<Job<impl StatefulJob>>,
	) -> Result<(), JobManagerError> {
		let job_hash = job.hash();

		if self.current_jobs_hashes.read().await.contains(&job_hash) {
			return Err(JobManagerError::AlreadyRunningJob {
				name: job.name(),
				hash: job_hash,
			});
		}

		debug!(
			"Ingesting job: <name='{}', hash='{}'>",
			job.name(),
			job_hash
		);

		self.current_jobs_hashes.write().await.insert(job_hash);
		self.dispatch(node, library, job).await;
		Ok(())
	}

	/// Dispatches a job to a worker if under MAX_WORKERS limit, queues it otherwise.
	async fn dispatch(
		self: Arc<Self>,
		node: &Arc<Node>,
		library: &Arc<LoadedLibrary>,
		mut job: Box<dyn DynJob>,
	) {
		let mut running_workers = self.running_workers.write().await;
		let mut job_report = job
			.report_mut()
			.take()
			.expect("critical error: missing job on worker");

		if running_workers.len() < MAX_WORKERS {
			info!("Running job: {:?}", job.name());

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
					error!("Error spawning worker: {:#?}", e);
				},
				|worker| {
					running_workers.insert(worker_id, worker);
				},
			);
		} else {
			debug!(
				"Queueing job: <name='{}', hash='{}'>",
				job.name(),
				job.hash()
			);
			if let Err(e) = job_report.create(library).await {
				// It's alright to just log here, as will try to create the report on run if it wasn't created before
				error!("Error creating job report: {:#?}", e);
			}

			// Put the report back, or it will be lost forever
			*job.report_mut() = Some(job_report);

			self.job_queue.write().await.push_back(job);
		}
	}

	pub async fn complete(
		self: Arc<Self>,
		library: &Arc<LoadedLibrary>,
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

	/// Pause a specific job.
	pub async fn pause(&self, job_id: Uuid) -> Result<(), JobManagerError> {
		// Look up the worker for the given job ID.
		if let Some(worker) = self.running_workers.read().await.get(&job_id) {
			debug!("Pausing job: {:#?}", worker.report());

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
			debug!("Resuming job: {:?}", worker.report());

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
			debug!("Canceling job: {:#?}", worker.report());

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
		library: &Arc<LoadedLibrary>,
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
			.map(JobReport::try_from);

		for job in all_jobs {
			let job = job?;

			match initialize_resumable_job(job.clone(), None) {
				Ok(resumable_job) => {
					info!("Resuming job: {} with uuid {}", job.name, job.id);
					Arc::clone(&self)
						.dispatch(node, library, resumable_job)
						.await;
				}
				Err(err) => {
					warn!(
						"Failed to initialize job: {} with uuid {}, error: {:?}",
						job.name, job.id, err
					);
					info!("Cancelling job: {} with uuid {}", job.name, job.id);
					library
						.db
						.job()
						.update(
							job::id::equals(job.id.as_bytes().to_vec()),
							vec![job::status::set(Some(JobStatus::Canceled as i32))],
						)
						.exec()
						.await?;
				}
			}
		}
		Ok(())
	}

	// get all active jobs, including paused jobs organized by job id
	pub async fn get_active_reports_with_id(&self) -> HashMap<Uuid, JobReport> {
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
	pub async fn get_running_reports(&self) -> HashMap<String, JobReport> {
		self.running_workers
			.read()
			.await
			.values()
			.filter_map(|worker| {
				(!worker.is_paused()).then(|| {
					let report = worker.report();
					(report.get_meta().0, report)
				})
			})
			.collect()
	}

	/// Check if the manager currently has some active workers.
	pub async fn has_active_workers(&self) -> bool {
		for worker in self.running_workers.read().await.values() {
			if !worker.is_paused() {
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
                $(<$job as $crate::job::StatefulJob>::NAME => {
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
	job_report: JobReport,
	next_jobs: Option<VecDeque<Box<dyn DynJob>>>,
) -> Result<Box<dyn DynJob>, JobError> {
	dispatch_call_to_job_by_name!(
		job_report.name.as_str(),
		T -> Job::<T>::new_from_report(job_report, next_jobs),
		default = {
			error!(
				"Unknown job type: {}, id: {}",
				job_report.name, job_report.id
			);
			Err(JobError::UnknownJobName(job_report.id, job_report.name))
		},
		jobs = [
			ThumbnailerJobInit,
			IndexerJobInit,
			FileIdentifierJobInit,
			ObjectValidatorJobInit,
			FileCutterJobInit,
			FileCopierJobInit,
			FileDeleterJobInit,
			FileEraserJobInit,
		]
	)
}
