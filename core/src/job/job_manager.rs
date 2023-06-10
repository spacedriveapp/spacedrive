use crate::{
	invalidate_query,
	job::{worker::Worker, DynJob, Job, JobError, StatefulJob},
	library::Library,
	location::indexer::indexer_job::IndexerJob,
	object::{
		file_identifier::file_identifier_job::FileIdentifierJob,
		fs::{
			copy::FileCopierJob, cut::FileCutterJob, decrypt::FileDecryptorJob,
			delete::FileDeleterJob, encrypt::FileEncryptorJob, erase::FileEraserJob,
		},
		preview::thumbnailer_job::ThumbnailerJob,
		validation::validator_job::ObjectValidatorJob,
	},
	prisma::{job, SortOrder},
};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	sync::{atomic::Ordering, Arc},
};

use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::{JobManagerError, JobReport, JobStatus, WorkerCommand};

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

pub enum JobManagerEvent {
	IngestJob(Library, Box<dyn DynJob>),
}

/// JobManager handles queueing and executing jobs using the `DynJob`
/// Handling persisting JobReports to the database, pause/resuming, and
///
pub struct JobManager {
	current_jobs_hashes: RwLock<HashSet<u64>>,
	job_queue: RwLock<VecDeque<Box<dyn DynJob>>>,
	running_workers: RwLock<HashMap<Uuid, Arc<Mutex<Worker>>>>,
	internal_sender: mpsc::UnboundedSender<JobManagerEvent>,
	shutdown_tx: Arc<broadcast::Sender<()>>,
}

impl JobManager {
	/// Initializes the JobManager and spawns the internal event loop to listen for ingest.
	pub fn new() -> Arc<Self> {
		let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
		let (internal_sender, mut internal_receiver) = mpsc::unbounded_channel();
		let this = Arc::new(Self {
			current_jobs_hashes: RwLock::new(HashSet::new()),
			job_queue: RwLock::new(VecDeque::new()),
			running_workers: RwLock::new(HashMap::new()),
			internal_sender,
			shutdown_tx: Arc::new(shutdown_tx),
		});

		let this2 = this.clone();
		tokio::spawn(async move {
			// FIXME: if this task crashes, the entire application is unusable
			while let Some(event) = internal_receiver.recv().await {
				match event {
					JobManagerEvent::IngestJob(library, job) => {
						this2.clone().dispatch(&library, job).await
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

			let worker = Worker::new(job, job_report);

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

	pub fn shutdown_tx(&self) -> Arc<broadcast::Sender<()>> {
		Arc::clone(&self.shutdown_tx)
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
			worker
				.command(WorkerCommand::Pause)
				.expect("Failed to send pause command");

			Ok(())
		} else {
			Err(JobManagerError::NotFound(job_id))
		}
	}

	pub async fn resume(
		self: Arc<Self>,
		library: &Library,
		id: Option<Uuid>,
	) -> Result<(), JobManagerError> {
		let find_condition = match id {
			Some(id) => vec![
				job::status::equals(JobStatus::Paused as i32),
				job::parent_id::equals(None), // only fetch top-level jobs, they will resume their children
				job::id::equals(id.as_bytes().to_vec()), // only fetch job with specified id
			],
			None => vec![
				job::status::equals(JobStatus::Paused as i32),
				job::parent_id::equals(None), // only fetch top-level jobs, they will resume their children
			],
		};

		for root_paused_job_report in library
			.db
			.job()
			.find_many(find_condition)
			.exec()
			.await?
			.into_iter()
			.map(JobReport::from)
		{
			info!(
				"Resuming job: {} with uuid {}",
				root_paused_job_report.name, root_paused_job_report.id
			);
			let children_jobs = library
				.db
				.job()
				.find_many(vec![job::parent_id::equals(Some(
					root_paused_job_report.id.as_bytes().to_vec(),
				))])
				.order_by(job::action::order(SortOrder::Asc))
				.exec()
				.await?
				.into_iter()
				.map(|job_data| get_resumable_job(JobReport::from(job_data), VecDeque::new()))
				.collect::<Result<_, _>>()?;

			Arc::clone(&self)
				.dispatch(
					library,
					get_resumable_job(root_paused_job_report, children_jobs)?,
				)
				.await;
		}

		Ok(())
	}

	pub async fn get_running(&self) -> Vec<JobReport> {
		let mut ret = vec![];

		for worker in self.running_workers.read().await.values() {
			let report = worker.lock().await.report();
			ret.push(report);
		}
		ret
	}

	pub async fn get_history(library: &Library) -> Result<Vec<JobReport>, JobManagerError> {
		Ok(library
			.db
			.job()
			.find_many(vec![job::status::not(JobStatus::Running as i32)])
			.order_by(job::date_created::order(SortOrder::Desc))
			.take(100)
			.exec()
			.await?
			.into_iter()
			.map(JobReport::from)
			.filter(|report| !report.is_background)
			.collect())
	}

	async fn get_paused_jobs(
		&self,
		library: &Library,
		id: Option<Uuid>,
	) -> Result<Vec<JobReport>, JobManagerError> {
		let find_condition = match id {
			Some(id) => vec![
				job::status::equals(JobStatus::Paused as i32),
				job::parent_id::equals(None), // only fetch top-level jobs, they will resume their children
				job::id::equals(id.as_bytes().to_vec()), // only fetch job with specified id
			],
			None => vec![
				job::status::equals(JobStatus::Paused as i32),
				job::parent_id::equals(None), // only fetch top-level jobs, they will resume their children
			],
		};

		let jobs = library
			.db
			.job()
			.find_many(find_condition)
			.exec()
			.await?
			.into_iter()
			.map(JobReport::from)
			.collect::<Vec<_>>();

		Ok(jobs)
	}

	pub async fn clear_all(library: &Library) -> Result<(), JobManagerError> {
		library.db.job().delete_many(vec![]).exec().await?;

		invalidate_query!(library, "jobs.getHistory");
		Ok(())
	}

	async fn dispatch_group(
		self: Arc<Self>,
		library: &Library,
		job_reports: Vec<JobReport>,
	) -> Result<(), JobManagerError> {
		for root_paused_job_report in job_reports {
			info!(
				"Resuming job: {} with uuid {}",
				root_paused_job_report.name, root_paused_job_report.id
			);
			let children_jobs = library
				.db
				.job()
				.find_many(vec![job::parent_id::equals(Some(
					root_paused_job_report.id.as_bytes().to_vec(),
				))])
				.order_by(job::action::order(SortOrder::Asc))
				.exec()
				.await?
				.into_iter()
				.map(|job_data| get_resumable_job(JobReport::from(job_data), VecDeque::new()))
				.collect::<Result<_, _>>()?;

			Arc::clone(&self)
				.dispatch(
					library,
					get_resumable_job(root_paused_job_report, children_jobs)?,
				)
				.await;
		}
		Ok(())
	}

	pub async fn clear(id: Uuid, library: &Library) -> Result<(), JobManagerError> {
		// unsure whether we should only delete jobs if marked as `JobStatus::Completed`?
		// took inspiration from `clear_all_jobs` for now though
		library
			.db
			.job()
			.delete(job::id::equals(id.as_bytes().to_vec()))
			.exec()
			.await?;

		invalidate_query!(library, "jobs.getHistory");
		Ok(())
	}

	async fn delete_inactive_jobs(&self, library: &Library) -> Result<(), JobManagerError> {
		library
			.db
			.job()
			.delete_many(vec![job::name::not_in_vec(
				JOBS.into_iter().map(|s| s.to_string()).collect(),
			)])
			.exec()
			.await?;
		Ok(())
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
fn get_resumable_job(
	job_report: JobReport,
	next_jobs: VecDeque<Box<dyn DynJob>>,
) -> Result<Box<dyn DynJob>, JobManagerError> {
	dispatch_call_to_job_by_name!(
		job_report.name.as_str(),
		T -> Job::resume(job_report, T {}, next_jobs),
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
	.map_err(Into::into)
}

const JOBS: &[&str] = &[
	ThumbnailerJob::NAME,
	IndexerJob::NAME,
	FileIdentifierJob::NAME,
	ObjectValidatorJob::NAME,
	FileCutterJob::NAME,
	FileCopierJob::NAME,
	FileDeleterJob::NAME,
	FileEraserJob::NAME,
	FileEncryptorJob::NAME,
	FileDecryptorJob::NAME,
];
