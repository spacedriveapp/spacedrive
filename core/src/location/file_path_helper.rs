use crate::prisma::{
	file_path::{self, FindMany},
	location, PrismaClient,
};

use std::{
	fmt::{Display, Formatter},
	path::{Path, PathBuf},
};

use dashmap::{mapref::entry::Entry, DashMap};
use futures::future::try_join_all;
use prisma_client_rust::{Direction, QueryError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use tracing::error;

use super::LocationId;

// File Path selectables!
file_path::select!(file_path_just_id_materialized_path {
	id
	materialized_path
});
file_path::select!(file_path_for_file_identifier {
	id
	materialized_path
	date_created
});
file_path::select!(file_path_just_object_id { object_id });
file_path::select!(file_path_for_object_validator {
	id
	materialized_path
	integrity_checksum
	location: select {
		id
		pub_id
	}
});
file_path::select!(file_path_just_materialized_path_cas_id {
	materialized_path
	cas_id
});

// File Path includes!
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
			name: Self::prepare_name(full_path),
			extension,
		})
	}

	pub fn location_id(&self) -> LocationId {
		self.location_id
	}

	fn prepare_name(path: &Path) -> String {
		// Not using `impl AsRef<Path>` here because it's an private method
		path.file_name()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default()
			.to_string()
	}

	pub fn parent(&self) -> Self {
		let parent_path = Path::new(&self.materialized_path)
			.parent()
			.unwrap_or_else(|| Path::new("/"));

		let mut parent_path_str = parent_path
			.to_str()
			.unwrap() // SAFETY: This unwrap is ok because this path was a valid UTF-8 String before
			.to_string();

		if !parent_path_str.ends_with('/') {
			parent_path_str += "/";
		}

		Self {
			materialized_path: parent_path_str,
			is_dir: true,
			location_id: self.location_id,
			// NOTE: This way we don't use the same name for "/" `file_path`, that uses the location
			// name in the database, check later if this is a problem
			name: Self::prepare_name(parent_path),
			extension: String::new(),
		}
	}
}

impl From<MaterializedPath> for String {
	fn from(path: MaterializedPath) -> Self {
		path.materialized_path
	}
}

impl From<&MaterializedPath> for String {
	fn from(path: &MaterializedPath) -> Self {
		path.materialized_path.clone()
	}
}

impl AsRef<str> for MaterializedPath {
	fn as_ref(&self) -> &str {
		self.materialized_path.as_ref()
	}
}

impl AsRef<Path> for MaterializedPath {
	fn as_ref(&self) -> &Path {
		Path::new(&self.materialized_path)
	}
}

impl Display for MaterializedPath {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.materialized_path)
	}
}

#[derive(Error, Debug)]
pub enum FilePathError {
	#[error("Received an invalid sub path: <location_path={location_path}, sub_path={sub_path}>")]
	InvalidSubPath {
		location_path: PathBuf,
		sub_path: PathBuf,
	},
	#[error("Sub path is not a directory: {0}")]
	SubPathNotDirectory(PathBuf),
	#[error("The parent directory of the received sub path isn't indexed in the location: <id={location_id}, sub_path={sub_path}>")]
	SubPathParentNotInLocation {
		location_id: LocationId,
		sub_path: PathBuf,
	},
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
			.select(file_path::select!({ id }))
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

pub async fn find_many_file_paths_by_full_path<'db>(
	location: &location::Data,
	full_paths: &[impl AsRef<Path>],
	db: &'db PrismaClient,
) -> Result<FindMany<'db>, FilePathError> {
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

	Ok(db.file_path().find_many(vec![
		file_path::location_id::equals(location.id),
		file_path::materialized_path::in_vec(materialized_paths),
	]))
}

pub async fn get_existing_file_path_id(
	materialized_path: MaterializedPath,
	db: &PrismaClient,
) -> Result<Option<i32>, FilePathError> {
	db.file_path()
		.find_first(vec![
			file_path::location_id::equals(materialized_path.location_id),
			file_path::materialized_path::equals(materialized_path.into()),
		])
		.select(file_path::select!({ id }))
		.exec()
		.await
		.map_or_else(|e| Err(e.into()), |r| Ok(r.map(|r| r.id)))
}

#[cfg(feature = "location-watcher")]
pub async fn get_existing_file_path(
	materialized_path: MaterializedPath,
	db: &PrismaClient,
) -> Result<Option<file_path::Data>, FilePathError> {
	db.file_path()
		.find_first(vec![
			file_path::location_id::equals(materialized_path.location_id),
			file_path::materialized_path::equals(materialized_path.into()),
		])
		.exec()
		.await
		.map_err(Into::into)
}

#[cfg(feature = "location-watcher")]
pub async fn get_existing_file_path_with_object(
	materialized_path: MaterializedPath,
	db: &PrismaClient,
) -> Result<Option<file_path_with_object::Data>, FilePathError> {
	db.file_path()
		.find_first(vec![
			file_path::location_id::equals(materialized_path.location_id),
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
	location: &super::location_with_indexer_rules::Data,
	path: impl AsRef<Path>,
	db: &PrismaClient,
) -> Result<Option<file_path_with_object::Data>, FilePathError> {
	let mut maybe_file_path = get_existing_file_path_with_object(
		MaterializedPath::new(location.id, &location.path, path.as_ref(), false)?,
		db,
	)
	.await?;
	// First we just check if this path was a file in our db, if it isn't then we check for a directory
	if maybe_file_path.is_none() {
		maybe_file_path = get_existing_file_path_with_object(
			MaterializedPath::new(location.id, &location.path, path.as_ref(), true)?,
			db,
		)
		.await?;
	}

	Ok(maybe_file_path)
}

#[cfg(feature = "location-watcher")]
pub async fn get_parent_dir(
	materialized_path: &MaterializedPath,
	db: &PrismaClient,
) -> Result<Option<file_path::Data>, FilePathError> {
	get_existing_file_path(materialized_path.parent(), db).await
}

#[cfg(feature = "location-watcher")]
pub async fn get_parent_dir_id(
	materialized_path: &MaterializedPath,
	db: &PrismaClient,
) -> Result<Option<i32>, FilePathError> {
	get_existing_file_path_id(materialized_path.parent(), db).await
}

pub async fn ensure_sub_path_is_in_location(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	let sub_path = sub_path.as_ref();
	let location_path = location_path.as_ref();

	if !sub_path.starts_with(location_path) {
		// If the sub_path doesn't start with the location_path, we have to check if it's a
		// materialized path received from the frontend, then we check if the full path exists
		let full_path = location_path.join(sub_path);
		match fs::metadata(&full_path).await {
			Ok(_) => Ok(full_path),
			Err(e) if e.kind() == io::ErrorKind::NotFound => Err(FilePathError::InvalidSubPath {
				sub_path: sub_path.to_path_buf(),
				location_path: location_path.to_path_buf(),
			}),
			Err(e) => Err(e.into()),
		}
	} else {
		Ok(sub_path.to_path_buf())
	}
}

pub async fn ensure_sub_path_is_directory(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<(), FilePathError> {
	let sub_path = sub_path.as_ref();
	let location_path = location_path.as_ref();

	match fs::metadata(sub_path).await {
		Ok(meta) => {
			if meta.is_file() {
				Err(FilePathError::SubPathNotDirectory(sub_path.to_path_buf()))
			} else {
				Ok(())
			}
		}
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			match fs::metadata(location_path.join(sub_path)).await {
				Ok(meta) => {
					if meta.is_file() {
						Err(FilePathError::SubPathNotDirectory(sub_path.to_path_buf()))
					} else {
						Ok(())
					}
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					Err(FilePathError::InvalidSubPath {
						sub_path: sub_path.to_path_buf(),
						location_path: location_path.to_path_buf(),
					})
				}
				Err(e) => Err(e.into()),
			}
		}
		Err(e) => Err(e.into()),
	}
}
