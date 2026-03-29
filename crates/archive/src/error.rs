//! Error types for the sd-archive crate.

use std::path::PathBuf;

/// Core error type for all archive operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("database error: {0}")]
	Database(#[from] sqlx::Error),

	#[error("schema parse error: {0}")]
	SchemaParse(String),

	#[error("schema codegen error: {0}")]
	SchemaCodegen(String),

	#[error("source not found: {0}")]
	SourceNotFound(String),

	#[error("adapter not found: {0}")]
	AdapterNotFound(String),

	#[error("data type not found: {0}")]
	DataTypeNotFound(String),

	#[error("io error: {0}")]
	Io(#[from] std::io::Error),

	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("toml parse error: {0}")]
	Toml(#[from] toml::de::Error),

	#[error("embedding error: {0}")]
	Embedding(String),

	#[error("safety screening error: {0}")]
	Safety(String),

	#[error("search error: {0}")]
	Search(String),

	#[error("adapter sync error: {0}")]
	AdapterSync(String),

	#[error("path not found: {0}")]
	PathNotFound(PathBuf),

	#[error("already exists: {0}")]
	AlreadyExists(String),

	#[error("schema migration refused: {0}")]
	SchemaMigrationRefused(String),

	#[error("{0}")]
	Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
