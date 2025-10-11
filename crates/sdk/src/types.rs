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
}

/// Result type for SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// Agent result type
pub type AgentResult<T> = std::result::Result<T, Error>;

/// Job result type
pub type JobResult<T> = std::result::Result<T, Error>;

/// Query result type
pub type QueryResult<T> = std::result::Result<T, Error>;

/// Entry in VDFS (file, directory, or virtual)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
	pub id: Uuid,
	pub uuid: Option<Uuid>,
	pub name: String,
	pub kind: EntryKind,
	pub extension: Option<String>,
	pub metadata_id: Option<i32>,
	pub content_id: Option<i32>,
	pub size: i64,
}

impl Entry {
	/// Get entry UUID
	pub fn id(&self) -> Uuid {
		self.uuid.unwrap_or(self.id)
	}

	/// Get content UUID (for content-scoped operations)
	pub fn content_uuid(&self) -> Option<Uuid> {
		todo!("Get content UUID from entry")
	}

	/// Get metadata ID
	pub fn metadata_id(&self) -> i32 {
		self.metadata_id.unwrap_or(0)
	}

	/// Get entry name
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Get entry path
	pub fn path(&self) -> String {
		todo!("Get full path")
	}

	/// Read entry data
	pub async fn read(&self) -> Result<Vec<u8>> {
		todo!("WASM host call")
	}

	/// Get custom field from entry's metadata
	pub fn custom_field<T: serde::de::DeserializeOwned>(&self, field: &str) -> Result<T> {
		todo!("WASM host call - read custom_data field")
	}
}

/// Entry kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
	File = 0,
	Directory = 1,
	Symlink = 2,
	Virtual = 3,
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
