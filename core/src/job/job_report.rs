use std::fmt::{Display, Formatter};

use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
	api::utils::LibraryArgs,
	invalidate_query,
	library::LibraryContext,
	prisma::{job, node},
};

use super::JobManager;

/// TODO: Can I remove this?
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
	pub data: Vec<u8>,
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
			data: Vec::new(),
			metadata: None,
			completed_task_count: 0,
			message: String::new(),
			seconds_elapsed: 0,
		}
	}

	pub async fn upsert(
		&self,
		job_manager: &JobManager,
		library_ctx: &LibraryContext,
	) -> Result<job::Data, prisma_client_rust::QueryError> {
		let v = library_ctx
			.db
			.job()
			.upsert(
				job::id::equals(self.id.as_bytes().to_vec()),
				(
					self.id.as_bytes().to_vec(),
					self.name.clone(),
					self.status.int_value(),
					self.data.clone(),
					node::id::equals(library_ctx.node_local_id),
					vec![],
				),
				vec![
					job::status::set(self.status.int_value()),
					job::data::set(self.data.clone()),
					job::metadata::set(match serde_json::to_vec(&self.metadata) {
						Ok(v) => Some(v),
						Err(err) => {
							warn!(
								"Failed to serialize metadata for job '{}' '{}': {}",
								self.name, self.id, err
							);
							None
						}
					}),
					job::task_count::set(self.task_count),
					job::completed_task_count::set(self.completed_task_count),
					job::date_modified::set(chrono::Utc::now().into()),
					job::seconds_elapsed::set(self.seconds_elapsed),
				],
			)
			.exec()
			.await?;

		let running_jobs = job_manager.get_running().await;
		invalidate_query!(library_ctx, "jobs.getRunning": LibraryArgs<()>, LibraryArgs::default(), Vec<JobReport>: running_jobs);

		match job_manager.get_history(library_ctx).await {
			Ok(jobs) => {
				invalidate_query!(library_ctx, "jobs.getHistory":  LibraryArgs<()>, LibraryArgs::default(), Vec<JobReport>: jobs);
			}
			Err(e) => {
				error!("Failed to get job history when invalidating it: {}", e);
			}
		}

		Ok(v)
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
