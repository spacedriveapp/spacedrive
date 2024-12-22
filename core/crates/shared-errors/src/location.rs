use std::path::Path;
use thiserror::Error;

use sd_prisma::prisma::location;

#[derive(Error, Debug)]
pub enum LocationManagerError {
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
	FilePath(#[from] sd_core_file_helper::FilePathError),
	#[error(transparent)]
	IndexerRuler(#[from] sd_core_indexer_rules::Error),
	#[error(transparent)]
	JobSystem(#[from] sd_core_job_errors::Error),
	#[error(transparent)]
	FileIO(#[from] sd_utils::error::FileIOError),
	#[error(transparent)]
	Sync(#[from] sd_core_library_sync::Error),
}
