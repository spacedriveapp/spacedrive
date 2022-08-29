pub mod indexer_job;
pub mod indexer_rules;
mod walk;

use globset::Error;
use indexer_rules::RuleKind;
use int_enum::IntEnumError;
use rmp_serde::{decode::Error as RMPDecodeError, encode::Error as RMPEncodeError};
use rspc::ErrorCode;
use serde_json::Error as SerdeJsonError;
use std::io;
use thiserror::Error;

/// Error type for the indexer module
#[derive(Error, Debug)]
pub enum IndexerError {
	// Not Found errors
	#[error("Indexer rule not found: <id={0}>")]
	IndexerRuleNotFound(i32),

	// User errors
	#[error("Invalid indexer rule kind integer: {0}")]
	InvalidRuleKindInt(#[from] IntEnumError<RuleKind>),
	#[error("Glob builder error: {0}")]
	GlobBuilderError(#[from] Error),

	// Internal Errors
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("I/O error: {0}")]
	IOError(#[from] io::Error),
	#[error("Indexer rule parameters json serialization error: {0}")]
	RuleParametersSerdeJson(#[from] SerdeJsonError),
	#[error("Indexer rule parameters encode error: {0}")]
	RuleParametersRMPEncode(#[from] RMPEncodeError),
	#[error("Indexer rule parameters decode error: {0}")]
	RuleParametersRMPDecode(#[from] RMPDecodeError),
}

impl From<IndexerError> for rspc::Error {
	fn from(err: IndexerError) -> Self {
		match err {
			IndexerError::IndexerRuleNotFound(_) => {
				rspc::Error::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			IndexerError::InvalidRuleKindInt(_) | IndexerError::GlobBuilderError(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}
