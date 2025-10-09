//! Common types used across the SDK

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// SDK error types
#[derive(Error, Debug)]
pub enum Error {
	#[error("Serialization error: {0}")]
	Serialization(String),

	#[error("Deserialization error: {0}")]
	Deserialization(String),

	#[error("Host call failed: {0}")]
	HostCall(String),

	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	#[error("Operation failed: {0}")]
	OperationFailed(String),

	#[error("Invalid input: {0}")]
	InvalidInput(String),
}

/// Result type for SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// Entry types in VDFS
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EntryType {
	File,
	Directory,
	FinancialDocument,
	Email,
	Receipt,
	Custom(String),
}

impl Default for EntryType {
	fn default() -> Self {
		EntryType::File
	}
}
