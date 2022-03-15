use crate::{
	db::get,
	prisma::{Client, Job, JobData},
	ClientQuery, Core, CoreEvent,
};
use anyhow::Result;
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
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
			percentage_complete: self.percentage_complete,
			task_count: self.task_count,
			completed_task_count: self.completed_task_count,
			date_created: self.date_created,
			date_modified: self.date_modified,
		}
	}
}

impl JobResource {
	pub async fn new(core: &Core, action: JobAction, task_count: i64) -> Result<Self> {
		let db = get().await?;
		let client = db
			.client()
			.find_unique(Client::uuid().equals(core.state.client_id.clone()))
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
		};

		db.job().create_one(
			Job::action().set(job.action.int_value()),
			Job::clients().link(Client::id().equals(client.id)),
			vec![],
		);

		Ok(job)
	}

	pub async fn update_task_count(&mut self, core: &Core, completed_task_count: i64) -> Result<Self> {
		let db = get().await.unwrap();
		db.job()
			.find_unique(Job::id().equals(self.id))
			.update(vec![Job::completed_task_count().set(completed_task_count)])
			.exec()
			.await;

		self.completed_task_count = completed_task_count;

		core.send(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;

		Ok(self.clone())
	}

	pub async fn update_status(&mut self, core: &Core, status: JobStatus) -> Result<Self> {
		let db = get().await.unwrap();
		db.job()
			.find_unique(Job::id().equals(self.id))
			.update(vec![Job::status().set(status.int_value())])
			.exec()
			.await;

		self.status = status;

		core.send(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;

		Ok(self.clone())
	}
}
