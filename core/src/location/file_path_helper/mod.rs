use crate::{
	prisma::{file_path, location, PrismaClient},
	util::error::{FileIOError, NonUtf8PathError},
};

use std::{
	fs::Metadata,
	path::{Path, PathBuf, MAIN_SEPARATOR_STR},
	time::SystemTime,
};

use chrono::{DateTime, Utc};
use prisma_client_rust::QueryError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use tracing::error;

pub mod isolated_file_path_data;

pub use isolated_file_path_data::{
	join_location_relative_path, push_location_relative_path, IsolatedFilePathData,
};

// File Path selectables!
file_path::select!(file_path_pub_and_cas_ids { pub_id cas_id });
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
file_path::select!(file_path_to_isolate_with_id {
	id
	location_id
	materialized_path
	is_dir
	name
	extension
});
file_path::select!(file_path_walker {
	pub_id
	location_id
	materialized_path
	is_dir
	name
	extension
	date_modified
	inode
	device
});
file_path::select!(file_path_to_handle_custom_uri {
	materialized_path
	is_dir
	name
	extension
	location: select {
		id
		path
	}
});
file_path::select!(file_path_to_full_path {
	id
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
}

#[cfg(feature = "location-watcher")]
pub async fn create_file_path(
	crate::location::LoadedLibrary { db, sync, .. }: &crate::location::LoadedLibrary,
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
	use crate::util::db::{device_to_db, inode_to_db};

	use sd_prisma::{prisma, prisma_sync};
	use sd_sync::OperationFactory;
	use serde_json::json;
	use uuid::Uuid;

	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.select(location::select!({ id pub_id }))
		.exec()
		.await?
		.ok_or(FilePathError::LocationNotFound(location_id))?;

	let params = {
		use file_path::*;

		vec![
			(
				location::NAME,
				json!(prisma_sync::location::SyncId {
					pub_id: location.pub_id
				}),
			),
			(cas_id::NAME, json!(cas_id)),
			(materialized_path::NAME, json!(materialized_path)),
			(name::NAME, json!(name)),
			(extension::NAME, json!(extension)),
			(
				size_in_bytes_bytes::NAME,
				json!(metadata.size_in_bytes.to_be_bytes().to_vec()),
			),
			(inode::NAME, json!(metadata.inode.to_le_bytes())),
			(device::NAME, json!(metadata.device.to_le_bytes())),
			(is_dir::NAME, json!(is_dir)),
			(date_created::NAME, json!(metadata.created_at)),
			(date_modified::NAME, json!(metadata.modified_at)),
		]
	};

	let pub_id = sd_utils::uuid_to_bytes(Uuid::new_v4());

	let created_path = sync
		.write_ops(
			db,
			(
				sync.shared_create(
					prisma_sync::file_path::SyncId {
						pub_id: pub_id.clone(),
					},
					params,
				),
				db.file_path().create(pub_id, {
					use file_path::*;
					vec![
						location::connect(prisma::location::id::equals(location.id)),
						materialized_path::set(Some(materialized_path.into_owned())),
						name::set(Some(name.into_owned())),
						extension::set(Some(extension.into_owned())),
						inode::set(Some(inode_to_db(metadata.inode))),
						device::set(Some(device_to_db(metadata.device))),
						cas_id::set(cas_id),
						is_dir::set(Some(is_dir)),
						size_in_bytes_bytes::set(Some(
							metadata.size_in_bytes.to_be_bytes().to_vec(),
						)),
						date_created::set(Some(metadata.created_at.into())),
						date_modified::set(Some(metadata.modified_at.into())),
					]
				}),
			),
		)
		.await?;

	Ok(created_path)
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
		file_path::location_id::equals(Some(*location_id)),
		file_path::materialized_path::equals(Some(materialized_path.to_string())),
		file_path::is_dir::equals(Some(*is_dir)),
		file_path::name::equals(Some(name.to_string())),
		file_path::extension::equals(Some(extension.to_string())),
	]
}

/// With this function we try to do a loose filtering of file paths, to avoid having to do check
/// twice for directories and for files. This is because directories have a trailing `/` or `\` in
/// the materialized path
#[allow(unused)]
pub fn loose_find_existing_file_path_params(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	full_path: impl AsRef<Path>,
) -> Result<Vec<file_path::WhereParam>, FilePathError> {
	let location_path = location_path.as_ref();
	let full_path = full_path.as_ref();

	let file_iso_file_path =
		IsolatedFilePathData::new(location_id, location_path, full_path, false)?;

	let dir_iso_file_path = IsolatedFilePathData::new(location_id, location_path, full_path, true)?;

	Ok(vec![
		file_path::location_id::equals(Some(location_id)),
		file_path::materialized_path::equals(Some(
			file_iso_file_path.materialized_path.to_string(),
		)),
		file_path::name::in_vec(vec![
			file_iso_file_path.name.to_string(),
			dir_iso_file_path.name.to_string(),
		]),
		file_path::extension::in_vec(vec![
			file_iso_file_path.extension.to_string(),
			dir_iso_file_path.extension.to_string(),
		]),
	])
}

pub async fn ensure_sub_path_is_in_location(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<PathBuf, FilePathError> {
	let mut sub_path = sub_path.as_ref();
	let location_path = location_path.as_ref();
	if sub_path.starts_with(MAIN_SEPARATOR_STR) {
		if sub_path == Path::new(MAIN_SEPARATOR_STR) {
			// We're dealing with the location root path here
			return Ok(location_path.to_path_buf());
		}
		// SAFETY: we just checked that it starts with the separator
		sub_path = sub_path
			.strip_prefix(MAIN_SEPARATOR_STR)
			.expect("we just checked that it starts with the separator");
	}

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
	Ok(iso_file_path.is_root()
		|| db
			.file_path()
			.count(filter_existing_file_path_params(iso_file_path))
			.exec()
			.await? > 0)
}

pub async fn ensure_sub_path_is_directory(
	location_path: impl AsRef<Path>,
	sub_path: impl AsRef<Path>,
) -> Result<(), FilePathError> {
	let mut sub_path = sub_path.as_ref();

	if sub_path == Path::new(MAIN_SEPARATOR_STR) {
		// Sub path for the location root path is always a directory
		return Ok(());
	}

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
				sub_path = sub_path
					.strip_prefix("/")
					.expect("we just checked that it starts with the separator");
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
