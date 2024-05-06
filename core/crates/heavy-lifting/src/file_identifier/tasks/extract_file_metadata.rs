use crate::{
	file_identifier::{FileMetadata, NonCriticalFileIdentifierError},
	Error, NonCriticalJobError,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_file_identifier;

use sd_prisma::prisma::location;
use sd_task_system::{
	ExecStatus, Interrupter, InterruptionKind, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};
use sd_utils::error::FileIOError;

use std::{
	collections::HashMap, future::IntoFuture, mem, path::PathBuf, pin::pin, sync::Arc,
	time::Duration,
};

use futures::stream::{self, FuturesUnordered, StreamExt};
use futures_concurrency::stream::Merge;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::error;
use uuid::Uuid;

use super::IdentifiedFile;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractFileMetadataTask {
	id: TaskId,
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	file_paths_by_id: HashMap<Uuid, file_path_for_file_identifier::Data>,
	identified_files: HashMap<Uuid, IdentifiedFile>,
	extract_metadata_time: Duration,
	errors: Vec<NonCriticalJobError>,
	is_shallow: bool,
}

#[derive(Debug)]
pub struct ExtractFileMetadataTaskOutput {
	pub identified_files: HashMap<Uuid, IdentifiedFile>,
	pub extract_metadata_time: Duration,
	pub errors: Vec<NonCriticalJobError>,
}

impl ExtractFileMetadataTask {
	fn new(
		location: Arc<location::Data>,
		location_path: Arc<PathBuf>,
		file_paths: Vec<file_path_for_file_identifier::Data>,
		is_shallow: bool,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			location,
			location_path,
			identified_files: HashMap::with_capacity(file_paths.len()),
			file_paths_by_id: file_paths
				.into_iter()
				.map(|file_path| {
					// SAFETY: This should never happen
					(
						Uuid::from_slice(&file_path.pub_id).expect("file_path.pub_id is invalid!"),
						file_path,
					)
				})
				.collect(),
			extract_metadata_time: Duration::ZERO,
			errors: Vec::new(),
			is_shallow,
		}
	}

	#[must_use]
	pub fn new_deep(
		location: Arc<location::Data>,
		location_path: Arc<PathBuf>,
		file_paths: Vec<file_path_for_file_identifier::Data>,
	) -> Self {
		Self::new(location, location_path, file_paths, false)
	}

	#[must_use]
	pub fn new_shallow(
		location: Arc<location::Data>,
		location_path: Arc<PathBuf>,
		file_paths: Vec<file_path_for_file_identifier::Data>,
	) -> Self {
		Self::new(location, location_path, file_paths, true)
	}
}

#[async_trait::async_trait]
impl Task<Error> for ExtractFileMetadataTask {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		self.is_shallow
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		enum StreamMessage {
			Processed(Uuid, Result<FileMetadata, FileIOError>),
			Interrupt(InterruptionKind),
		}

		let Self {
			location,
			location_path,
			file_paths_by_id,
			identified_files,
			extract_metadata_time,
			errors,
			..
		} = self;

		let start_time = Instant::now();

		if !file_paths_by_id.is_empty() {
			let extraction_futures = file_paths_by_id
				.iter()
				.filter_map(|(file_path_id, file_path)| {
					try_iso_file_path_extraction(
						location.id,
						*file_path_id,
						file_path,
						Arc::clone(location_path),
						errors,
					)
				})
				.map(|(file_path_id, iso_file_path, location_path)| async move {
					StreamMessage::Processed(
						file_path_id,
						FileMetadata::new(&*location_path, &iso_file_path).await,
					)
				})
				.collect::<FuturesUnordered<_>>();

			let mut msg_stream = pin!((
				extraction_futures,
				stream::once(interrupter.into_future()).map(StreamMessage::Interrupt)
			)
				.merge());

			while let Some(msg) = msg_stream.next().await {
				match msg {
					StreamMessage::Processed(file_path_pub_id, res) => {
						let file_path = file_paths_by_id
							.remove(&file_path_pub_id)
							.expect("file_path must be here");

						match res {
							Ok(FileMetadata { cas_id, kind, .. }) => {
								identified_files.insert(
									file_path_pub_id,
									IdentifiedFile {
										file_path,
										cas_id,
										kind,
									},
								);
							}
							Err(e) => {
								handle_non_critical_errors(
									location.id,
									file_path_pub_id,
									&e,
									errors,
								);
							}
						}

						if file_paths_by_id.is_empty() {
							// All files have been processed so we can end this merged stream and don't keep waiting an
							// interrupt signal
							break;
						}
					}

					StreamMessage::Interrupt(kind) => {
						*extract_metadata_time += start_time.elapsed();
						return Ok(match kind {
							InterruptionKind::Pause => ExecStatus::Paused,
							InterruptionKind::Cancel => ExecStatus::Canceled,
						});
					}
				}
			}
		}

		Ok(ExecStatus::Done(
			ExtractFileMetadataTaskOutput {
				identified_files: mem::take(identified_files),
				extract_metadata_time: *extract_metadata_time + start_time.elapsed(),
				errors: mem::take(errors),
			}
			.into_output(),
		))
	}
}

fn handle_non_critical_errors(
	location_id: location::id::Type,
	file_path_pub_id: Uuid,
	e: &FileIOError,
	errors: &mut Vec<NonCriticalJobError>,
) {
	error!("Failed to extract file metadata <location_id={location_id}, file_path_pub_id='{file_path_pub_id}'>: {e:#?}");

	let formatted_error = format!("<file_path_pub_id='{file_path_pub_id}', error={e}>");

	#[cfg(target_os = "windows")]
	{
		// Handle case where file is on-demand (NTFS only)
		if e.source.raw_os_error().map_or(false, |code| code == 362) {
			errors.push(
				NonCriticalFileIdentifierError::FailedToExtractMetadataFromOnDemandFile(
					formatted_error,
				)
				.into(),
			);
		} else {
			errors.push(
				NonCriticalFileIdentifierError::FailedToExtractFileMetadata(formatted_error).into(),
			);
		}
	}

	#[cfg(not(target_os = "windows"))]
	{
		errors.push(
			NonCriticalFileIdentifierError::FailedToExtractFileMetadata(formatted_error).into(),
		);
	}
}

fn try_iso_file_path_extraction(
	location_id: location::id::Type,
	file_path_pub_id: Uuid,
	file_path: &file_path_for_file_identifier::Data,
	location_path: Arc<PathBuf>,
	errors: &mut Vec<NonCriticalJobError>,
) -> Option<(Uuid, IsolatedFilePathData<'static>, Arc<PathBuf>)> {
	IsolatedFilePathData::try_from((location_id, file_path))
		.map(IsolatedFilePathData::to_owned)
		.map(|iso_file_path| (file_path_pub_id, iso_file_path, location_path))
		.map_err(|e| {
			error!("Failed to extract isolated file path data: {e:#?}");
			errors.push(
				NonCriticalFileIdentifierError::FailedToExtractIsolatedFilePathData(format!(
					"<file_path_pub_id='{file_path_pub_id}', error={e}>"
				))
				.into(),
			);
		})
		.ok()
}

impl SerializableTask<Error> for ExtractFileMetadataTask {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = ();

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		rmp_serde::to_vec_named(&self)
	}

	async fn deserialize(
		data: &[u8],
		(): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data)
	}
}
