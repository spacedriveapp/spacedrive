use crate::Error;

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

	#[error("job canceled: <id='{0}'>")]
	Canceled(JobId),

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

	#[error(transparent)]
	Processing(#[from] Error),
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

			JobSystemError::Canceled(_) => {
				Self::with_cause(rspc::ErrorCode::ClientClosedRequest, e.to_string(), e)
			}
			JobSystemError::Processing(e) => e.into(),
			JobSystemError::Report(e) => e.into(),

			_ => Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}
