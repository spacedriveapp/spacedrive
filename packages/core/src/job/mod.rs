use crate::{
	db::{self, get},
	prisma::{Client, Job, JobData},
	ClientQuery, Core, CoreEvent,
};
use anyhow::Result;
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JobResource {
	pub id: i64,
	pub client_id: i64,
	pub action: JobAction,
	pub status: JobStatus,
	pub percentage_complete: i64,
	pub task_count: i64,
	pub message: String,
	pub completed_task_count: i64,
	#[ts(type = "string")]
	pub date_created: chrono::DateTime<chrono::Utc>,
	#[ts(type = "string")]
	pub date_modified: chrono::DateTime<chrono::Utc>,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum JobAction {
	ScanLoc = 0,
	GeneratePreviewMedia = 1,
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
impl Into<JobResource> for JobData {
	fn into(self) -> JobResource {
		JobResource {
			id: self.id,
			client_id: self.client_id,
			action: JobAction::from_int(self.action).unwrap(),
			status: JobStatus::from_int(self.status).unwrap(),
			task_count: self.task_count,
			completed_task_count: self.completed_task_count,
			date_created: self.date_created,
			date_modified: self.date_modified,
			percentage_complete: 0,
			message: "".to_string(),
		}
	}
}

pub struct JobRunner {
	pub running_jobs: Vec<JobResource>,
	pub queued_jobs: Vec<JobResource>,
}

impl JobRunner {
	pub fn new() -> Self {
		Self {
			running_jobs: Vec::new(),
			queued_jobs: Vec::new(),
		}
	}
}

impl JobResource {
	pub async fn new(client_uuid: String, action: JobAction, task_count: i64) -> Result<Self, JobError> {
		let db = get().await?;
		let client = db
			.client()
			.find_unique(Client::uuid().equals(client_uuid))
			.exec()
			.await
			.unwrap();

		let job = Self {
			id: 0,
			client_id: client.id,
			action,
			status: JobStatus::Queued,
			percentage_complete: 0,
			task_count,
			completed_task_count: 0,
			date_created: chrono::Utc::now(),
			date_modified: chrono::Utc::now(),
			message: "".to_string(),
		};

		db.job().create_one(
			Job::action().set(job.action.int_value()),
			Job::clients().link(Client::id().equals(client.id)),
			vec![],
		);

		Ok(job)
	}

	pub async fn save(&self, core: &Core) -> Result<(), JobError> {
		let db = get().await?;
		let job = db
			.job()
			.find_unique(Job::id().equals(self.id))
			.update(vec![
				Job::status().set(self.status.int_value()),
				Job::completed_task_count().set(self.completed_task_count),
				Job::date_modified().set(chrono::Utc::now()),
			])
			.exec()
			.await;

		core.send(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;

		Ok(())
	}
	pub fn set_progress(&mut self, completed_task_count: Option<i64>, message: Option<String>) -> &Self {
		if let Some(count) = completed_task_count {
			self.completed_task_count = count;
			self.percentage_complete = (count as f64 / self.task_count as f64 * 100.0) as i64;
		}
		if let Some(msg) = message {
			self.message = msg;
		}

		self
	}
	pub fn set_status(&mut self, status: JobStatus, task_count: Option<i64>) -> &Self {
		self.status = status;
		if let Some(count) = task_count {
			self.task_count = count;
		}
		self.set_progress(None, Some("Starting job...".to_string()));
		self
	}

	pub async fn get_running() -> Result<Vec<JobResource>, JobError> {
		let db = get().await?;
		let jobs = db
			.job()
			.find_many(vec![Job::status().equals(JobStatus::Running.int_value())])
			.exec()
			.await;

		Ok(jobs.into_iter().map(|j| j.into()).collect())
	}

	pub async fn get_history() -> Result<Vec<JobResource>, JobError> {
		let db = get().await?;
		let jobs = db
			.job()
			.find_many(vec![
				Job::status().equals(JobStatus::Completed.int_value()),
				Job::status().equals(JobStatus::Canceled.int_value()),
				Job::status().equals(JobStatus::Queued.int_value()),
			])
			.exec()
			.await;

		Ok(jobs.into_iter().map(|j| j.into()).collect())
	}
}

#[derive(Error, Debug)]
pub enum JobError {
	#[error("Failed to create job (job_id {job_id:?})")]
	CreateFailure { job_id: String },
	#[error("Database error")]
	DatabaseError(#[from] db::DatabaseError),
}
