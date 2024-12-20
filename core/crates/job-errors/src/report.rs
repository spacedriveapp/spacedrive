use std::string::ParseError;

use prisma_client_rust::QueryError;
use sd_utils::db::MissingFieldError;

use uuid::Uuid;

pub type JobId = Uuid;

#[derive(thiserror::Error, Debug)]

pub enum ReportError {
	#[error("failed to create job report in database: {0}")]
	Create(QueryError),
	#[error("failed to update job report in database: {0}")]
	Update(QueryError),
	#[error("invalid job status integer: {0}")]
	InvalidJobStatusInt(i32),
	#[error("job not found in database: <id='{0}'>")]
	MissingReport(JobId),
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),
	#[error(transparent)]
	MissingField(#[from] MissingFieldError),
	#[error("failed to parse job name from database: {0}")]
	JobNameParse(#[from] ParseError),
}

impl From<ReportError> for rspc::Error {
	fn from(e: ReportError) -> Self {
		match e {
			ReportError::Create(_)
			| ReportError::Update(_)
			| ReportError::InvalidJobStatusInt(_) => {
				Self::with_cause(rspc::ErrorCode::BadRequest, e.to_string(), e)
			}

			ReportError::MissingReport(_) => {
				Self::with_cause(rspc::ErrorCode::NotFound, e.to_string(), e)
			}
			ReportError::Json(_) | ReportError::MissingField(_) | ReportError::JobNameParse(_) => {
				Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
			}
		}
	}
}
