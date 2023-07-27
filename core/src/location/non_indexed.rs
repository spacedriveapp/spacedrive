use crate::{
	api::locations::ExplorerItem,
	library::Library,
	object::{cas::generate_cas_id, preview::get_thumb_key},
	prisma::location,
	util::error::FileIOError,
};

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};

use sd_file_ext::{extensions::Extension, kind::ObjectKind};

use chrono::{DateTime, Utc};
use rspc::ErrorCode;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use tokio::{fs, io};
use tracing::{error, warn};

use super::{file_path_helper::MetadataExt, generate_thumbnail, normalize_path};

#[derive(Debug, Error)]
pub enum NonIndexedLocationError {
	#[error("path not found: {}", .0.display())]
	NotFound(PathBuf),

	#[error(transparent)]
	FileIO(#[from] FileIOError),

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
}

impl From<NonIndexedLocationError> for rspc::Error {
	fn from(err: NonIndexedLocationError) -> Self {
		match err {
			NonIndexedLocationError::NotFound(_) => {
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
pub struct NonIndexedPathItem {
	id: i32,
	path: String,
	name: String,
	extension: String,
	kind: i32,
	is_dir: bool,
	date_created: DateTime<Utc>,
	date_modified: DateTime<Utc>,
	size_in_bytes_bytes: Vec<u8>,
}

pub async fn walk(
	full_path: impl AsRef<Path>,
	library: Library,
) -> Result<NonIndexedFileSystemEntries, NonIndexedLocationError> {
	let path = full_path.as_ref();
	let mut read_dir = fs::read_dir(path).await.map_err(|e| (path, e))?;

	let mut id = 1;
	let mut directories = vec![];
	let mut errors = vec![];
	let mut entries = vec![];

	while let Some(entry) = read_dir.next_entry().await.map_err(|e| (path, e))? {
		let Ok((entry_path, name)) = normalize_path(entry.path())
			.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into())) else {
			continue;
		};

		let Ok(metadata) = entry.metadata()
			.await
			.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into()))
			else {
			continue;
		};

		if metadata.is_dir() {
			directories.push((entry_path, id, name, metadata));
		} else {
			let path = Path::new(&entry_path);

			let Some(name) = path.file_stem()
				.and_then(|s| s.to_str().map(str::to_string))
				else {
				warn!("Failed to extract name from path: {}", &entry_path);
				continue;
			};

			let extension = path
				.extension()
				.and_then(|s| s.to_str().map(str::to_string))
				.unwrap_or("".to_string());

			let kind = Extension::resolve_conflicting(&path, false)
				.await
				.map(Into::into)
				.unwrap_or(ObjectKind::Unknown);

			let thumbnail_key = if matches!(kind, ObjectKind::Image | ObjectKind::Video) {
				if let Ok(cas_id) = generate_cas_id(&entry_path, metadata.len())
					.await
					.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into()))
				{
					let thumbnail_key = get_thumb_key(&cas_id);
					let entry_path = entry_path.clone();
					let extension = extension.clone();
					let library = library.clone();
					tokio::spawn(async move {
						generate_thumbnail(&extension, &cas_id, entry_path, &library).await;
					});

					Some(thumbnail_key)
				} else {
					None
				}
			} else {
				None
			};

			entries.push(ExplorerItem::NonIndexedPath {
				has_local_thumbnail: thumbnail_key.is_some(),
				thumbnail_key,
				item: NonIndexedPathItem {
					id,
					path: entry_path,
					name,
					extension,
					kind: kind as i32,
					is_dir: false,
					date_created: metadata.created_or_now().into(),
					date_modified: metadata.modified_or_now().into(),
					size_in_bytes_bytes: metadata.len().to_be_bytes().to_vec(),
				},
			});
		}

		id += 1;
	}

	let mut locations = library
		.db
		.location()
		.find_many(vec![location::path::in_vec(
			directories
				.iter()
				.map(|(path, _, _, _)| path.clone())
				.collect(),
		)])
		.exec()
		.await?
		.into_iter()
		.flat_map(|location| {
			location
				.path
				.clone()
				.map(|location_path| (location_path, location))
		})
		.collect::<HashMap<_, _>>();

	for (directory, id, name, metadata) in directories {
		if let Some(location) = locations.remove(&directory) {
			entries.push(ExplorerItem::Location {
				has_local_thumbnail: false,
				thumbnail_key: None,
				item: location,
			});
		} else {
			entries.push(ExplorerItem::NonIndexedPath {
				has_local_thumbnail: false,
				thumbnail_key: None,
				item: NonIndexedPathItem {
					id,
					path: directory,
					name,
					extension: "".to_string(),
					kind: ObjectKind::Folder as i32,
					is_dir: true,
					date_created: metadata.created_or_now().into(),
					date_modified: metadata.modified_or_now().into(),
					size_in_bytes_bytes: metadata.len().to_be_bytes().to_vec(),
				},
			});
		}
	}

	Ok(NonIndexedFileSystemEntries { entries, errors })
}
