use crate::{utils::sub_path, OuterContext};

use sd_core_file_path_helper::{FilePathError, IsolatedFilePathData};
use sd_core_prisma_helpers::CasId;

use sd_file_ext::{extensions::Extension, kind::ObjectKind};
use sd_prisma::prisma::{file_path, location};
use sd_task_system::{TaskDispatcher, TaskHandle};
use sd_utils::{db::MissingFieldError, error::FileIOError};

use std::{
	collections::{hash_map::Entry, HashMap},
	fs::Metadata,
	mem,
	path::Path,
	sync::Arc,
};

use prisma_client_rust::{or, QueryError};
use rspc::ErrorCode;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::fs;
use tracing::trace;

mod cas_id;
pub mod job;
mod shallow;
mod tasks;

pub use cas_id::generate_cas_id;

pub use job::FileIdentifier;
pub use shallow::shallow;

use tasks::FilePathToCreateOrLinkObject;

// we break these tasks into chunks of 100 to improve performance
const CHUNK_SIZE: usize = 100;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("missing field on database: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("failed to deserialized stored tasks for job resume: {0}")]
	DeserializeTasks(#[from] rmp_serde::decode::Error),
	#[error("database error: {0}")]
	Database(#[from] QueryError),

	#[error(transparent)]
	FilePathError(#[from] FilePathError),
	#[error(transparent)]
	SubPath(#[from] sub_path::Error),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::SubPath(sub_path_err) => sub_path_err.into(),

			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NonCriticalFileIdentifierError {
	#[error("failed to extract file metadata: {0}")]
	FailedToExtractFileMetadata(String),
	#[cfg(target_os = "windows")]
	#[error("failed to extract metadata from on-demand file: {0}")]
	FailedToExtractMetadataFromOnDemandFile(String),
	#[error("failed to extract isolated file path data: {0}")]
	FailedToExtractIsolatedFilePathData(String),
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
	pub cas_id: Option<CasId<'static>>,
	pub kind: ObjectKind,
	pub fs_metadata: Metadata,
}

impl FileMetadata {
	/// Fetch metadata from the file system and generate a cas id for the file
	/// if it's not empty.
	///
	/// # Panics
	/// Will panic if the file is a directory.
	pub async fn new(
		location_path: impl AsRef<Path> + Send,
		iso_file_path: &IsolatedFilePathData<'_>,
	) -> Result<Self, FileIOError> {
		let path = location_path.as_ref().join(iso_file_path);

		let fs_metadata = fs::metadata(&path)
			.await
			.map_err(|e| FileIOError::from((&path, e)))?;

		if fs_metadata.is_dir() {
			trace!(path = %path.display(), "Skipping directory;");
			return Ok(Self {
				cas_id: None,
				kind: ObjectKind::Folder,
				fs_metadata,
			});
		}

		// derive Object kind
		let kind = Extension::resolve_conflicting(&path, false)
			.await
			.map_or(ObjectKind::Unknown, Into::into);

		let cas_id = if fs_metadata.len() != 0 {
			generate_cas_id(&path, fs_metadata.len())
				.await
				.map(Some)
				.map_err(|e| FileIOError::from((&path, e)))?
		} else {
			// We can't do shit with empty files
			None
		};

		trace!(
			path = %path.display(),
			?cas_id,
			%kind,
			"Analyzed file;",
		);

		Ok(Self {
			cas_id,
			kind,
			fs_metadata,
		})
	}
}

fn orphan_path_filters_shallow(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	sub_iso_file_path: &IsolatedFilePathData<'_>,
) -> Vec<file_path::WhereParam> {
	sd_utils::chain_optional_iter(
		[
			or!(
				file_path::object_id::equals(None),
				file_path::cas_id::equals(None)
			),
			file_path::is_dir::equals(Some(false)),
			file_path::location_id::equals(Some(location_id)),
			file_path::materialized_path::equals(Some(
				sub_iso_file_path
					.materialized_path_for_children()
					.expect("sub path for shallow identifier must be a directory"),
			)),
			file_path::size_in_bytes_bytes::not(Some(0u64.to_be_bytes().to_vec())),
		],
		[file_path_id.map(file_path::id::gt)],
	)
}

fn orphan_path_filters_deep(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	maybe_sub_iso_file_path: &Option<IsolatedFilePathData<'_>>,
) -> Vec<file_path::WhereParam> {
	sd_utils::chain_optional_iter(
		[
			or!(
				file_path::object_id::equals(None),
				file_path::cas_id::equals(None)
			),
			file_path::is_dir::equals(Some(false)),
			file_path::location_id::equals(Some(location_id)),
			file_path::size_in_bytes_bytes::not(Some(0u64.to_be_bytes().to_vec())),
		],
		[
			// this is a workaround for the cursor not working properly
			file_path_id.map(file_path::id::gt),
			maybe_sub_iso_file_path.as_ref().map(|sub_iso_file_path| {
				file_path::materialized_path::starts_with(
					sub_iso_file_path
						.materialized_path_for_children()
						.expect("sub path iso_file_path must be a directory"),
				)
			}),
		],
	)
}

async fn dispatch_object_processor_tasks<Iter, Dispatcher>(
	file_paths_by_cas_id: Iter,
	ctx: &impl OuterContext,
	dispatcher: &Dispatcher,
	with_priority: bool,
) -> Result<Vec<TaskHandle<crate::Error>>, Dispatcher::DispatchError>
where
	Iter: IntoIterator<Item = (CasId<'static>, Vec<FilePathToCreateOrLinkObject>)> + Send,
	Iter::IntoIter: Send,
	Dispatcher: TaskDispatcher<crate::Error>,
{
	let mut current_batch = HashMap::<_, Vec<_>>::new();
	let mut tasks = vec![];

	let mut current_batch_size = 0;

	for (cas_id, objects_to_create_or_link) in file_paths_by_cas_id {
		if objects_to_create_or_link.len() >= CHUNK_SIZE {
			tasks.push(
				dispatcher
					.dispatch(tasks::ObjectProcessor::new(
						HashMap::from([(cas_id, objects_to_create_or_link)]),
						Arc::clone(ctx.db()),
						Arc::clone(ctx.sync()),
						with_priority,
					))
					.await?,
			);
		} else {
			current_batch_size += objects_to_create_or_link.len();
			match current_batch.entry(cas_id) {
				Entry::Occupied(entry) => {
					entry.into_mut().extend(objects_to_create_or_link);
				}
				Entry::Vacant(entry) => {
					entry.insert(objects_to_create_or_link);
				}
			}

			if current_batch_size >= CHUNK_SIZE {
				tasks.push(
					dispatcher
						.dispatch(tasks::ObjectProcessor::new(
							mem::take(&mut current_batch),
							Arc::clone(ctx.db()),
							Arc::clone(ctx.sync()),
							with_priority,
						))
						.await?,
				);

				current_batch_size = 0;
			}
		}
	}

	if !current_batch.is_empty() {
		tasks.push(
			dispatcher
				.dispatch(tasks::ObjectProcessor::new(
					current_batch,
					Arc::clone(ctx.db()),
					Arc::clone(ctx.sync()),
					with_priority,
				))
				.await?,
		);
	}

	Ok(tasks)
}

fn accumulate_file_paths_by_cas_id(
	input: HashMap<CasId<'static>, Vec<FilePathToCreateOrLinkObject>>,
	accumulator: &mut HashMap<CasId<'static>, Vec<FilePathToCreateOrLinkObject>>,
) {
	for (cas_id, file_paths) in input {
		match accumulator.entry(cas_id) {
			Entry::<_, Vec<_>>::Occupied(entry) => {
				entry.into_mut().extend(file_paths);
			}
			Entry::Vacant(entry) => {
				entry.insert(file_paths);
			}
		}
	}
}
