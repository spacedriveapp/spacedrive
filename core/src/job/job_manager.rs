use crate::{
	encode::{ThumbnailJob, THUMBNAIL_JOB_NAME},
	file::{
		cas::{FileIdentifierJob, IDENTIFIER_JOB_NAME},
		indexer::{IndexerJob, INDEXER_JOB_NAME},
	},
	job::{worker::Worker, DynJob, Job, JobError},
	library::LibraryContext,
	prisma::{self, job, node},
};
use int_enum::IntEnum;
use rspc::Type;
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, VecDeque},
	fmt::Debug,
	fmt::{Display, Formatter},
	sync::Arc,
	time::Duration,
};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::{sync::broadcast, time::sleep};
use tracing::{error, info};
use uuid::Uuid;

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

pub enum JobManagerEvent {
	IngestJob(LibraryContext, Box<dyn DynJob>),
}

// jobs struct is maintained by the core
pub struct JobManager {
	job_queue: RwLock<VecDeque<Box<dyn DynJob>>>,
	// workers are spawned when jobs are picked off the queue
	running_workers: RwLock<HashMap<Uuid, Arc<Mutex<Worker>>>>,
	internal_sender: mpsc::UnboundedSender<JobManagerEvent>,
	shutdown_tx: Arc<broadcast::Sender<()>>,
}

impl JobManager {
	pub fn new() -> Arc<Self> {
		let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
		let (internal_sender, mut internal_receiver) = mpsc::unbounded_channel();
		let this = Arc::new(Self {
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
					JobManagerEvent::IngestJob(ctx, job) => this2.clone().ingest(&ctx, job).await,
				}
			}
		});

		this
	}

	pub async fn ingest(self: Arc<Self>, ctx: &LibraryContext, mut job: Box<dyn DynJob>) {
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

			Worker::spawn(Arc::clone(&self), Arc::clone(&wrapped_worker), ctx.clone()).await;

			running_workers.insert(job_id, wrapped_worker);
		} else {
			self.job_queue.write().await.push_back(job);
		}
	}

	pub async fn ingest_queue(&self, _ctx: &LibraryContext, job: Box<dyn DynJob>) {
		self.job_queue.write().await.push_back(job);
	}

	pub async fn complete(self: Arc<Self>, ctx: &LibraryContext, job_id: Uuid) {
		// remove worker from running workers
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

	pub async fn get_history(ctx: &LibraryContext) -> Result<Vec<JobReport>, prisma::QueryError> {
		let jobs = ctx
			.db
			.job()
			.find_many(vec![job::status::not(JobStatus::Running.int_value())])
			.exec()
			.await?;

		Ok(jobs.into_iter().map(Into::into).collect())
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
						.ingest(ctx, Job::resume(paused_job, Box::new(ThumbnailJob {}))?)
						.await;
				}
				INDEXER_JOB_NAME => {
					Arc::clone(&self)
						.ingest(ctx, Job::resume(paused_job, Box::new(IndexerJob {}))?)
						.await;
				}
				IDENTIFIER_JOB_NAME => {
					Arc::clone(&self)
						.ingest(
							ctx,
							Job::resume(paused_job, Box::new(FileIdentifierJob {}))?,
						)
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
			completed_task_count: 0,
			message: String::new(),
			seconds_elapsed: 0,
		}
	}

	pub async fn create(&self, ctx: &LibraryContext) -> Result<(), JobError> {
		let mut params = Vec::new();

		if self.data.is_some() {
			params.push(job::data::set(self.data.clone()))
		}

		ctx.db
			.job()
			.create(
				self.id.as_bytes().to_vec(),
				self.name.clone(),
				1,
				node::id::equals(ctx.node_local_id),
				params,
			)
			.exec()
			.await?;
		Ok(())
	}
	pub async fn update(&self, ctx: &LibraryContext) -> Result<(), JobError> {
		ctx.db
			.job()
			.find_unique(job::id::equals(self.id.as_bytes().to_vec()))
			.update(vec![
				job::status::set(self.status.int_value()),
				job::data::set(self.data.clone()),
				job::task_count::set(self.task_count),
				job::completed_task_count::set(self.completed_task_count),
				job::date_modified::set(chrono::Utc::now().into()),
				job::seconds_elapsed::set(self.seconds_elapsed),
			])
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
