use crate::{
	location::{indexer::IndexerError, LocationError},
	object::{
		file_identifier::FileIdentifierJobError, fs::error::FileSystemJobsError,
		preview::ThumbnailerError,
	},
	util::{db::MissingFieldError, error::FileIOError},
};

use std::fmt::Debug;

use prisma_client_rust::QueryError;
use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use sd_crypto::Error as CryptoError;
use thiserror::Error;
use uuid::Uuid;

use super::JobRunErrors;

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
	#[error("error converting/handling OS strings")]
	OsStr,
	#[error("error converting/handling paths")]
	Path,
	#[error("invalid job status integer: {0}")]
	InvalidJobStatusInt(i32),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error("Location error: {0}")]
	Location(#[from] LocationError),
	#[error("job failed to pause: {0}")]
	PauseFailed(String),
	#[error("failed to send command to worker")]
	WorkerCommandSendFailed,

	// Specific job errors
	#[error(transparent)]
	Indexer(#[from] IndexerError),
	#[error(transparent)]
	ThumbnailError(#[from] ThumbnailerError),
	#[error(transparent)]
	IdentifierError(#[from] FileIdentifierJobError),
	#[error(transparent)]
	FileSystemJobsError(#[from] FileSystemJobsError),
	#[error(transparent)]
	CryptoError(#[from] CryptoError),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("item of type '{0}' with id '{1}' is missing from the db")]
	MissingFromDb(&'static str, String),
	#[error("Thumbnail skipped")]
	ThumbnailSkipped,

	// Not errors
	#[error("step completed with errors: {0:?}")]
	StepCompletedWithErrors(JobRunErrors),
	#[error("job had a early finish: <name='{name}', reason='{reason}'>")]
	EarlyFinish { name: String, reason: String },
	#[error("data needed for job execution not found: job <name='{0}'>")]
	JobDataNotFound(String),
	#[error("job paused")]
	Paused(Vec<u8>),
	#[error("job canceled")]
	Canceled(Vec<u8>),
}

#[derive(Error, Debug)]
pub enum JobManagerError {
	#[error("Tried to dispatch a job that is already running: Job <name='{name}', hash='{hash}'>")]
	AlreadyRunningJob { name: &'static str, hash: u64 },

	#[error("Failed to fetch job data from database: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	#[error("job not found: {0}")]
	NotFound(Uuid),
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
		}
	}
}
