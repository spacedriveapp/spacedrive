use crate::{
	prisma::{file_path, location, PrismaClient},
	util::db::uuid_to_bytes,
};

use std::{
	borrow::Cow,
	fmt::{Display, Formatter},
	fs::Metadata,
	path::{Path, PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR},
	time::SystemTime,
};

use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use prisma_client_rust::QueryError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use tracing::error;
use uuid::Uuid;

use super::LocationId;

// File Path selectables!
file_path::select!(file_path_just_id_materialized_path {
	pub_id
	materialized_path
});
file_path::select!(file_path_for_file_identifier {
	id
	pub_id
	materialized_path
	date_created
});
file_path::select!(file_path_for_object_validator {
	pub_id
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FilePathMetadata {
	pub inode: u64,
	pub device: u64,
	pub size_in_bytes: u64,
	pub created_at: DateTime<Utc>,
	pub modified_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MaterializedPath<'a> {
	pub(super) materialized_path: Cow<'a, str>,
	pub(super) is_dir: bool,
	pub(super) location_id: LocationId,
	pub(super) name: Cow<'a, str>,
	pub(super) extension: Cow<'a, str>,
}

impl MaterializedPath<'static> {
	pub fn new(
		location_id: LocationId,
		location_path: impl AsRef<Path>,
		full_path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<Self, FilePathError> {
		let full_path = full_path.as_ref();
		let mut materialized_path = format!(
			"{MAIN_SEPARATOR_STR}{}",
			extract_materialized_path(location_id, location_path, full_path)?
				.to_str()
				.expect("Found non-UTF-8 path")
		);

		if is_dir && !materialized_path.ends_with(MAIN_SEPARATOR) {
			materialized_path += MAIN_SEPARATOR_STR;
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
			materialized_path: Cow::Owned(materialized_path),
			is_dir,
			location_id,
			name: Cow::Owned(Self::prepare_name(full_path).to_string()),
			extension: Cow::Owned(extension),
		})
	}
}

impl<'a> MaterializedPath<'a> {
	pub fn location_id(&self) -> LocationId {
		self.location_id
	}

	fn prepare_name(path: &Path) -> &str {
		// Not using `impl AsRef<Path>` here because it's an private method
		path.file_stem()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default()
	}

	pub fn parent(&self) -> Self {
		let parent_path = Path::new(self.materialized_path.as_ref())
			.parent()
			.unwrap_or_else(|| Path::new(MAIN_SEPARATOR_STR));

		let mut parent_path_str = parent_path
			.to_str()
			.unwrap() // SAFETY: This unwrap is ok because this path was a valid UTF-8 String before
			.to_string();

		if !parent_path_str.ends_with(MAIN_SEPARATOR) {
			parent_path_str += MAIN_SEPARATOR_STR;
		}

		Self {
			materialized_path: Cow::Owned(parent_path_str),
			is_dir: true,
			location_id: self.location_id,
			// NOTE: This way we don't use the same name for "/" `file_path`, that uses the location
			// name in the database, check later if this is a problem
			name: Cow::Owned(Self::prepare_name(parent_path).to_string()),
			extension: Cow::Owned(String::new()),
		}
	}
}

impl<'a, S: AsRef<str> + 'a> From<(LocationId, &'a S)> for MaterializedPath<'a> {
	fn from((location_id, materialized_path): (LocationId, &'a S)) -> Self {
		let materialized_path = materialized_path.as_ref();
		let is_dir = materialized_path.ends_with(MAIN_SEPARATOR);
		let length = materialized_path.len();

		let (name, extension) = if length == 1 {
			// The case for the root path
			(materialized_path, "")
		} else if is_dir {
			let first_name_char = materialized_path[..(length - 1)]
				.rfind(MAIN_SEPARATOR)
				.unwrap_or(0) + 1;
			(&materialized_path[first_name_char..(length - 1)], "")
		} else {
			let first_name_char = materialized_path.rfind(MAIN_SEPARATOR).unwrap_or(0) + 1;
			if let Some(last_dot_relative_idx) = materialized_path[first_name_char..].rfind('.') {
				let last_dot_idx = first_name_char + last_dot_relative_idx;
				(
					&materialized_path[first_name_char..last_dot_idx],
					&materialized_path[last_dot_idx + 1..],
				)
			} else {
				(&materialized_path[first_name_char..], "")
			}
		};

		Self {
			materialized_path: Cow::Borrowed(materialized_path),
			location_id,
			is_dir,
			name: Cow::Borrowed(name),
			extension: Cow::Borrowed(extension),
		}
	}
}

impl From<MaterializedPath<'_>> for String {
	fn from(path: MaterializedPath) -> Self {
		path.materialized_path.into_owned()
	}
}

impl From<&MaterializedPath<'_>> for String {
	fn from(path: &MaterializedPath) -> Self {
		path.materialized_path.to_string()
	}
}

impl AsRef<str> for MaterializedPath<'_> {
	fn as_ref(&self) -> &str {
		self.materialized_path.as_ref()
	}
}

impl AsRef<Path> for &MaterializedPath<'_> {
	fn as_ref(&self) -> &Path {
		// Skipping / because it's not a valid path to be joined
		Path::new(&self.materialized_path[1..])
	}
}

impl Display for MaterializedPath<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.materialized_path)
	}
}

#[derive(Error, Debug)]
pub enum FilePathError {
	#[error("File Path not found: <path={0}>")]
	NotFound(PathBuf),
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

#[cfg(feature = "location-watcher")]
pub async fn create_file_path(
	crate::location::Library { db, sync, .. }: &crate::location::Library,
	MaterializedPath {
		materialized_path,
		is_dir,
		location_id,
		name,
		extension,
	}: MaterializedPath<'_>,
	parent_id: Option<Uuid>,
	cas_id: Option<String>,
	metadata: FilePathMetadata,
) -> Result<file_path::Data, FilePathError> {
	// Keeping a reference in that map for the entire duration of the function, so we keep it locked

	use crate::{sync, util};
	use serde_json::json;

	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.select(location::select!({ id pub_id }))
		.exec()
		.await?
		.unwrap();

	let params = {
		use file_path::*;

		util::db::chain_optional_iter(
			[
				(
					location::NAME,
					json!(sync::location::SyncId {
						pub_id: location.pub_id
					}),
				),
				(cas_id::NAME, json!(cas_id)),
				(materialized_path::NAME, json!(materialized_path)),
				(name::NAME, json!(name)),
				(extension::NAME, json!(extension)),
				(
					size_in_bytes::NAME,
					json!(metadata.size_in_bytes.to_string()),
				),
				(inode::NAME, json!(metadata.inode.to_le_bytes())),
				(device::NAME, json!(metadata.device.to_le_bytes())),
				(is_dir::NAME, json!(is_dir)),
				(date_created::NAME, json!(metadata.created_at)),
				(date_modified::NAME, json!(metadata.modified_at)),
			],
			[parent_id.map(|parent_id| {
				(
					parent_id::NAME,
					json!(sync::file_path::SyncId {
						pub_id: uuid_to_bytes(parent_id)
					}),
				)
			})],
		)
	};

	let pub_id = uuid_to_bytes(Uuid::new_v4());

	let created_path = sync
		.write_op(
			db,
			sync.unique_shared_create(
				sync::file_path::SyncId {
					pub_id: pub_id.clone(),
				},
				params,
			),
			db.file_path().create(
				pub_id,
				location::id::equals(location.id),
				materialized_path.into_owned(),
				name.into_owned(),
				extension.into_owned(),
				metadata.inode.to_le_bytes().into(),
				metadata.device.to_le_bytes().into(),
				{
					use file_path::*;
					vec![
						cas_id::set(cas_id),
						parent_id::set(parent_id.map(uuid_to_bytes)),
						is_dir::set(is_dir),
						size_in_bytes::set(metadata.size_in_bytes.to_string()),
						date_created::set(metadata.created_at.into()),
						date_modified::set(metadata.modified_at.into()),
					]
				},
			),
		)
		.await?;

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
	location_id: LocationId,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	subtract_location_path(location_path, &path).ok_or_else(|| {
		FilePathError::UnableToExtractMaterializedPath(location_id, path.as_ref().to_path_buf())
	})
}

pub async fn filter_file_paths_by_many_full_path_params(
	location: &location::Data,
	full_paths: &[impl AsRef<Path>],
) -> Result<Vec<file_path::WhereParam>, FilePathError> {
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

	Ok(vec![
		file_path::location_id::equals(location.id),
		file_path::materialized_path::in_vec(materialized_paths),
	])
}

#[cfg(feature = "location-watcher")]
pub async fn check_existing_file_path(
	materialized_path: &MaterializedPath<'_>,
	db: &PrismaClient,
) -> Result<bool, FilePathError> {
	db.file_path()
		.count(filter_existing_file_path_params(materialized_path))
		.exec()
		.await
		.map_or_else(|e| Err(e.into()), |count| Ok(count > 0))
}

pub fn filter_existing_file_path_params(
	MaterializedPath {
		materialized_path,
		is_dir,
		location_id,
		name,
		extension,
	}: &MaterializedPath,
) -> Vec<file_path::WhereParam> {
	let mut params = vec![
		file_path::location_id::equals(*location_id),
		file_path::materialized_path::equals(materialized_path.to_string()),
		file_path::is_dir::equals(*is_dir),
		file_path::extension::equals(extension.to_string()),
	];

	// This is due to a limitation of MaterializedPath, where we don't know the location name to use
	// as the file_path name at the root of the location "/" or "\" on Windows
	if materialized_path != MAIN_SEPARATOR_STR {
		params.push(file_path::name::equals(name.to_string()));
	}

	params
}

/// With this function we try to do a loose filtering of file paths, to avoid having to do check
/// twice for directories and for files. This is because directories have a trailing `/` or `\` in
/// the materialized path
#[allow(unused)]
pub fn loose_find_existing_file_path_params(
	MaterializedPath {
		materialized_path,
		is_dir,
		location_id,
		name,
		..
	}: &MaterializedPath,
) -> Vec<file_path::WhereParam> {
	let mut materialized_path_str = materialized_path.to_string();
	if *is_dir {
		materialized_path_str.pop();
	}

	let mut params = vec![
		file_path::location_id::equals(*location_id),
		file_path::materialized_path::starts_with(materialized_path_str),
	];

	// This is due to a limitation of MaterializedPath, where we don't know the location name to use
	// as the file_path name at the root of the location "/" or "\" on Windows
	if materialized_path != MAIN_SEPARATOR_STR {
		params.push(file_path::name::equals(name.to_string()));
	}

	params
}

pub async fn get_existing_file_path_id(
	materialized_path: &MaterializedPath<'_>,
	db: &PrismaClient,
) -> Result<Option<Uuid>, FilePathError> {
	db.file_path()
		.find_first(filter_existing_file_path_params(materialized_path))
		.select(file_path::select!({ pub_id }))
		.exec()
		.await
		.map_or_else(
			|e| Err(e.into()),
			|r| Ok(r.map(|r| Uuid::from_slice(&r.pub_id).unwrap())),
		)
}

#[cfg(feature = "location-watcher")]
pub async fn get_parent_dir(
	materialized_path: &MaterializedPath<'_>,
	db: &PrismaClient,
) -> Result<Option<file_path::Data>, FilePathError> {
	db.file_path()
		.find_first(filter_existing_file_path_params(
			&materialized_path.parent(),
		))
		.exec()
		.await
		.map_err(Into::into)
}

#[cfg(feature = "location-watcher")]
pub async fn get_parent_dir_id(
	materialized_path: &MaterializedPath<'_>,
	db: &PrismaClient,
) -> Result<Option<Uuid>, FilePathError> {
	get_existing_file_path_id(&materialized_path.parent(), db).await
}

pub async fn ensure_sub_path_is_in_location(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	let mut sub_path = sub_path.as_ref();
	if sub_path.starts_with(MAIN_SEPARATOR_STR) {
		// SAFETY: we just checked that it starts with the separator
		sub_path = sub_path.strip_prefix(MAIN_SEPARATOR_STR).unwrap();
	}
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
	let mut sub_path = sub_path.as_ref();

	match fs::metadata(sub_path).await {
		Ok(meta) => {
			if meta.is_file() {
				Err(FilePathError::SubPathNotDirectory(sub_path.to_path_buf()))
			} else {
				Ok(())
			}
		}
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			if sub_path.starts_with(MAIN_SEPARATOR_STR) {
				// SAFETY: we just checked that it starts with the separator
				sub_path = sub_path.strip_prefix(MAIN_SEPARATOR_STR).unwrap();
			}

			let location_path = location_path.as_ref();

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

pub async fn retain_file_paths_in_location(
	location_id: LocationId,
	to_retain: Vec<Uuid>,
	maybe_parent_file_path: Option<file_path_just_id_materialized_path::Data>,
	db: &PrismaClient,
) -> Result<i64, FilePathError> {
	let mut to_delete_params = vec![
		file_path::location_id::equals(location_id),
		file_path::pub_id::not_in_vec(to_retain.into_iter().map(uuid_to_bytes).collect()),
	];

	if let Some(parent_file_path) = maybe_parent_file_path {
		// If the parent_materialized_path is not the root path, we only delete file paths that start with the parent path
		let param = if parent_file_path.materialized_path != MAIN_SEPARATOR_STR {
			file_path::materialized_path::starts_with(parent_file_path.materialized_path)
		} else {
			// If the parent_materialized_path is the root path, we fetch children using the parent id
			file_path::parent_id::equals(Some(parent_file_path.pub_id))
		};

		to_delete_params.push(param);
	}

	db.file_path()
		.delete_many(to_delete_params)
		.exec()
		.await
		.map_err(Into::into)
}

#[allow(unused)] // TODO remove this annotation when we can use it on windows
pub fn get_inode_and_device(metadata: &Metadata) -> Result<(u64, u64), FilePathError> {
	#[cfg(target_family = "unix")]
	{
		use std::os::unix::fs::MetadataExt;

		Ok((metadata.ino(), metadata.dev()))
	}

	#[cfg(target_family = "windows")]
	{
		// TODO use this when it's stable and remove winapi-utils dependency

		// use std::os::windows::fs::MetadataExt;

		// Ok((
		// 	metadata
		// 		.file_index()
		// 		.expect("This function must not be called from a `DirEntry`'s `Metadata"),
		// 	metadata
		// 		.volume_serial_number()
		// 		.expect("This function must not be called from a `DirEntry`'s `Metadata") as u64,
		// ))

		todo!("Use metadata: {:#?}", metadata)
	}
}

#[allow(unused)]
pub async fn get_inode_and_device_from_path(
	path: impl AsRef<Path>,
) -> Result<(u64, u64), FilePathError> {
	#[cfg(target_family = "unix")]
	{
		// TODO use this when it's stable and remove winapi-utils dependency
		let metadata = fs::metadata(path.as_ref()).await?;

		get_inode_and_device(&metadata)
	}

	#[cfg(target_family = "windows")]
	{
		use winapi_util::{file::information, Handle};

		let info = information(&Handle::from_path_any(path.as_ref())?)?;

		Ok((info.file_index(), info.volume_serial_number()))
	}
}

pub trait MetadataExt {
	fn created_or_now(&self) -> SystemTime;

	fn modified_or_now(&self) -> SystemTime;
}

impl MetadataExt for Metadata {
	fn created_or_now(&self) -> SystemTime {
		self.created().unwrap_or_else(|_| SystemTime::now())
	}

	fn modified_or_now(&self) -> SystemTime {
		self.modified().unwrap_or_else(|_| SystemTime::now())
	}
}
