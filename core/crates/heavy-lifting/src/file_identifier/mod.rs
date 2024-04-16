use sd_core_file_path_helper::{FilePathError, IsolatedFilePathData};

use sd_file_ext::{extensions::Extension, kind::ObjectKind};
use sd_utils::error::FileIOError;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::trace;

use std::{fs::Metadata, path::Path};

use prisma_client_rust::QueryError;
use tokio::fs;

mod cas_id;
mod job;
mod shallow;
mod tasks;

use cas_id::generate_cas_id;

#[derive(thiserror::Error, Debug)]
pub enum FileIdentifierError {
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal Errors
	#[error(transparent)]
	FilePathError(#[from] FilePathError),
	#[error("database error: {0}")]
	Database(#[from] QueryError),
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalFileIdentifierError {
	#[error("failed to extract file metadata: {0}")]
	FailedToExtractFileMetadata(String),
	#[cfg(target_os = "windows")]
	#[error("failed to extract metadata from on-demand file: {0}")]
	FailedToExtractMetadataFromOnDemandFile(String),
	#[error("failed to extract isolated file path data: {0}")]
	FailedToExtractIsolatedFilePathData(String),
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
	pub cas_id: Option<String>,
	pub kind: ObjectKind,
	pub fs_metadata: Metadata,
}

impl FileMetadata {
	/// Fetch metadata from the file system and generate a cas id for the file
	/// if it's not empty.
	///
	/// # Panics
	/// Will panic if the file is a directory.
	pub async fn new(
		location_path: impl AsRef<Path> + Send,
		iso_file_path: &IsolatedFilePathData<'_>,
	) -> Result<Self, FileIOError> {
		let path = location_path.as_ref().join(iso_file_path);

		let fs_metadata = fs::metadata(&path)
			.await
			.map_err(|e| FileIOError::from((&path, e)))?;

		assert!(
			!fs_metadata.is_dir(),
			"We can't generate cas_id for directories"
		);

		// derive Object kind
		let kind = Extension::resolve_conflicting(&path, false)
			.await
			.map_or(ObjectKind::Unknown, Into::into);

		let cas_id = if fs_metadata.len() != 0 {
			generate_cas_id(&path, fs_metadata.len())
				.await
				.map(Some)
				.map_err(|e| FileIOError::from((&path, e)))?
		} else {
			// We can't do shit with empty files
			None
		};

		trace!(
			"Analyzed file: <path='{}', cas_id={cas_id:?}, object_kind={kind}>",
			path.display()
		);

		Ok(Self {
			cas_id,
			kind,
			fs_metadata,
		})
	}
}
