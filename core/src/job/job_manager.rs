use crate::{
	invalidate_query,
	job::{worker::Worker, DynJob, Job, JobError},
	library::LibraryContext,
	location::indexer::indexer_job::{IndexerJob, INDEXER_JOB_NAME},
	object::{
		fs::{
			copy::{FileCopierJob, COPY_JOB_NAME},
			cut::{FileCutterJob, CUT_JOB_NAME},
			delete::{FileDeleterJob, DELETE_JOB_NAME},
			erase::{FileEraserJob, ERASE_JOB_NAME},
		},
		identifier_job::full_identifier_job::{FullFileIdentifierJob, FULL_IDENTIFIER_JOB_NAME},
		preview::{ThumbnailJob, THUMBNAIL_JOB_NAME},
		validation::validator_job::{ObjectValidatorJob, VALIDATOR_JOB_NAME},
	},
	prisma::{job, node},
};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	fmt::Debug,
	fmt::{Display, Formatter},
	sync::Arc,
	time::Duration,
};

use int_enum::IntEnum;
use prisma_client_rust::Direction;
use rspc::Type;
use serde::{Deserialize, Serialize};
use tokio::{
	sync::{broadcast, mpsc, Mutex, RwLock},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

pub enum JobManagerEvent {
	IngestJob(LibraryContext, Box<dyn DynJob>),
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
					JobManagerEvent::IngestJob(ctx, job) => {
						this2.clone().dispatch_job(&ctx, job).await
					}
				}
			}
		});

		debug!("JobManager initialized");

		this
	}

	pub async fn ingest(self: Arc<Self>, ctx: &LibraryContext, job: Box<dyn DynJob>) {
		let job_hash = job.hash();
		debug!(
			"Ingesting job: <name='{}', hash='{}'>",
			job.name(),
			job_hash
		);

		if !self.current_jobs_hashes.read().await.contains(&job_hash) {
			self.current_jobs_hashes.write().await.insert(job_hash);
			self.dispatch_job(ctx, job).await;
		} else {
			debug!(
				"Job already in queue: <name='{}', hash='{}'>",
				job.name(),
				job_hash
			);
		}
	}

	pub async fn ingest_queue(&self, job: Box<dyn DynJob>) {
		let job_hash = job.hash();
		debug!("Queueing job: <name='{}', hash='{}'>", job.name(), job_hash);

		if !self.current_jobs_hashes.read().await.contains(&job_hash) {
			self.current_jobs_hashes.write().await.insert(job_hash);
			self.job_queue.write().await.push_back(job);
		} else {
			debug!(
				"Job already in queue: <name='{}', hash='{}'>",
				job.name(),
				job_hash
			);
		}
	}

	pub async fn complete(self: Arc<Self>, ctx: &LibraryContext, job_id: Uuid, job_hash: u64) {
		// remove worker from running workers and from current jobs hashes
		self.current_jobs_hashes.write().await.remove(&job_hash);
		self.running_workers.write().await.remove(&job_id);
		// continue queue
		let job = self.job_queue.write().await.pop_front();
		if let Some(job) = job {
			// We can't directly execute `self.ingest` here because it would cause an async cycle.
			self.internal_sender
				.send(JobManagerEvent::IngestJob(ctx.clone(), job))
				.unwrap_or_else(|_| {
					error!("Failed to ingest job!");
				});
		}
	}

	pub async fn get_running(&self) -> Vec<JobReport> {
		let mut ret = vec![];

		for worker in self.running_workers.read().await.values() {
			let worker = worker.lock().await;
			ret.push(worker.report());
		}
		ret
	}

	pub async fn get_history(
		ctx: &LibraryContext,
	) -> Result<Vec<JobReport>, prisma_client_rust::QueryError> {
		Ok(ctx
			.db
			.job()
			.find_many(vec![job::status::not(JobStatus::Running.int_value())])
			.order_by(job::date_created::order(Direction::Desc))
			.take(100)
			.exec()
			.await?
			.into_iter()
			.map(Into::into)
			.collect())
	}

	pub async fn clear_all_jobs(
		ctx: &LibraryContext,
	) -> Result<(), prisma_client_rust::QueryError> {
		ctx.db.job().delete_many(vec![]).exec().await?;

		invalidate_query!(ctx, "jobs.getHistory");
		Ok(())
	}

	pub fn shutdown_tx(&self) -> Arc<broadcast::Sender<()>> {
		Arc::clone(&self.shutdown_tx)
	}

	pub async fn pause(&self) {
		let running_workers_read_guard = self.running_workers.read().await;
		if !running_workers_read_guard.is_empty() {
			self.shutdown_tx
				.send(())
				.expect("Failed to send shutdown signal");
		}
		// Dropping our handle so jobs can finish
		drop(running_workers_read_guard);

		loop {
			sleep(Duration::from_millis(50)).await;
			if self.running_workers.read().await.is_empty() {
				break;
			}
		}
	}

	pub async fn resume_jobs(self: Arc<Self>, ctx: &LibraryContext) -> Result<(), JobError> {
		let paused_jobs = ctx
			.db
			.job()
			.find_many(vec![job::status::equals(JobStatus::Paused.int_value())])
			.exec()
			.await?;

		for paused_job_data in paused_jobs {
			let paused_job = JobReport::from(paused_job_data);

			info!("Resuming job: {}, id: {}", paused_job.name, paused_job.id);
			match paused_job.name.as_str() {
				THUMBNAIL_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, ThumbnailJob {})?)
						.await;
				}
				INDEXER_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, IndexerJob {})?)
						.await;
				}
				FULL_IDENTIFIER_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, FullFileIdentifierJob {})?)
						.await;
				}
				VALIDATOR_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, ObjectValidatorJob {})?)
						.await;
				}
				CUT_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, FileCutterJob {})?)
						.await;
				}
				COPY_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(
							ctx,
							Job::resume(paused_job, FileCopierJob { done_tx: None })?,
						)
						.await;
				}
				DELETE_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, FileDeleterJob {})?)
						.await;
				}
				ERASE_JOB_NAME => {
					Arc::clone(&self)
						.dispatch_job(ctx, Job::resume(paused_job, FileEraserJob {})?)
						.await;
				}
				_ => {
					error!(
						"Unknown job type: {}, id: {}",
						paused_job.name, paused_job.id
					);
					return Err(JobError::UnknownJobName(paused_job.id, paused_job.name));
				}
			};
		}

		Ok(())
	}

	async fn dispatch_job(self: Arc<Self>, ctx: &LibraryContext, mut job: Box<dyn DynJob>) {
		// create worker to process job
		let mut running_workers = self.running_workers.write().await;
		if running_workers.len() < MAX_WORKERS {
			info!("Running job: {:?}", job.name());

			let job_report = job
				.report()
				.take()
				.expect("critical error: missing job on worker");

			let job_id = job_report.id;

			let worker = Worker::new(job, job_report);

			let wrapped_worker = Arc::new(Mutex::new(worker));

			if let Err(e) =
				Worker::spawn(Arc::clone(&self), Arc::clone(&wrapped_worker), ctx.clone()).await
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
}

#[derive(Debug)]
pub enum JobReportUpdate {
	TaskCount(usize),
	CompletedTaskCount(usize),
	Message(String),
	SecondsElapsed(u64),
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct JobReport {
	pub id: Uuid,
	pub name: String,
	pub data: Option<Vec<u8>>,
	pub metadata: Option<serde_json::Value>,
	// client_id: i32,
	pub date_created: chrono::DateTime<chrono::Utc>,
	pub date_modified: chrono::DateTime<chrono::Utc>,

	pub status: JobStatus,
	pub task_count: i32,
	pub completed_task_count: i32,

	pub message: String,
	// pub percentage_complete: f64,
	// #[ts(type = "string")] // TODO: Make this work with specta
	pub seconds_elapsed: i32,
}

impl Display for JobReport {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"Job <name='{}', uuid='{}'> {:#?}",
			self.name, self.id, self.status
		)
	}
}

// convert database struct into a resource struct
impl From<job::Data> for JobReport {
	fn from(data: job::Data) -> JobReport {
		JobReport {
			id: Uuid::from_slice(&data.id).unwrap(),
			name: data.name,
			// client_id: data.client_id,
			status: JobStatus::from_int(data.status).unwrap(),
			task_count: data.task_count,
			completed_task_count: data.completed_task_count,
			date_created: data.date_created.into(),
			date_modified: data.date_modified.into(),
			data: data.data,
			metadata: data.metadata.and_then(|m| {
				serde_json::from_slice(&m).unwrap_or_else(|e| -> Option<serde_json::Value> {
					error!("Failed to deserialize job metadata: {}", e);
					None
				})
			}),
			message: String::new(),
			seconds_elapsed: data.seconds_elapsed,
		}
	}
}

impl JobReport {
	pub fn new(uuid: Uuid, name: String) -> Self {
		Self {
			id: uuid,
			name,
			// client_id: 0,
			date_created: chrono::Utc::now(),
			date_modified: chrono::Utc::now(),
			status: JobStatus::Queued,
			task_count: 0,
			data: None,
			metadata: None,
			completed_task_count: 0,
			message: String::new(),
			seconds_elapsed: 0,
		}
	}

	pub async fn create(&self, ctx: &LibraryContext) -> Result<(), JobError> {
		ctx.db
			.job()
			.create(
				self.id.as_bytes().to_vec(),
				self.name.clone(),
				JobStatus::Running as i32,
				node::id::equals(ctx.node_local_id),
				vec![job::data::set(self.data.clone())],
			)
			.exec()
			.await?;
		Ok(())
	}
	pub async fn update(&self, ctx: &LibraryContext) -> Result<(), JobError> {
		ctx.db
			.job()
			.update(
				job::id::equals(self.id.as_bytes().to_vec()),
				vec![
					job::status::set(self.status.int_value()),
					job::data::set(self.data.clone()),
					job::metadata::set(serde_json::to_vec(&self.metadata).ok()),
					job::task_count::set(self.task_count),
					job::completed_task_count::set(self.completed_task_count),
					job::date_modified::set(chrono::Utc::now().into()),
					job::seconds_elapsed::set(self.seconds_elapsed),
				],
			)
			.exec()
			.await?;
		Ok(())
	}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq, IntEnum)]
pub enum JobStatus {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
	Failed = 4,
	Paused = 5,
}
