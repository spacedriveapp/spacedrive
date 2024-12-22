use rmp_serde::{decode, encode};
use rspc::ErrorCode;
use sd_utils::db::MissingFieldError;
pub use sd_utils::error::{FileIOError, NonUtf8PathError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	// User errors
	#[error("invalid indexer rule kind integer: {0}")]
	InvalidRuleKindInt(i32),
	#[error("glob builder error: {0}")]
	Glob(#[from] globset::Error),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),

	// Internal Errors
	#[error("indexer rule parameters encode error: {0}")]
	RuleParametersRMPEncode(#[from] encode::Error),
	#[error("indexer rule parameters decode error: {0}")]
	RuleParametersRMPDecode(#[from] decode::Error),
	#[error("accept by its children file I/O error: {0}")]
	AcceptByItsChildrenFileIO(FileIOError),
	#[error("reject by its children file I/O error: {0}")]
	RejectByItsChildrenFileIO(FileIOError),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::InvalidRuleKindInt(_) | Error::Glob(_) | Error::NonUtf8Path(_) => {
				Self::with_cause(ErrorCode::BadRequest, e.to_string(), e)
			}

			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug)]
pub enum SeederError {
	#[error("Failed to run indexer rules seeder: {0}")]
	IndexerRules(#[from] Error),
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("Failed to parse indexer rules based on external system")]
	InheritedExternalRules,
}
