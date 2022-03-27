use super::worker::{Worker, WorkerContext};
use crate::{prisma::JobData, CoreContext};
use anyhow::Result;
use dyn_clone::DynClone;
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};
use ts_rs::TS;

const MAX_WORKERS: usize = 4;

#[async_trait::async_trait]
pub trait Job: Send + Sync + Debug + DynClone {
	async fn run(&self, ctx: WorkerContext) -> Result<()>;
}

// jobs struct is maintained by the core
pub struct Jobs {
	job_queue: Vec<Box<dyn Job>>,
	// workers are spawned when jobs are picked off the queue
	running_workers: HashMap<String, Worker>,
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
		let mut worker = Worker::new(job);

		if self.running_workers.len() < MAX_WORKERS {
			worker.spawn(ctx).await;
			self.running_workers.insert(worker.id(), worker);
		}
	}
	pub async fn get_running(&self) -> Vec<JobReport> {
		self.running_workers
			.values()
			.into_iter()
			.map(|worker| worker.job_report.clone())
			.collect()
	}
}

pub enum JobReportUpdate {
	TaskCount(i64),
	CompletedTaskCount(i64),
	Message(String),
}

#[derive(Debug, Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct JobReport {
	pub id: String,
	// client_id: i64,
	#[ts(type = "string")]
	pub date_created: chrono::DateTime<chrono::Utc>,
	#[ts(type = "string")]
	pub date_modified: chrono::DateTime<chrono::Utc>,

	pub status: JobStatus,
	pub task_count: i64,
	pub completed_task_count: i64,

	pub message: String,
	// pub percentage_complete: f64,
	#[ts(type = "string")]
	pub seconds_elapsed: i64,
}

// convert database struct into a resource struct
impl Into<JobReport> for JobData {
	fn into(self) -> JobReport {
		JobReport {
			id: self.id,
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
	pub fn new(uuid: String) -> Self {
		Self {
			id: uuid,
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
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum JobStatus {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
	Failed = 4,
}
