use crate::{db, file::indexer::IndexerJob, prisma::JobData, Core, CoreContext};
use anyhow::Result;
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;
use tokio::sync::mpsc::{self, Sender, UnboundedSender};
use ts_rs::TS;

#[async_trait::async_trait]
pub trait Job: Send + Sync + Debug {
	async fn run(&self, core: CoreContext) -> Result<()>;
}

// a struct to handle the runtime and execution of jobs
pub struct Jobs {
	// messaging channel for jobs
	pub job_sender_channel: Sender<JobCommand>,
	// in memory cache of jobs with metadata for external use
	pub jobs: Vec<JobMetadata>,
}

impl Jobs {
	pub fn new(ctx: CoreContext) -> Self {
		let (job_sender, mut job_receiver) = mpsc::channel(100);
		// open a thread to handle job execution
		tokio::spawn(async move {
			// local memory for job queue
			let mut job_is_running = false;
			let mut queued_jobs: Vec<Box<dyn Job>> = vec![];
			loop {
				tokio::select! {
					// when job is received via message channel
					Some(request) = job_receiver.recv() => {
						match request {
							// create a new job
							JobCommand::Create(job) => {
								println!("Creating job: {:?}", job);
								queued_jobs.push(job);
								if !job_is_running {
									let job = queued_jobs.pop().unwrap();
									// push this job into running jobs
									job_is_running = true;
									// open a dedicated blocking thread to run job
									let ctx = ctx.clone();
									tokio::task::spawn_blocking(move || {
										// asynchronously call run method
										tokio::runtime::Handle::current().block_on(job.run(ctx))
									});
								}
							}
							// update a running job
							JobCommand::Update { id: _, data: _ } => {
								break;
							}
						}
					}
				}
			}
		});

		Self {
			job_sender_channel: job_sender,
			jobs: vec![],
		}
	}

	pub async fn queue(&mut self, job: Box<dyn Job>) {
		self.job_sender_channel
			.send(JobCommand::Create(job))
			.await
			.unwrap_or(());
	}
}

pub enum JobCommand {
	Create(Box<dyn Job>),
	Update { id: i64, data: JobUpdateEvent },
}

pub struct JobUpdateEvent {
	pub task_count: Option<i64>,
	pub completed_task_count: Option<i64>,
	pub message: Option<String>,
}

pub struct JobContext {
	job_id: i64,
	job_sender: UnboundedSender<(i64, JobUpdateEvent)>,
}

impl JobContext {
	pub async fn send(&self, event: JobUpdateEvent) {
		self.job_sender.send((self.job_id, event)).unwrap_or(());
	}
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JobMetadata {
	id: i64,
	client_id: i64,
	#[ts(type = "string")]
	date_created: chrono::DateTime<chrono::Utc>,
	#[ts(type = "string")]
	date_modified: chrono::DateTime<chrono::Utc>,
	// mutable status
	pub status: JobStatus,
	pub task_count: i64,
	pub completed_task_count: i64,
	pub message: String,
}

pub enum JobRegister {
	IndexerJob(IndexerJob),
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum JobStatus {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
}

// convert database struct into a resource struct
impl Into<JobMetadata> for JobData {
	fn into(self) -> JobMetadata {
		JobMetadata {
			id: self.id,
			client_id: self.client_id,
			status: JobStatus::from_int(self.status).unwrap(),
			task_count: self.task_count,
			completed_task_count: self.completed_task_count,
			date_created: self.date_created,
			date_modified: self.date_modified,
			message: "".to_string(),
		}
	}
}

// impl Job {
// 	pub async fn new<F>(client_uuid: String, action: JobAction, task_count: i64) -> Result<Self, JobError> {
// 		let db = get().await?;
// 		let client = db
// 			.client()
// 			.find_unique(Client::uuid().equals(client_uuid))
// 			.exec()
// 			.await
// 			.unwrap();

// 		let job = Self {
// 			id: 0,
// 			client_id: client.id,
// 			action,
// 			status: JobStatus::Queued,
// 			task_count,
// 			completed_task_count: 0,
// 			date_created: chrono::Utc::now(),
// 			date_modified: chrono::Utc::now(),
// 			message: "".to_string(),
// 		};

// 		db.job().create_one(
// 			prisma::Job::action().set(job.action.int_value()),
// 			prisma::Job::clients().link(Client::id().equals(client.id)),
// 			vec![],
// 		);

// 		Ok(job)
// 	}

// 	pub async fn save(&self, core: &Core) -> Result<(), JobError> {
// 		let db = get().await?;
// 		db.job()
// 			.find_unique(prisma::Job::id().equals(self.id))
// 			.update(vec![
// 				prisma::Job::status().set(self.status.int_value()),
// 				prisma::Job::completed_task_count().set(self.completed_task_count),
// 				prisma::Job::date_modified().set(chrono::Utc::now()),
// 			])
// 			.exec()
// 			.await;

// 		core.send(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;

// 		Ok(())
// 	}
// 	pub fn set_progress(&mut self, completed_task_count: Option<i64>, message: Option<String>) -> &Self {
// 		if let Some(count) = completed_task_count {
// 			self.completed_task_count = count;
// 			self.percentage_complete = (count as f64 / self.task_count as f64 * 100.0) as i64;
// 		}
// 		if let Some(msg) = message {
// 			self.message = msg;
// 		}

// 		self
// 	}
// 	pub fn set_status(&mut self, status: JobStatus, task_count: Option<i64>) -> &Self {
// 		self.status = status;
// 		if let Some(count) = task_count {
// 			self.task_count = count;
// 		}
// 		self.set_progress(None, Some("Starting job...".to_string()));
// 		self
// 	}

// 	pub async fn get_running() -> Result<Vec<Job>, JobError> {
// 		let db = get().await?;
// 		let jobs = db
// 			.job()
// 			.find_many(vec![prisma::Job::status().equals(JobStatus::Running.int_value())])
// 			.exec()
// 			.await;

// 		Ok(jobs.into_iter().map(|j| j.into()).collect())
// 	}

// 	pub async fn get_history() -> Result<Vec<Job>, JobError> {
// 		let db = get().await?;
// 		let jobs = db
// 			.job()
// 			.find_many(vec![
// 				prisma::Job::status().equals(JobStatus::Completed.int_value()),
// 				prisma::Job::status().equals(JobStatus::Canceled.int_value()),
// 				prisma::Job::status().equals(JobStatus::Queued.int_value()),
// 			])
// 			.exec()
// 			.await;

// 		Ok(jobs.into_iter().map(|j| j.into()).collect())
// 	}
// }

#[derive(Error, Debug)]
pub enum JobError {
	#[error("Failed to create job (job_id {job_id:?})")]
	CreateFailure { job_id: String },
	#[error("Database error")]
	DatabaseError(#[from] db::DatabaseError),
}
