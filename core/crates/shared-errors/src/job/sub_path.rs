use prisma_client_rust::QueryError;
use rspc::ErrorCode;
use sd_core_file_helper::FilePathError;
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	#[error("database error: {0}")]
	Database(#[from] QueryError),

	#[error(transparent)]
	IsoFilePath(#[from] FilePathError),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::SubPathNotFound(_) => Self::with_cause(ErrorCode::NotFound, e.to_string(), e),

			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}
