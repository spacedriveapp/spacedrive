use crate::sub_path;
use prisma_client_rust::QueryError;
use rspc::ErrorCode;
\use sd_core_library_sync::DevicePubId;
use sd_prisma::prisma::indexer_rule;
use sd_utils::db::MissingFieldError;
use sd_utils::error::{FileIOError, NonUtf8PathError};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	// Not Found errors
	#[error("indexer rule not found: <id='{0}'>")]
	IndexerRuleNotFound(indexer_rule::id::Type),
	#[error(transparent)]
	SubPath(#[from] sub_path::Error),
	#[error("device not found: <device_pub_id='{0}'")]
	DeviceNotFound(DevicePubId),

	// Internal Errors
	#[error("database error: {0}")]
	Database(#[from] QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error(transparent)]
	IsoFilePath(#[from] crate::file_helper::Error),
	#[error(transparent)]
	Sync(#[from] crate::library_sync::Error),
	#[error("missing field on database: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("failed to deserialized stored tasks for job resume: {0}")]
	DeserializeTasks(#[from] rmp_serde::decode::Error),

	// Mixed errors
	#[error(transparent)]
	Rules(#[from] crate::indexer_rules::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::IndexerRuleNotFound(_) => {
				Self::with_cause(ErrorCode::NotFound, e.to_string(), e)
			}

			Error::SubPath(sub_path_err) => sub_path_err.into(),

			Error::Rules(rule_err) => rule_err.into(),

			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NonCriticalIndexerError {
	#[error("failed to read directory entry: {0}")]
	FailedDirectoryEntry(String),
	#[error("failed to fetch metadata: {0}")]
	Metadata(String),
	#[error("error applying indexer rule: {0}")]
	IndexerRule(String),
	#[error("error trying to extract file path metadata from a file: {0}")]
	FilePathMetadata(String),
	#[error("failed to fetch file paths ids from existing files on database: {0}")]
	FetchAlreadyExistingFilePathIds(String),
	#[error("failed to fetch file paths to be removed from database: {0}")]
	FetchFilePathsToRemove(String),
	#[error("error constructing isolated file path: {0}")]
	IsoFilePath(String),
	#[error("failed to dispatch new task to keep walking a directory: {0}")]
	DispatchKeepWalking(String),
	#[error("missing file_path data on database: {0}")]
	MissingFilePathData(String),
}
