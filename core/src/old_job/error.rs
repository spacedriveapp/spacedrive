use crate::{
	location::{/*indexer::IndexerError,*/ LocationError},
	object::{
		fs::error::FileSystemJobsError, /*media::old_media_processor::MediaProcessorError,*/
		/*old_file_identifier::FileIdentifierJobError,*/ validation::ValidatorError,
	},
};

// use sd_crypto::Error as CryptoError;
use sd_utils::{db::MissingFieldError, error::FileIOError};

use std::time::Duration;

use prisma_client_rust::QueryError;
use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use thiserror::Error;
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum JobError {
	// General errors
	#[error("database error: {0}")]
	Database(#[from] QueryError),
	#[error("Failed to join Tokio spawn blocking: {0}")]
	JoinTask(#[from] tokio::task::JoinError),
	#[error("job state encode error: {0}")]
	StateEncode(#[from] EncodeError),
	#[error("job state decode error: {0}")]
	StateDecode(#[from] DecodeError),
	#[error("job metadata serialization error: {0}")]
	MetadataSerialization(#[from] serde_json::Error),
	#[error("tried to resume a job with unknown name: job <name='{1}', uuid='{0}'>")]
	UnknownJobName(Uuid, String),
	#[error(
		"Tried to resume a job that doesn't have saved state data: job <name='{1}', uuid='{0}'>"
	)]
	MissingJobDataState(Uuid, String),
	#[error("missing report field: job <uuid='{id}', name='{name}'>")]
	MissingReport { id: Uuid, name: String },
	#[error("missing some job data: '{value}'")]
	MissingData { value: String },
	#[error("invalid job status integer: {0}")]
	InvalidJobStatusInt(i32),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error("Location error: {0}")]
	Location(#[from] LocationError),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("item of type '{0}' with id '{1}' is missing from the db")]
	MissingFromDb(&'static str, String),
	#[error("job timed out after {0:?} without updates")]
	Timeout(Duration),
	#[error("critical job error: {0}")]
	Critical(&'static str),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),

	// Specific job errors
	#[error(transparent)]
	Validator(#[from] ValidatorError),
	#[error(transparent)]
	FileSystemJobsError(#[from] FileSystemJobsError),
	// #[error(transparent)]
	// CryptoError(#[from] CryptoError),

	// Not errors
	#[error("job had a early finish: <name='{name}', reason='{reason}'>")]
	EarlyFinish { name: String, reason: String },
	#[error("data needed for job execution not found: job <name='{0}'>")]
	JobDataNotFound(String),
	#[error("job paused")]
	Paused(Vec<u8>, oneshot::Sender<()>),
	#[error("job canceled")]
	Canceled(oneshot::Sender<()>),
}

#[derive(Error, Debug)]
pub enum JobManagerError {
	#[error("Tried to dispatch a job that is already running: Job <name='{name}', hash='{hash}'>")]
	AlreadyRunningJob { name: &'static str, hash: u64 },

	#[error("Failed to fetch job data from database: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	#[error("job not found: {0}")]
	NotFound(Uuid),

	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
}

impl From<JobManagerError> for rspc::Error {
	fn from(value: JobManagerError) -> Self {
		match value {
			JobManagerError::AlreadyRunningJob { .. } => Self::with_cause(
				rspc::ErrorCode::BadRequest,
				"Tried to spawn a job that is already running!".to_string(),
				value,
			),
			JobManagerError::Database(_) => Self::with_cause(
				rspc::ErrorCode::InternalServerError,
				"Error accessing the database".to_string(),
				value,
			),
			JobManagerError::NotFound(_) => Self::with_cause(
				rspc::ErrorCode::NotFound,
				"Job not found".to_string(),
				value,
			),
			JobManagerError::MissingField(_) => Self::with_cause(
				rspc::ErrorCode::InternalServerError,
				"Missing field".to_string(),
				value,
			),
		}
	}
}
