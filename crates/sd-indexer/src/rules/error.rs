use rmp_serde::{decode, encode};
use rspc::ErrorCode;
use sd_utils::{
	db::MissingFieldError,
	error::{FileIOError, NonUtf8PathError},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexerRuleError {
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
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),

	#[error("{0}")]
	Other(String),
}

impl From<String> for IndexerRuleError {
	fn from(err: String) -> Self {
		IndexerRuleError::Other(err)
	}
}

impl From<IndexerRuleError> for rspc::Error {
	fn from(err: IndexerRuleError) -> Self {
		match err {
			IndexerRuleError::InvalidRuleKindInt(_)
			| IndexerRuleError::Glob(_)
			| IndexerRuleError::NonUtf8Path(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}
			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}
