use super::{
	worker::{Worker, WorkerContext},
	JobError,
};
use crate::{
	node::state,
	prisma::{job, node},
	sync::{crdt::Replicate, engine::SyncContext},
	CoreContext,
};
use anyhow::Result;
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use tokio::sync::Mutex;
use ts_rs::TS;

const MAX_WORKERS: usize = 4;

#[async_trait::async_trait]
pub trait Job: Send + Sync + Debug {
	async fn run(&self, ctx: WorkerContext) -> Result<()>;
	fn name(&self) -> &'static str;
}

// jobs struct is maintained by the core
pub struct Jobs {
	job_queue: Vec<Box<dyn Job>>,
	// workers are spawned when jobs are picked off the queue
	running_workers: HashMap<String, Arc<Mutex<Worker>>>,
}

impl Jobs {
	pub fn new() -> Self {
		Self {
			job_queue: vec![],
			running_workers: HashMap::new(),
		}
	}
	pub async fn ingest(&mut self, ctx: &CoreContext, job: Box<dyn Job>) {
		// create worker to process job
		if self.running_workers.len() < MAX_WORKERS {
			let worker = Worker::new(job);
			let id = worker.id();

			let wrapped_worker = Arc::new(Mutex::new(worker));

			Worker::spawn(wrapped_worker.clone(), ctx).await;

			self.running_workers.insert(id, wrapped_worker);
		} else {
			self.job_queue.push(job);
		}
	}
	pub fn ingest_queue(&mut self, ctx: &CoreContext, job: Box<dyn Job>) {
		self.job_queue.push(job);
	}
	pub async fn complete(&mut self, ctx: &CoreContext, job_id: String) {
		// remove worker from running workers
		self.running_workers.remove(&job_id);
		// continue queue
		let job = self.job_queue.pop();
		if let Some(job) = job {
			self.ingest(ctx, job).await;
		}
	}
	pub async fn get_running(&self) -> Vec<JobReport> {
		let mut ret = vec![];

		for worker in self.running_workers.values() {
			let worker = worker.lock().await;
			ret.push(worker.job_report.clone());
		}
		ret
	}
	pub async fn get_history(ctx: &CoreContext) -> Result<Vec<JobReport>, JobError> {
		let db = &ctx.database;
		let jobs = db
			.job()
			.find_many(vec![job::status::not(JobStatus::Running.int_value())])
			.exec()
			.await?;

		Ok(jobs.into_iter().map(|j| j.into()).collect())
	}
}

#[derive(Debug)]
pub enum JobReportUpdate {
	TaskCount(usize),
	CompletedTaskCount(usize),
	Message(String),
	SecondsElapsed(u64),
}

#[derive(Debug, Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct JobReport {
	pub id: String,
	pub name: String,
	// client_id: i32,
	#[ts(type = "string")]
	pub date_created: chrono::DateTime<chrono::Utc>,
	#[ts(type = "string")]
	pub date_modified: chrono::DateTime<chrono::Utc>,

	pub status: JobStatus,
	pub task_count: i32,
	pub completed_task_count: i32,

	pub message: String,
	// pub percentage_complete: f64,
	#[ts(type = "string")]
	pub seconds_elapsed: i32,
}

// convert database struct into a resource struct
impl Into<JobReport> for job::Data {
	fn into(self) -> JobReport {
		JobReport {
			id: self.id,
			name: self.name,
			// client_id: self.client_id,
			status: JobStatus::from_int(self.status).unwrap(),
			task_count: self.task_count,
			completed_task_count: self.completed_task_count,
			date_created: self.date_created,
			date_modified: self.date_modified,
			message: String::new(),
			seconds_elapsed: self.seconds_elapsed,
		}
	}
}

impl JobReport {
	pub fn new(uuid: String, name: String) -> Self {
		Self {
			id: uuid,
			name,
			// client_id: 0,
			date_created: chrono::Utc::now(),
			date_modified: chrono::Utc::now(),
			status: JobStatus::Queued,
			task_count: 0,
			completed_task_count: 0,
			message: String::new(),
			seconds_elapsed: 0,
		}
	}
	pub async fn create(&self, ctx: &CoreContext) -> Result<(), JobError> {
		let config = state::get();
		ctx.database
			.job()
			.create(
				job::id::set(self.id.clone()),
				job::name::set(self.name.clone()),
				job::action::set(1),
				job::nodes::link(node::id::equals(config.node_id)),
				vec![],
			)
			.exec()
			.await?;
		Ok(())
	}
	pub async fn update(&self, ctx: &CoreContext) -> Result<(), JobError> {
		ctx.database
			.job()
			.find_unique(job::id::equals(self.id.clone()))
			.update(vec![
				job::status::set(self.status.int_value()),
				job::task_count::set(self.task_count),
				job::completed_task_count::set(self.completed_task_count),
				job::date_modified::set(chrono::Utc::now()),
				job::seconds_elapsed::set(self.seconds_elapsed),
			])
			.exec()
			.await?;
		Ok(())
	}
}

#[derive(Clone)]
pub struct JobReportCreate {}

#[async_trait::async_trait]
impl Replicate for JobReport {
	type Create = JobReportCreate;

	async fn create(_data: Self::Create, _ctx: SyncContext) {}
	async fn delete(_ctx: SyncContext) {}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum JobStatus {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
	Failed = 4,
}
