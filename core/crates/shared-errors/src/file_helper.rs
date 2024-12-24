use std::path::Path;

use prisma_client_rust::QueryError;
use sd_prisma::prisma::{file_path, location};
use sd_utils::error::{FileIOError, NonUtf8PathError};
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("file path not found: <id='{0}'>")]
	IdNotFound(file_path::id::Type),
	#[error("file Path not found: <path='{}'>", .0.display())]
	NotFound(Box<Path>),
	#[error("location '{0}' not found")]
	LocationNotFound(location::id::Type),
	#[error("received an invalid sub path: <location_path='{}', sub_path='{}'>", .location_path.display(), .sub_path.display())]
	InvalidSubPath {
		location_path: Box<Path>,
		sub_path: Box<Path>,
	},
	#[error("sub path is not a directory: <path='{}'>", .0.display())]
	SubPathNotDirectory(Box<Path>),
	#[error(
		"the parent directory of the received sub path isn't indexed in the location: <id='{}', sub_path='{}'>",
		.location_id,
		.sub_path.display()
	)]
	SubPathParentNotInLocation {
		location_id: location::id::Type,
		sub_path: Box<Path>,
	},
	#[error("unable to extract materialized path from location: <id='{}', path='{}'>", .location_id, .path.display())]
	UnableToExtractMaterializedPath {
		location_id: location::id::Type,
		path: Box<Path>,
	},
	#[error("database error: {0}")]
	Database(#[from] QueryError),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error("received an invalid filename and extension: <filename_and_extension='{0}'>")]
	InvalidFilenameAndExtension(String),
	#[error(transparent)]
	Sync(#[from] crate::library_sync::Error),
}
