use sd_task_system::{DispatcherShutdownError, Task};
use sd_utils::error::FileIOError;

use prisma_client_rust::QueryError;

use super::{job::JobName, report::ReportError, JobId};

#[derive(thiserror::Error, Debug)]
pub enum JobSystemError {
	#[error("job not found: <id='{0}'>")]
	NotFound(JobId),
	#[error("job already running: <new_id='{new_id}', name='{job_name}', already_running_id='{already_running_id}'>")]
	AlreadyRunning {
		new_id: JobId,
		job_name: JobName,
		already_running_id: JobId,
	},

	#[error("failed to load job reports from database to resume jobs: {0}")]
	LoadReportsForResume(#[from] QueryError),

	#[error("failed to serialize job to be saved and resumed later: {0}")]
	Serialize(#[from] rmp_serde::encode::Error),

	#[error("failed to deserialize job to be resumed: {0}")]
	Deserialize(#[from] rmp_serde::decode::Error),

	#[error("failed to save or load jobs on disk: {0}")]
	StoredJobs(FileIOError),

	#[error(transparent)]
	Report(#[from] ReportError),

	#[error("internal job panic! <id='{0}'>")]
	Panic(JobId),
}

impl From<JobSystemError> for rspc::Error {
	fn from(e: JobSystemError) -> Self {
		match e {
			JobSystemError::NotFound(_) => {
				Self::with_cause(rspc::ErrorCode::NotFound, e.to_string(), e)
			}

			JobSystemError::AlreadyRunning { .. } => {
				Self::with_cause(rspc::ErrorCode::Conflict, e.to_string(), e)
			}

			JobSystemError::Report(e) => e.into(),

			_ => Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug)]
pub enum DispatcherError {
	#[error("job canceled: <id='{0}'>")]
	JobCanceled(JobId),
	#[error("system entered on shutdown mode <task_count={}>", .0.len())]
	Shutdown(Vec<Box<dyn Task<crate::Error>>>),
}

#[derive(Debug, thiserror::Error)]
pub enum JobErrorOrDispatcherError<JobError: Into<crate::Error>> {
	#[error(transparent)]
	JobError(#[from] JobError),
	#[error(transparent)]
	Dispatcher(#[from] DispatcherError),
}

impl From<DispatcherShutdownError<crate::Error>> for DispatcherError {
	fn from(DispatcherShutdownError(tasks): DispatcherShutdownError<crate::Error>) -> Self {
		Self::Shutdown(tasks)
	}
}
