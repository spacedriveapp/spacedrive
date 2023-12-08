use crate::{
	api::locations::ExplorerItem,
	library::Library,
	object::{
		cas::generate_cas_id,
		media::thumbnail::{get_ephemeral_thumb_key, BatchToProcess, GenerateThumbnailArgs},
	},
	prisma::location,
	util::error::FileIOError,
	Node,
};

use std::{
	collections::HashMap,
	io::ErrorKind,
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::Stream;
use sd_file_ext::{extensions::Extension, kind::ObjectKind};

use chrono::{DateTime, Utc};
use rspc::ErrorCode;
use sd_utils::chain_optional_iter;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use tokio::{
	fs::{self, DirEntry},
	io,
	sync::mpsc,
	task::JoinError,
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, instrument, span, warn, Level};

use super::{
	file_path_helper::{path_is_hidden, MetadataExt},
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

	#[error("error joining tokio task: {0}")]
	TaskJoinError(#[from] JoinError),

	#[error("receiver shutdown error: {0}")]
	SendError(#[from] mpsc::error::SendError<Result<ExplorerItem, rspc::Error>>),
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
pub struct NonIndexedPathItem {
	pub path: String,
	pub name: String,
	pub extension: String,
	pub kind: i32,
	pub is_dir: bool,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub size_in_bytes_bytes: Vec<u8>,
	pub hidden: bool,
}

// #[instrument(name = "non_indexed::walk", skip(sort_fn))]
pub async fn walk(
	path: PathBuf,
	with_hidden_files: bool,
	node: Arc<Node>,
	library: Arc<Library>,
	sort_fn: impl FnOnce(&mut Vec<Entry>) + Send,
) -> Result<impl Stream<Item = Result<ExplorerItem, rspc::Error>> + Send, NonIndexedLocationError> {
	let start = std::time::Instant::now();

	let mut entries = get_all_entries(path.clone()).await?;

	println!("get_all_entries took: {:?}", start.elapsed());
	let start = std::time::Instant::now();

	{
		let span = span!(Level::INFO, "sort_fn");
		let _enter = span.enter();

		sort_fn(&mut entries);
	}

	println!("sort_fn took: {:?}", start.elapsed());

	let (tx, rx) = mpsc::channel(128);

	// We wanna process and let the caller use the stream.
	tokio::spawn(async move {
		let path = &path;
		let rules = chain_optional_iter(
			[IndexerRule::from(no_os_protected())],
			[(!with_hidden_files).then(|| IndexerRule::from(no_hidden()))],
		);

		let mut thumbnails_to_generate = vec![];
		// Generating thumbnails for PDFs is kinda slow, so we're leaving them for last in the batch
		let mut document_thumbnails_to_generate = vec![];
		let mut directories = vec![];

		for entry in entries.into_iter() {
			let (entry_path, name) = match normalize_path(entry.path) {
				Ok(v) => v,
				Err(e) => {
					tx.send(Err(NonIndexedLocationError::from((path, e)).into()))
						.await?;
					continue;
				}
			};

			match IndexerRule::apply_all(&rules, &entry_path).await {
				Ok(rule_results) => {
					// No OS Protected and No Hidden rules, must always be from this kind, should panic otherwise
					if rule_results[&RuleKind::RejectFilesByGlob]
						.iter()
						.any(|reject| !reject)
					{
						continue;
					}
				}
				Err(e) => {
					tx.send(Err(e.into())).await?;
					continue;
				}
			};

			if entry.metadata.is_dir() {
				directories.push((entry_path, name, entry.metadata));
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
					if let Ok(cas_id) = generate_cas_id(&path, entry.metadata.len())
						.await
						.map_err(|e| tx.send(Err(NonIndexedLocationError::from((path, e)).into())))
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

						Some(get_ephemeral_thumb_key(&cas_id))
					} else {
						None
					}
				} else {
					None
				};

				tx.send(Ok(ExplorerItem::NonIndexedPath {
					has_local_thumbnail: thumbnail_key.is_some(),
					thumbnail_key,
					item: NonIndexedPathItem {
						hidden: path_is_hidden(Path::new(&entry_path), &entry.metadata),
						path: entry_path,
						name,
						extension,
						kind: kind as i32,
						is_dir: false,
						date_created: entry.metadata.created_or_now().into(),
						date_modified: entry.metadata.modified_or_now().into(),
						size_in_bytes_bytes: entry.metadata.len().to_be_bytes().to_vec(),
					},
				}))
				.await?;
			}
		}

		thumbnails_to_generate.extend(document_thumbnails_to_generate);

		node.thumbnailer
			.new_ephemeral_thumbnails_batch(BatchToProcess::new(
				thumbnails_to_generate,
				false,
				false,
			))
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
				tx.send(Ok(ExplorerItem::Location {
					has_local_thumbnail: false,
					thumbnail_key: None,
					item: location,
				}))
				.await?;
			} else {
				tx.send(Ok(ExplorerItem::NonIndexedPath {
					has_local_thumbnail: false,
					thumbnail_key: None,
					item: NonIndexedPathItem {
						hidden: path_is_hidden(Path::new(&directory), &metadata),
						path: directory,
						name,
						extension: String::new(),
						kind: ObjectKind::Folder as i32,
						is_dir: true,
						date_created: metadata.created_or_now().into(),
						date_modified: metadata.modified_or_now().into(),
						size_in_bytes_bytes: metadata.len().to_be_bytes().to_vec(),
					},
				}))
				.await?;
			}
		}

		Ok::<(), NonIndexedLocationError>(()) // TODO: Actually handle this error and not just send it to the ether
	});

	Ok(ReceiverStream::new(rx))
}

#[derive(Debug)]
pub struct Entry {
	path: PathBuf,
	name: String,
	// size_in_bytes: u64,
	// date_created:
	metadata: std::fs::Metadata,
}

impl Entry {
	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn size_in_bytes(&self) -> u64 {
		self.metadata.len()
	}

	pub fn date_created(&self) -> DateTime<Utc> {
		self.metadata.created_or_now().into()
	}

	pub fn date_modified(&self) -> DateTime<Utc> {
		self.metadata.modified_or_now().into()
	}
}

/// We get all of the FS entries first before we start processing on each of them.
///
/// From my M1 Macbook Pro this:
///  - takes 11ms per 10 000 files
///  and
///  - consumes 0.16MB of RAM per 10 000 entries.
///
/// The reason we collect these all up is so we can apply ordering, and then begin streaming the data as it's processed to the frontend.
// #[instrument(name = "get_all_entries")]
pub async fn get_all_entries(path: PathBuf) -> Result<Vec<Entry>, NonIndexedLocationError> {
	tokio::task::spawn_blocking(move || {
		let path = &path;
		let mut dir = std::fs::read_dir(&path).map_err(|e| (path, e))?;
		let mut entries = Vec::new();
		while let Some(entry) = dir.next() {
			let entry = entry.map_err(|e| (path, e))?;

			// We must not keep `entry` around as we will quickly hit the OS limit on open file descriptors
			entries.push(Entry {
				path: entry.path(),
				name: entry
					.file_name()
					.to_str()
					.ok_or_else(|| {
						(
							path,
							io::Error::new(ErrorKind::Other, "error non UTF-8 path"),
						)
					})?
					.to_string(),
				metadata: entry.metadata().map_err(|e| (path, e))?,
			});
		}

		Ok(entries)
	})
	.await?
}
