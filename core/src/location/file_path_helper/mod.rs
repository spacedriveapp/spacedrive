use crate::{
	prisma::{file_path, PrismaClient},
	util::error::{FileIOError, NonUtf8PathError},
};

use std::{
	fs::Metadata,
	path::{Path, PathBuf},
	time::SystemTime,
};

use chrono::{DateTime, Utc};
use prisma_client_rust::QueryError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use tracing::error;

pub mod isolated_file_path_data;

pub use isolated_file_path_data::IsolatedFilePathData;

use super::LocationId;

// File Path selectables!
file_path::select!(file_path_just_pub_id { pub_id });
file_path::select!(file_path_just_pub_id_materialized_path {
	pub_id
	materialized_path
});
file_path::select!(file_path_for_file_identifier {
	id
	pub_id
	materialized_path
	date_created
	is_dir
	name
	extension
});
file_path::select!(file_path_for_object_validator {
	pub_id
	materialized_path
	is_dir
	name
	extension
	integrity_checksum
	location: select {
		id
		pub_id
	}
});
file_path::select!(file_path_for_thumbnailer {
	materialized_path
	is_dir
	name
	extension
	cas_id
});
file_path::select!(file_path_to_isolate {
	location_id
	materialized_path
	is_dir
	name
	extension
});
file_path::select!(file_path_to_handle_custom_uri {
	materialized_path
	is_dir
	name
	extension
	location: select {
		path
	}
});
file_path::select!(file_path_to_full_path {
	materialized_path
	is_dir
	name
	extension
	location: select {
		id
		path
	}
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

#[derive(Error, Debug)]
pub enum FilePathError {
	#[error("file Path not found: <path='{}'>", .0.display())]
	NotFound(Box<Path>),
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
		location_id: LocationId,
		sub_path: Box<Path>,
	},
	#[error("unable to extract materialized path from location: <id='{}', path='{}'>", .location_id, .path.display())]
	UnableToExtractMaterializedPath {
		location_id: LocationId,
		path: Box<Path>,
	},
	#[error("database error")]
	Database(#[from] QueryError),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
}

#[cfg(feature = "location-watcher")]
pub async fn create_file_path(
	crate::location::Library { db, sync, .. }: &crate::location::Library,
	IsolatedFilePathData {
		materialized_path,
		is_dir,
		location_id,
		name,
		extension,
		..
	}: IsolatedFilePathData<'_>,
	cas_id: Option<String>,
	metadata: FilePathMetadata,
) -> Result<file_path::Data, FilePathError> {
	use crate::{prisma::location, sync, util::db::uuid_to_bytes};

	use serde_json::json;
	use uuid::Uuid;

	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.select(location::select!({ id pub_id }))
		.exec()
		.await?
		.unwrap();

	let params = {
		use file_path::*;

		vec![
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
		]
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

#[cfg(feature = "location-watcher")]
pub async fn check_existing_file_path(
	materialized_path: &IsolatedFilePathData<'_>,
	db: &PrismaClient,
) -> Result<bool, FilePathError> {
	Ok(db
		.file_path()
		.count(filter_existing_file_path_params(materialized_path))
		.exec()
		.await? > 0)
}

pub fn filter_existing_file_path_params(
	IsolatedFilePathData {
		materialized_path,
		is_dir,
		location_id,
		name,
		extension,
		..
	}: &IsolatedFilePathData,
) -> Vec<file_path::WhereParam> {
	vec![
		file_path::location_id::equals(*location_id),
		file_path::materialized_path::equals(materialized_path.to_string()),
		file_path::is_dir::equals(*is_dir),
		file_path::name::equals(name.to_string()),
		file_path::extension::equals(extension.to_string()),
	]
}

/// With this function we try to do a loose filtering of file paths, to avoid having to do check
/// twice for directories and for files. This is because directories have a trailing `/` or `\` in
/// the materialized path
#[allow(unused)]
pub fn loose_find_existing_file_path_params(
	IsolatedFilePathData {
		materialized_path,
		location_id,
		name,
		extension,
		..
	}: &IsolatedFilePathData,
) -> Vec<file_path::WhereParam> {
	vec![
		file_path::location_id::equals(*location_id),
		file_path::materialized_path::equals(materialized_path.to_string()),
		file_path::name::equals(name.to_string()),
		file_path::extension::equals(extension.to_string()),
	]
}

#[cfg(feature = "location-watcher")]
pub async fn get_parent_dir(
	materialized_path: &IsolatedFilePathData<'_>,
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

pub async fn ensure_sub_path_is_in_location(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	let mut sub_path = sub_path.as_ref();
	if sub_path.starts_with("/") {
		// SAFETY: we just checked that it starts with the separator
		sub_path = sub_path.strip_prefix("/").unwrap();
	}
	let location_path = location_path.as_ref();

	if !sub_path.starts_with(location_path) {
		// If the sub_path doesn't start with the location_path, we have to check if it's a
		// materialized path received from the frontend, then we check if the full path exists
		let full_path = location_path.join(sub_path);

		match fs::metadata(&full_path).await {
			Ok(_) => Ok(full_path),
			Err(e) if e.kind() == io::ErrorKind::NotFound => Err(FilePathError::InvalidSubPath {
				sub_path: sub_path.into(),
				location_path: location_path.into(),
			}),
			Err(e) => Err(FileIOError::from((full_path, e)).into()),
		}
	} else {
		Ok(sub_path.to_path_buf())
	}
}

pub async fn ensure_file_path_exists<E>(
	sub_path: impl AsRef<Path>,
	iso_file_path: &IsolatedFilePathData<'_>,
	db: &PrismaClient,
	error_fn: impl FnOnce(Box<Path>) -> E,
) -> Result<(), E>
where
	E: From<QueryError>,
{
	if !check_file_path_exists(iso_file_path, db).await? {
		Err(error_fn(sub_path.as_ref().into()))
	} else {
		Ok(())
	}
}

pub async fn check_file_path_exists<E>(
	iso_file_path: &IsolatedFilePathData<'_>,
	db: &PrismaClient,
) -> Result<bool, E>
where
	E: From<QueryError>,
{
	db.file_path()
		.count(filter_existing_file_path_params(iso_file_path))
		.exec()
		.await
		.map(|count| count > 0)
		.map_err(Into::into)
}

pub async fn ensure_sub_path_is_directory(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<(), FilePathError> {
	let mut sub_path = sub_path.as_ref();

	match fs::metadata(sub_path).await {
		Ok(meta) => {
			if meta.is_file() {
				Err(FilePathError::SubPathNotDirectory(sub_path.into()))
			} else {
				Ok(())
			}
		}
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			if sub_path.starts_with("/") {
				// SAFETY: we just checked that it starts with the separator
				sub_path = sub_path.strip_prefix("/").unwrap();
			}

			let location_path = location_path.as_ref();
			let full_path = location_path.join(sub_path);
			match fs::metadata(&full_path).await {
				Ok(meta) => {
					if meta.is_file() {
						Err(FilePathError::SubPathNotDirectory(sub_path.into()))
					} else {
						Ok(())
					}
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					Err(FilePathError::InvalidSubPath {
						sub_path: sub_path.into(),
						location_path: location_path.into(),
					})
				}
				Err(e) => Err(FileIOError::from((full_path, e)).into()),
			}
		}
		Err(e) => Err(FileIOError::from((sub_path, e)).into()),
	}
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
		let metadata = fs::metadata(path.as_ref())
			.await
			.map_err(|e| FileIOError::from((path, e)))?;

		get_inode_and_device(&metadata)
	}

	#[cfg(target_family = "windows")]
	{
		use winapi_util::{file::information, Handle};

		let info = Handle::from_path_any(path.as_ref())
			.and_then(|ref handle| information(handle))
			.map_err(|e| FileIOError::from((path, e)))?;

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
