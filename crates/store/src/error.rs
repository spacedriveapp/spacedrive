//! Error types for the source store.

/// Everything that can go wrong opening or writing a source store.
#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("database error: {0}")]
	Database(#[from] sqlx::Error),

	#[error("schema parse error: {0}")]
	SchemaParse(String),

	#[error("source not found: {0}")]
	SourceNotFound(String),

	#[error("source index uses an unsupported generation: {0}")]
	UnsupportedGeneration(String),

	#[error("io error: {0}")]
	Io(#[from] std::io::Error),

	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("{0}")]
	Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
