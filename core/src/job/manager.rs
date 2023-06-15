use crate::{
	job::{worker::Worker, DynJob, Job, JobError},
	library::Library,
	location::indexer::indexer_job::IndexerJob,
	object::{
		file_identifier::file_identifier_job::FileIdentifierJob,
		fs::{
			copy::FileCopierJob, cut::FileCutterJob, delete::FileDeleterJob, erase::FileEraserJob,
		},
		preview::thumbnailer_job::ThumbnailerJob,
		validation::validator_job::ObjectValidatorJob,
	},
	prisma::job,
};
use prisma_client_rust::operator::or;

use std::{
	collections::{HashMap, HashSet, VecDeque},
	sync::Arc,
};

use tokio::sync::{
	mpsc::{self, UnboundedSender},
	Mutex, RwLock,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{JobManagerError, JobReport, JobStatus, WorkerCommand};

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

pub enum JobManagerEvent {
	IngestJob(Library, Box<dyn DynJob>),
	Shutdown,
}
/// JobManager handles queueing and executing jobs using the `DynJob`
/// Handling persisting JobReports to the database, pause/resuming, and
///
pub struct JobManager {
	current_jobs_hashes: RwLock<HashSet<u64>>,
	job_queue: RwLock<VecDeque<Box<dyn DynJob>>>,
	running_workers: RwLock<HashMap<Uuid, Arc<Mutex<Worker>>>>,
	internal_sender: UnboundedSender<JobManagerEvent>,
	// pub external_receiver: UnboundedReceiver<JobManagerUpdate>,
	// external_sender: UnboundedSender<JobManagerUpdate>,
}

impl JobManager {
	/// Initializes the JobManager and spawns the internal event loop to listen for ingest.
	pub fn new() -> Arc<Self> {
		// allow the job manager to control its workers
		let (internal_sender, mut internal_receiver) = mpsc::unbounded_channel();
		// // emit realtime events to the rest of the application
		// let (external_sender, external_receiver) = mpsc::unbounded_channel();

		let this = Arc::new(Self {
			current_jobs_hashes: RwLock::new(HashSet::new()),
			job_queue: RwLock::new(VecDeque::new()),
			running_workers: RwLock::new(HashMap::new()),
			internal_sender,
			// external_receiver,
			// external_sender,
		});

		let this2 = this.clone();
		tokio::spawn(async move {
			// FIXME: if this task crashes, the entire application is unusable
			while let Some(event) = internal_receiver.recv().await {
				match event {
					JobManagerEvent::IngestJob(library, job) => {
						this2.clone().dispatch(&library, job).await
					}
					// When the app shuts down, we need to gracefully shutdown all
					// active workers and preserve their state
					JobManagerEvent::Shutdown => {
						info!("Shutting down job manager");
						let mut running_workers = this2.running_workers.write().await;
						for (_, worker) in running_workers.iter_mut() {
							worker
								.lock()
								.await
								.command(WorkerCommand::Shutdown)
								.expect("Failed to send shutdown command to worker");
						}
					}
				}
			}
		});

		debug!("JobManager initialized");
		this
	}

	/// Ingests a new job and dispatches it if possible, queues it otherwise.
	pub async fn ingest(
		self: Arc<Self>,
		library: &Library,
		job: Box<dyn DynJob>,
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
		self.dispatch(library, job).await;
		Ok(())
	}

	/// Dispatches a job to a worker if under MAX_WORKERS limit, queues it otherwise.
	async fn dispatch(self: Arc<Self>, library: &Library, mut job: Box<dyn DynJob>) {
		let mut running_workers = self.running_workers.write().await;

		if running_workers.len() < MAX_WORKERS {
			info!("Running job: {:?}", job.name());

			let job_report = job
				.report_mut()
				.take()
				.expect("critical error: missing job on worker");

			let job_id = job_report.id;

			let worker = Worker::new(
				job, job_report, // , self.external_sender.clone()
			);

			let wrapped_worker = Arc::new(Mutex::new(worker));

			if let Err(e) =
				Worker::spawn(self.clone(), Arc::clone(&wrapped_worker), library.clone()).await
			{
				error!("Error spawning worker: {:?}", e);
			} else {
				running_workers.insert(job_id, wrapped_worker);
			}
		} else {
			debug!(
				"Queueing job: <name='{}', hash='{}'>",
				job.name(),
				job.hash()
			);
			self.job_queue.write().await.push_back(job);
		}
	}

	pub async fn complete(self: Arc<Self>, library: &Library, job_id: Uuid, job_hash: u64) {
		// remove worker from running workers and from current jobs hashes
		self.current_jobs_hashes.write().await.remove(&job_hash);
		self.running_workers.write().await.remove(&job_id);
		// continue queue
		let job = self.job_queue.write().await.pop_front();
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
	pub async fn shutdown(self: Arc<Self>) {
		self.internal_sender
			.send(JobManagerEvent::Shutdown)
			.unwrap_or_else(|_| {
				error!("Failed to send shutdown event to job manager!");
			});
	}

	// Pause a specific job.
	pub async fn pause(&self, job_id: Uuid) -> Result<(), JobManagerError> {
		// Get a read lock on the running workers.
		let workers_guard = self.running_workers.read().await;

		// Look up the worker for the given job ID.
		if let Some(worker_mutex) = workers_guard.get(&job_id) {
			// Lock the worker.
			let worker = worker_mutex.lock().await;

			info!("Pausing job: {:?}", worker.report());

			// Set the pause signal in the worker.
			worker.pause();

			Ok(())
		} else {
			Err(JobManagerError::NotFound(job_id))
		}
	}
	// Resume a specific job.
	pub async fn resume(&self, job_id: Uuid) -> Result<(), JobManagerError> {
		// Get a read lock on the running workers.
		let workers_guard = self.running_workers.read().await;

		// Look up the worker for the given job ID.
		if let Some(worker_mutex) = workers_guard.get(&job_id) {
			// Lock the worker.
			let worker = worker_mutex.lock().await;

			info!("Resuming job: {:?}", worker.report());

			// Set the pause signal in the worker.
			worker.resume();

			Ok(())
		} else {
			Err(JobManagerError::NotFound(job_id))
		}
	}

	/// This is called at startup to resume all paused jobs or jobs that were running
	/// when the core was shut down.
	/// - It will resume jobs that contain data and cancel jobs that do not.
	/// - Prevents jobs from being stuck in a paused/running state
	pub async fn cold_resume(self: Arc<Self>, library: &Library) -> Result<(), JobManagerError> {
		// Include the Queued status in the initial find condition
		let find_condition = vec![or(vec![
			job::status::equals(JobStatus::Paused as i32),
			job::status::equals(JobStatus::Running as i32),
			job::status::equals(JobStatus::Queued as i32),
		])];

		let all_jobs = library
			.db
			.job()
			.find_many(find_condition)
			.exec()
			.await?
			.into_iter()
			.map(JobReport::from);

		for job in all_jobs {
			match initialize_resumable_job(job.clone(), None) {
				Ok(resumable_job) => {
					info!("Resuming job: {} with uuid {}", job.name, job.id);
					Arc::clone(&self).dispatch(library, resumable_job).await;
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
							vec![job::status::set(JobStatus::Canceled as i32)],
						)
						.exec()
						.await?;
				}
			}
		}
		Ok(())
	}

	// get all active jobs, including paused jobs
	pub async fn get_active_reports(&self) -> HashMap<String, JobReport> {
		let mut active_reports = HashMap::new();
		for worker in self.running_workers.read().await.values() {
			let report = worker.lock().await.report();
			active_reports.insert(report.get_meta().0, report);
		}
		active_reports
	}
	// get all running jobs, excluding paused jobs
	pub async fn get_running_reports(&self) -> HashMap<String, JobReport> {
		let mut active_reports = HashMap::new();
		for worker in self.running_workers.read().await.values() {
			let worker = worker.lock().await;
			if !worker.is_paused() {
				let report = worker.report();
				active_reports.insert(report.get_meta().0, report);
			}
		}
		active_reports
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
		T -> Job::new_from_report(job_report, T {}, next_jobs),
		default = {
			error!(
				"Unknown job type: {}, id: {}",
				job_report.name, job_report.id
			);
			Err(JobError::UnknownJobName(job_report.id, job_report.name))
		},
		jobs = [
			ThumbnailerJob,
			IndexerJob,
			FileIdentifierJob,
			ObjectValidatorJob,
			FileCutterJob,
			FileCopierJob,
			FileDeleterJob,
			FileEraserJob,
		]
	)
}
