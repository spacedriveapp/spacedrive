use sd_prisma::prisma::location;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("location not found in database: <id={0}>")]
	LocationNotFound(location::id::Type),

	#[error("watcher error: {0}")]
	Watcher(#[from] notify::Error),

	#[error("non local location: <id='{0}'>")]
	NonLocalLocation(location::id::Type),

	#[error("file still exists on disk after remove event received: <path='{}'>", .0.display())]
	FileStillExistsOnDisk(Box<Path>),

	#[error("failed to move file '{}' for reason: {reason}", .path.display())]
	MoveError {
		path: Box<Path>,
		reason: &'static str,
	},

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("corrupted location pub_id on database: {0}")]
	CorruptedLocationPubId(#[from] uuid::Error),
	#[error("missing field: {0}")]
	MissingField(#[from] sd_utils::db::MissingFieldError),

	#[error(transparent)]
	FilePath(#[from] crate::file_helper::Error),
	#[error(transparent)]
	IndexerRuler(#[from] crate::indexer_rules::Error),
	#[error(transparent)]
	JobSystem(#[from] crate::job::Error),
	#[error(transparent)]
	FileIO(#[from] sd_utils::error::FileIOError),
	#[error(transparent)]
	Sync(#[from] crate::library_sync::Error),
}
