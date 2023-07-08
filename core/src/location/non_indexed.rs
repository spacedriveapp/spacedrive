use crate::{api::locations::ExplorerItem, library::Library, util::error::FileIOError};

use std::path::{Path, PathBuf};

use rspc::ErrorCode;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use tokio::{fs, io};

#[derive(Debug, Error)]
pub enum NonIndexedLocationError {
	#[error("path not found: {}", .0.display())]
	NotFound(PathBuf),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

impl From<NonIndexedLocationError> for rspc::Error {
	fn from(err: NonIndexedLocationError) -> Self {
		match err {
			NonIndexedLocationError::NotFound(path) => {
				rspc::Error::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}
			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

impl<P: AsRef<Path>> From<(P, io::Error)> for NonIndexedLocationError {
	fn from((path, source): (P, io::Error)) -> Self {
		if source.kind() == io::ErrorKind::NotFound {
			Self::NotFound(path.as_ref().into())
		} else {
			Self::FileIO(FileIOError::from((path, source)))
		}
	}
}

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedFileSystemEntries {
	entries: Vec<ExplorerItem>,
	errors: Vec<rspc::Error>,
}

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedPath {
	name: String,
	extension: String,
	kind: i32,
	is_dir: bool,
}

pub async fn walk(
	path: impl AsRef<Path>,
	library: Library,
) -> Result<NonIndexedFileSystemEntries, NonIndexedLocationError> {
	let path = path.as_ref();
	let mut read_dir = fs::read_dir(path).await.map_err(|e| (path, e))?;

	let mut directories = vec![];
	let mut errors = vec![];

	while let Some(entry) = read_dir.next_entry().await.map_err(|e| (path, e))? {
		let entry_path = entry.path();
		let Ok(metadata) = entry.metadata()
			.await
			.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into()))
			else {
			continue;
		};

		if metadata.is_dir() {
			directories.push(entry_path);
		}
	}

	Ok(NonIndexedFileSystemEntries {
		entries: vec![],
		errors,
	})
}
