//! Common types used across the SDK

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Re-export commonly used types
pub use uuid::Uuid;

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

	#[error("Missing data: {0}")]
	MissingData(String),

	#[error("Not found")]
	NotFound,

	/// The host has no provider for what was asked: no model of that kind is
	/// installed, no tool is present. The call was understood and refused.
	#[error("Not available: {0}")]
	NotAvailable(String),

	/// The SDK declares this call but no host function backs it yet.
	#[error("Unsupported by this host: {0}")]
	Unsupported(String),

	/// A task ran past the timeout its `#[task]` attribute declares.
	#[error("Timed out: {0}")]
	Timeout(String),

	/// The job was asked to pause or cancel.
	#[error("Interrupted")]
	Interrupted,
}

impl Error {
	/// Whether a task that failed with this error is worth running again.
	///
	/// A refusal, a missing provider or an interrupt will come back the same;
	/// a timeout or a failed operation might not.
	pub fn is_retryable(&self) -> bool {
		matches!(
			self,
			Error::Timeout(_) | Error::OperationFailed(_) | Error::HostCall(_)
		)
	}
}

/// Result type for SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// Agent result type
pub type AgentResult<T> = std::result::Result<T, Error>;

/// Job result type
pub type JobResult<T> = std::result::Result<T, Error>;

/// Query result type
pub type QueryResult<T> = std::result::Result<T, Error>;

/// A record in a source store: a file, directory or symlink an ingest saw.
///
/// This is the SDK's view of `sd_store::Record` with its filesystem facet.
/// Records replace the old `Entry`: a record's identity survives a move, its
/// bytes are identified by `content_uuid` once hashing has reached them, and
/// its path is relative to the source that holds it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
	pub uuid: Uuid,
	/// The source whose store holds this record.
	pub source_id: Uuid,
	pub name: String,
	pub kind: RecordKind,
	/// Lowercase, without the dot.
	pub extension: Option<String>,
	/// Relative to the source root, `/`-separated.
	pub relative_path: String,
	pub size: Option<u64>,
	/// Unix milliseconds.
	pub modified_ms: Option<i64>,
	/// The identity of the bytes, once the hash job has reached them.
	pub content_uuid: Option<Uuid>,
}

impl Record {
	/// The record uuid.
	pub fn id(&self) -> Uuid {
		self.uuid
	}

	/// The identity of the bytes, for content-scoped sidecars and models.
	/// `None` means the file has not been hashed yet.
	pub fn content_uuid(&self) -> Option<Uuid> {
		self.content_uuid
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	/// The source-relative path.
	pub fn path(&self) -> &str {
		&self.relative_path
	}

	/// The record's bytes, read through the source's resolved path.
	///
	/// Needs the `read_records` grant for this record's extension; fails
	/// with `NotFound` when the source is detached.
	pub async fn read(&self) -> Result<Vec<u8>> {
		crate::vdfs::VdfsContext.read_record(self.uuid).await
	}

	/// Get custom field from the record's metadata
	pub fn custom_field<T: serde::de::DeserializeOwned>(&self, field: &str) -> Result<T> {
		Err(Error::Unsupported("custom_field".into()))
	}
}

/// What the ingest found at the record's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordKind {
	File,
	Directory,
	Symlink,
}

/// Tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
	pub id: Uuid,
	pub name: String,
	pub color: Option<String>,
	pub icon: Option<String>,
}

/// Priority levels
#[derive(Debug, Clone, Copy)]
pub enum Priority {
	Low,
	Normal,
	High,
}

/// Device capabilities
#[derive(Debug, Clone, Copy)]
pub enum Capability {
	GPU,
	CPU,
}

/// Progress indicator
#[derive(Debug, Clone)]
pub enum Progress {
	Indeterminate(String),
	Simple { fraction: f32, message: String },
	Complete(String),
}

impl Progress {
	pub fn indeterminate(msg: impl Into<String>) -> Self {
		Progress::Indeterminate(msg.into())
	}

	pub fn simple(fraction: f32, msg: impl Into<String>) -> Self {
		Progress::Simple {
			fraction,
			message: msg.into(),
		}
	}

	pub fn complete(msg: impl Into<String>) -> Self {
		Progress::Complete(msg.into())
	}
}

/// Spacedrive path
pub type SdPath = String;

/// Image type marker
pub struct Image;

/// PDF type marker
pub struct Pdf;

/// Permission types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
	ReadEntries,
	WriteEntries,
	ReadSidecars {
		kinds: Vec<String>,
	},
	WriteSidecars {
		kinds: Vec<String>,
	},
	WriteTags,
	WriteCustomFields {
		namespace: String,
	},
	DispatchJobs,
	UseModel {
		category: String,
		preference: ModelPreference,
	},
	RegisterModel {
		category: String,
		max_memory_mb: u64,
	},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelPreference {
	LocalOnly,
	ApiAllowed,
	BundledWithExtension,
}

/// Job error type
#[derive(Error, Debug)]
pub enum JobError {
	#[error("Job failed: {0}")]
	Failed(String),

	#[error("Missing data: {0}")]
	MissingData(String),
}

impl JobError {
	pub fn missing_data(msg: impl Into<String>) -> Self {
		JobError::MissingData(msg.into())
	}
}

/// Query error type
#[derive(Error, Debug)]
pub enum QueryError {
	#[error("Not found")]
	NotFound,

	#[error("Query failed: {0}")]
	Failed(String),
}

/// Task error type
pub type TaskError = Error;

// Implement From conversions for common error types
impl From<serde_json::Error> for Error {
	fn from(err: serde_json::Error) -> Self {
		Error::Serialization(err.to_string())
	}
}

impl From<QueryError> for Error {
	fn from(err: QueryError) -> Self {
		match err {
			QueryError::NotFound => Error::NotFound,
			QueryError::Failed(msg) => Error::OperationFailed(msg),
		}
	}
}
