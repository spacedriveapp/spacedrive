use crate::{
	api::locations::ExplorerItem,
	library::Library,
	object::{
		cas::generate_cas_id,
		media::thumbnail::{
			actor::{BatchToProcess, GenerateThumbnailArgs},
			get_thumb_key,
		},
	},
	prisma::location,
	util::error::FileIOError,
	Node,
};

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_file_ext::{extensions::Extension, kind::ObjectKind};

use chrono::{DateTime, Utc};
use rspc::ErrorCode;
use sd_utils::chain_optional_iter;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use tokio::{fs, io};
use tracing::{error, warn};

use super::{
	file_path_helper::MetadataExt,
	indexer::rules::{
		seed::{no_hidden, no_os_protected},
		IndexerRule, RuleKind,
	},
	normalize_path,
};

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
	pub entries: Vec<ExplorerItem>,
	pub errors: Vec<rspc::Error>,
}

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedPathItem {
	pub path: String,
	pub name: String,
	pub extension: String,
	pub kind: i32,
	pub is_dir: bool,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub size_in_bytes_bytes: Vec<u8>,
}

pub async fn walk(
	full_path: impl AsRef<Path>,
	with_hidden_files: bool,
	node: Arc<Node>,
	library: Arc<Library>,
) -> Result<NonIndexedFileSystemEntries, NonIndexedLocationError> {
	let path = full_path.as_ref();
	let mut read_dir = fs::read_dir(path).await.map_err(|e| (path, e))?;

	let mut directories = vec![];
	let mut errors = vec![];
	let mut entries = vec![];

	let rules = chain_optional_iter(
		[IndexerRule::from(no_os_protected())],
		[(!with_hidden_files).then(|| IndexerRule::from(no_hidden()))],
	);

	let mut thumbnails_to_generate = vec![];
	// Generating thumbnails for PDFs is kinda slow, so we're leaving them for last in the batch
	let mut document_thumbnails_to_generate = vec![];

	while let Some(entry) = read_dir.next_entry().await.map_err(|e| (path, e))? {
		let Ok((entry_path, name)) = normalize_path(entry.path())
			.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into()))
		else {
			continue;
		};

		if let Ok(rule_results) = IndexerRule::apply_all(&rules, &entry_path)
			.await
			.map_err(|e| errors.push(e.into()))
		{
			// No OS Protected and No Hidden rules, must always be from this kind, should panic otherwise
			if rule_results[&RuleKind::RejectFilesByGlob]
				.iter()
				.any(|reject| !reject)
			{
				continue;
			}
		} else {
			continue;
		}

		let Ok(metadata) = entry
			.metadata()
			.await
			.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into()))
		else {
			continue;
		};

		if metadata.is_dir() {
			directories.push((entry_path, name, metadata));
		} else {
			let path = Path::new(&entry_path);

			let Some(name) = path
				.file_stem()
				.and_then(|s| s.to_str().map(str::to_string))
			else {
				warn!("Failed to extract name from path: {}", &entry_path);
				continue;
			};

			let extension = path
				.extension()
				.and_then(|s| s.to_str().map(str::to_string))
				.unwrap_or_default();

			let kind = Extension::resolve_conflicting(&path, false)
				.await
				.map(Into::into)
				.unwrap_or(ObjectKind::Unknown);

			let should_generate_thumbnail = {
				#[cfg(feature = "ffmpeg")]
				{
					matches!(
						kind,
						ObjectKind::Image | ObjectKind::Video | ObjectKind::Document
					)
				}

				#[cfg(not(feature = "ffmpeg"))]
				{
					matches!(kind, ObjectKind::Image | ObjectKind::Document)
				}
			};

			let thumbnail_key = if should_generate_thumbnail {
				if let Ok(cas_id) = generate_cas_id(&path, metadata.len())
					.await
					.map_err(|e| errors.push(NonIndexedLocationError::from((path, e)).into()))
				{
					if kind == ObjectKind::Document {
						document_thumbnails_to_generate.push(GenerateThumbnailArgs::new(
							extension.clone(),
							cas_id.clone(),
							path.to_path_buf(),
						));
					} else {
						thumbnails_to_generate.push(GenerateThumbnailArgs::new(
							extension.clone(),
							cas_id.clone(),
							path.to_path_buf(),
						));
					}

					let thumbnail_key = get_thumb_key(&cas_id);

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
	}

	thumbnails_to_generate.extend(document_thumbnails_to_generate);

	node.thumbnailer
		.new_ephemeral_thumbnails_batch(BatchToProcess {
			batch: thumbnails_to_generate,
			should_regenerate: false,
			in_background: false,
		})
		.await;

	let mut locations = library
		.db
		.location()
		.find_many(vec![location::path::in_vec(
			directories
				.iter()
				.map(|(path, _, _)| path.clone())
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

	for (directory, name, metadata) in directories {
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
					path: directory,
					name,
					extension: String::new(),
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
