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

impl Error {
	/// Whether this failed because the volume holding the store is full.
	///
	/// Disk full is a property of the volume rather than of any row, so a
	/// caller that retries row by row only repeats the failure.
	pub fn is_disk_full(&self) -> bool {
		match self {
			Error::Database(error) => sqlx_disk_full(error),
			Error::Io(error) => io_disk_full(error),
			_ => false,
		}
	}
}

/// SQLite reports a full disk, and a database grown past `max_page_count`,
/// as `SQLITE_FULL`.
pub fn sqlx_disk_full(error: &sqlx::Error) -> bool {
	const SQLITE_FULL: i32 = 13;
	match error {
		sqlx::Error::Database(error) => error
			.code()
			.and_then(|code| code.parse::<i32>().ok())
			.is_some_and(|code| code & 0xff == SQLITE_FULL),
		sqlx::Error::Io(error) => io_disk_full(error),
		_ => false,
	}
}

pub fn io_disk_full(error: &std::io::Error) -> bool {
	error.kind() == std::io::ErrorKind::StorageFull
}

pub type Result<T> = std::result::Result<T, Error>;
