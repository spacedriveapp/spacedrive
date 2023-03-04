use crate::prisma::{file_path, PrismaClient};

use std::path::{Path, PathBuf};

use dashmap::{mapref::entry::Entry, DashMap};
use futures::future::try_join_all;
use prisma_client_rust::{Direction, QueryError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use tracing::error;

use super::{indexer::indexer_job_location, LocationId};

file_path::select!(file_path_id_only { id });
file_path::include!(file_path_with_object { object });

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MaterializedPath {
	pub(super) materialized_path: String,
	pub(super) is_dir: bool,
	pub(super) location_id: LocationId,
	pub(super) name: String,
	pub(super) extension: String,
}

impl MaterializedPath {
	pub fn new(
		location_id: LocationId,
		location_path: impl AsRef<Path>,
		full_path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<Self, FilePathError> {
		let full_path = full_path.as_ref();
		let mut materialized_path =
			extract_materialized_path(location_id, location_path, full_path)?
				.to_str()
				.expect("Found non-UTF-8 path")
				.to_string();

		if is_dir && !materialized_path.ends_with('/') {
			materialized_path += "/";
		}

		let extension = if !is_dir {
			let extension = full_path
				.extension()
				.unwrap_or_default()
				.to_str()
				.unwrap_or_default();

			#[cfg(debug_assertions)]
			{
				// In dev mode, we lowercase the extension as we don't use the SQL migration,
				// and using prisma.schema directly we can't set `COLLATE NOCASE` in the
				// `extension` column at `file_path` table
				extension.to_lowercase()
			}
			#[cfg(not(debug_assertions))]
			{
				extension.to_string()
			}
		} else {
			String::new()
		};

		Ok(Self {
			materialized_path,
			is_dir,
			location_id,
			name: full_path
				.file_name()
				.unwrap_or_default()
				.to_str()
				.unwrap_or_default()
				.to_string(),
			extension,
		})
	}
}

impl From<MaterializedPath> for String {
	fn from(path: MaterializedPath) -> Self {
		path.materialized_path
	}
}

impl AsRef<str> for MaterializedPath {
	fn as_ref(&self) -> &str {
		self.materialized_path.as_ref()
	}
}

#[derive(Error, Debug)]
pub enum FilePathError {
	#[error("Unable to extract materialized path from location: <id='{0}', path='{1:?}'>")]
	UnableToExtractMaterializedPath(LocationId, PathBuf),
	#[error("Database error (error: {0:?})")]
	DatabaseError(#[from] QueryError),
	#[error("Database error (error: {0:?})")]
	IOError(#[from] io::Error),
}

#[derive(Debug)]
pub struct LastFilePathIdManager {
	last_id_by_location: DashMap<LocationId, i32>,
}

impl Default for LastFilePathIdManager {
	fn default() -> Self {
		Self {
			last_id_by_location: DashMap::with_capacity(4),
		}
	}
}

impl LastFilePathIdManager {
	pub fn new() -> Self {
		Default::default()
	}

	pub async fn get_max_file_path_id(
		&self,
		location_id: LocationId,
		db: &PrismaClient,
	) -> Result<i32, FilePathError> {
		Ok(match self.last_id_by_location.entry(location_id) {
			Entry::Occupied(entry) => *entry.get(),
			Entry::Vacant(entry) => {
				// I wish I could use `or_try_insert_with` method instead of this crappy match,
				// but we don't have async closures yet ):
				let id = Self::fetch_max_file_path_id(location_id, db).await?;
				entry.insert(id);
				id
			}
		})
	}

	pub async fn set_max_file_path_id(&self, location_id: LocationId, id: i32) {
		self.last_id_by_location.insert(location_id, id);
	}

	async fn fetch_max_file_path_id(
		location_id: LocationId,
		db: &PrismaClient,
	) -> Result<i32, FilePathError> {
		Ok(db
			.file_path()
			.find_first(vec![file_path::location_id::equals(location_id)])
			.order_by(file_path::id::order(Direction::Desc))
			.select(file_path_id_only::select())
			.exec()
			.await?
			.map(|r| r.id)
			.unwrap_or(0))
	}

	#[cfg(feature = "location-watcher")]
	pub async fn create_file_path(
		&self,
		db: &PrismaClient,
		MaterializedPath {
			materialized_path,
			is_dir,
			location_id,
			name,
			extension,
		}: MaterializedPath,
		parent_id: Option<i32>,
	) -> Result<file_path::Data, FilePathError> {
		use crate::prisma::location;

		// Keeping a reference in that map for the entire duration of the function, so we keep it locked
		let mut last_id_ref = match self.last_id_by_location.entry(location_id) {
			Entry::Occupied(ocupied) => ocupied.into_ref(),
			Entry::Vacant(vacant) => {
				let id = Self::fetch_max_file_path_id(location_id, db).await?;
				vacant.insert(id)
			}
		};

		let next_id = *last_id_ref + 1;

		let created_path = db
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

		*last_id_ref = next_id;

		Ok(created_path)
	}
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
	location_id: LocationId,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	subtract_location_path(location_path, &path).ok_or_else(|| {
		FilePathError::UnableToExtractMaterializedPath(location_id, path.as_ref().to_path_buf())
	})
}

pub async fn get_many_file_paths_by_full_path(
	location: &indexer_job_location::Data,
	full_paths: &[impl AsRef<Path>],
	db: &PrismaClient,
) -> Result<Vec<file_path::Data>, FilePathError> {
	let is_dirs = try_join_all(
		full_paths
			.iter()
			.map(|path| async move { fs::metadata(path).await.map(|metadata| metadata.is_dir()) }),
	)
	.await?;

	let materialized_paths = full_paths
		.iter()
		.zip(is_dirs.into_iter())
		.map(|(path, is_dir)| {
			MaterializedPath::new(location.id, &location.path, path, is_dir).map(Into::into)
		})
		// Collecting in a Result, so we stop on the first error
		.collect::<Result<Vec<_>, _>>()?;

	db.file_path()
		.find_many(vec![file_path::materialized_path::in_vec(
			materialized_paths,
		)])
		.exec()
		.await
		.map_err(Into::into)
}

pub async fn get_existing_file_path(
	location_id: LocationId,
	materialized_path: MaterializedPath,
	db: &PrismaClient,
) -> Result<Option<file_path_with_object::Data>, FilePathError> {
	db.file_path()
		.find_first(vec![
			file_path::location_id::equals(location_id),
			file_path::materialized_path::equals(materialized_path.into()),
		])
		// include object for orphan check
		.include(file_path_with_object::include())
		.exec()
		.await
		.map_err(Into::into)
}

#[cfg(feature = "location-watcher")]
pub async fn get_existing_file_or_directory(
	location: &indexer_job_location::Data,
	path: impl AsRef<Path>,
	db: &PrismaClient,
) -> Result<Option<file_path_with_object::Data>, FilePathError> {
	let mut maybe_file_path = get_existing_file_path(
		location.id,
		MaterializedPath::new(location.id, &location.path, path.as_ref(), false)?,
		db,
	)
	.await?;
	// First we just check if this path was a file in our db, if it isn't then we check for a directory
	if maybe_file_path.is_none() {
		maybe_file_path = get_existing_file_path(
			location.id,
			MaterializedPath::new(location.id, &location.path, path.as_ref(), true)?,
			db,
		)
		.await?;
	}

	Ok(maybe_file_path)
}

pub async fn get_parent_dir(
	location_id: LocationId,
	path: impl AsRef<Path>,
	db: &PrismaClient,
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

	db.file_path()
		.find_first(vec![
			file_path::location_id::equals(location_id),
			file_path::materialized_path::equals(parent_path_str),
		])
		.exec()
		.await
		.map_err(Into::into)
}
