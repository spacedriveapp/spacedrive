use sd_utils::error::FileIOError;

mod tasks;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	MediaData(#[from] sd_media_metadata::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}
