use std::path::Path;

use sd_core_file_path_helper::FilePathError;
use sd_core_indexer_rules::IndexerRuleError;

use sd_utils::error::{FileIOError, NonUtf8PathError};

use rspc::ErrorCode;

pub mod saver;
pub mod walker;

#[derive(thiserror::Error, Debug)]
pub enum IndexerError {
	// Not Found errors
	#[error("indexer rule not found: <id='{0}'>")]
	IndexerRuleNotFound(i32),
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal Errors
	#[error("Database Error: {}", .0.to_string())]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error(transparent)]
	IsoFilePath(#[from] FilePathError),

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
