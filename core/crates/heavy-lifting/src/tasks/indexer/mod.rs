use std::path::Path;

use sd_core_file_path_helper::FilePathError;
use sd_core_indexer_rules::IndexerRuleError;

use sd_utils::{
	db::MissingFieldError,
	error::{FileIOError, NonUtf8PathError},
};

use rspc::ErrorCode;
use serde::{Deserialize, Serialize};
use specta::Type;

pub mod saver;
pub mod updater;
pub mod walker;

#[derive(thiserror::Error, Debug)]
pub enum IndexerError {
	// Not Found errors
	#[error("indexer rule not found: <id='{0}'>")]
	IndexerRuleNotFound(i32),
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal Errors
	#[error("database Error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error(transparent)]
	IsoFilePath(#[from] FilePathError),
	#[error("missing field on database: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("failed to deserialized stored tasks for job resume: {0}")]
	DeserializeTasks(#[from] rmp_serde::decode::Error),

	// Mixed errors
	#[error(transparent)]
	Rules(#[from] IndexerRuleError),
}

impl From<IndexerError> for rspc::Error {
	fn from(err: IndexerError) -> Self {
		match err {
			IndexerError::IndexerRuleNotFound(_) | IndexerError::SubPathNotFound(_) => {
				Self::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			IndexerError::Rules(rule_err) => rule_err.into(),

			_ => Self::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
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
}
