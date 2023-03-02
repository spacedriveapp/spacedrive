use crate::{library::LibraryContext, prisma::file_path};

use std::{
	path::{Path, PathBuf},
	sync::atomic::{AtomicI32, Ordering},
};

use prisma_client_rust::{Direction, QueryError};
use thiserror::Error;
use tracing::error;

use super::{indexer::indexer_job_location, LocationId};

static LAST_FILE_PATH_ID: AtomicI32 = AtomicI32::new(0);

file_path::select!(file_path_id_only { id });
file_path::include!(file_path_with_object { object });

#[derive(Error, Debug)]
pub enum FilePathError {
	#[error("Unable to extract materialized path from location: <id='{0}', path='{1:?}'>")]
	UnableToExtractMaterializedPath(LocationId, PathBuf),
	#[error("Database error (error: {0:?})")]
	DatabaseError(#[from] QueryError),
}

pub async fn get_max_file_path_id(library_ctx: &LibraryContext) -> Result<i32, FilePathError> {
	let mut last_id = LAST_FILE_PATH_ID.load(Ordering::Acquire);
	if last_id == 0 {
		last_id = fetch_max_file_path_id(library_ctx).await?;
		LAST_FILE_PATH_ID.store(last_id, Ordering::Release);
	}

	Ok(last_id)
}

pub fn set_max_file_path_id(id: i32) {
	LAST_FILE_PATH_ID.store(id, Ordering::Relaxed);
}

async fn fetch_max_file_path_id(library_ctx: &LibraryContext) -> Result<i32, FilePathError> {
	Ok(library_ctx
		.db
		.file_path()
		.find_first(vec![])
		.order_by(file_path::id::order(Direction::Desc))
		.select(file_path_id_only::select())
		.exec()
		.await?
		.map(|r| r.id)
		.unwrap_or(0))
}

#[cfg(feature = "location-watcher")]
pub async fn create_file_path(
	library_ctx: &LibraryContext,
	location_id: i32,
	mut materialized_path: String,
	name: String,
	extension: String,
	parent_id: Option<i32>,
	is_dir: bool,
) -> Result<file_path::Data, FilePathError> {
	use crate::prisma::location;

	let mut last_id = LAST_FILE_PATH_ID.load(Ordering::Acquire);
	if last_id == 0 {
		last_id = fetch_max_file_path_id(library_ctx).await?;
	}

	// If this new file_path is a directory, materialized_path must end with "/"
	if is_dir && !materialized_path.ends_with('/') {
		materialized_path += "/";
	}

	let next_id = last_id + 1;

	let created_path = library_ctx
		.db
		.file_path()
		.create(
			next_id,
			location::id::equals(location_id),
			materialized_path,
			name,
			extension,
			vec![
				file_path::parent_id::set(parent_id),
				file_path::is_dir::set(is_dir),
			],
		)
		.exec()
		.await?;

	LAST_FILE_PATH_ID.store(next_id, Ordering::Release);

	Ok(created_path)
}

pub fn subtract_location_path(
	location_path: impl AsRef<Path>,
	current_path: impl AsRef<Path>,
) -> Option<PathBuf> {
	let location_path = location_path.as_ref();
	let current_path = current_path.as_ref();

	if let Ok(stripped) = current_path.strip_prefix(location_path) {
		Some(stripped.to_path_buf())
	} else {
		error!(
			"Failed to strip location root path ({}) from current path ({})",
			location_path.display(),
			current_path.display()
		);
		None
	}
}

pub fn extract_materialized_path(
	location: &indexer_job_location::Data,
	path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	subtract_location_path(&location.path, &path).ok_or_else(|| {
		FilePathError::UnableToExtractMaterializedPath(location.id, path.as_ref().to_path_buf())
	})
}

pub async fn get_existing_file_path(
	location: &indexer_job_location::Data,
	path: impl AsRef<Path>,
	is_dir: bool,
	library_ctx: &LibraryContext,
) -> Result<Option<file_path_with_object::Data>, FilePathError> {
	let mut materialized_path = extract_materialized_path(location, path)?
		.to_str()
		.expect("Found non-UTF-8 path")
		.to_string();
	if is_dir && !materialized_path.ends_with('/') {
		materialized_path += "/";
	}

	library_ctx
		.db
		.file_path()
		.find_first(vec![file_path::materialized_path::equals(
			materialized_path,
		)])
		// include object for orphan check
		.include(file_path_with_object::include())
		.exec()
		.await
		.map_err(Into::into)
}

pub async fn get_existing_file_or_directory(
	location: &indexer_job_location::Data,
	path: impl AsRef<Path>,
	library_ctx: &LibraryContext,
) -> Result<Option<file_path_with_object::Data>, FilePathError> {
	let mut maybe_file_path =
		get_existing_file_path(location, path.as_ref(), false, library_ctx).await?;
	// First we just check if this path was a file in our db, if it isn't then we check for a directory
	if maybe_file_path.is_none() {
		maybe_file_path =
			get_existing_file_path(location, path.as_ref(), true, library_ctx).await?;
	}

	Ok(maybe_file_path)
}

pub async fn get_parent_dir(
	location_id: LocationId,
	path: impl AsRef<Path>,
	library_ctx: &LibraryContext,
) -> Result<Option<file_path::Data>, FilePathError> {
	let mut parent_path_str = path
		.as_ref()
		.parent()
		// We have an "/" `materialized_path` for each location_id
		.unwrap_or_else(|| Path::new("/"))
		.to_str()
		.expect("Found non-UTF-8 path")
		.to_string();

	// As we're looking specifically for a parent directory, it must end with '/'
	if !parent_path_str.ends_with('/') {
		parent_path_str += "/";
	}

	library_ctx
		.db
		.file_path()
		.find_first(vec![
			file_path::location_id::equals(location_id),
			file_path::materialized_path::equals(parent_path_str),
		])
		.exec()
		.await
		.map_err(Into::into)
}
