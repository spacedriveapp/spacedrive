use crate::{
	file_identifier::{self, FileMetadata},
	Error, NonCriticalError,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::{file_path_for_file_identifier, CasId, FilePathPubId};
use sd_core_sync::SyncManager;

use sd_file_ext::kind::ObjectKind;
use sd_prisma::{
	prisma::{device, file_path, location, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_entry, OperationFactory};
use sd_task_system::{
	ExecStatus, Interrupter, InterruptionKind, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};
use sd_utils::error::FileIOError;

use std::{
	collections::HashMap, convert::identity, future::IntoFuture, mem, path::PathBuf, pin::pin,
	sync::Arc, time::Duration,
};

use futures::stream::{self, FuturesUnordered, StreamExt};
use futures_concurrency::{future::TryJoin, stream::Merge};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{error, instrument, trace, warn, Level};

use super::{create_objects_and_update_file_paths, FilePathToCreateOrLinkObject};

#[derive(Debug, Serialize, Deserialize)]
struct IdentifiedFile {
	file_path: file_path_for_file_identifier::Data,
	cas_id: CasId<'static>,
	kind: ObjectKind,
}

impl IdentifiedFile {
	pub fn new(
		file_path: file_path_for_file_identifier::Data,
		cas_id: impl Into<CasId<'static>>,
		kind: ObjectKind,
	) -> Self {
		Self {
			file_path,
			cas_id: cas_id.into(),
			kind,
		}
	}
}

#[derive(Debug)]
pub struct Identifier {
	// Task control
	id: TaskId,
	with_priority: bool,

	// Received input args
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	file_paths_by_id: HashMap<FilePathPubId, file_path_for_file_identifier::Data>,

	// Inner state
	device_id: device::id::Type,
	identified_files: HashMap<FilePathPubId, IdentifiedFile>,
	file_paths_without_cas_id: Vec<FilePathToCreateOrLinkObject>,

	// Out collector
	output: Output,

	// Dependencies
	db: Arc<PrismaClient>,
	sync: SyncManager,
}

/// Output from the `[Identifier]` task
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Output {
	/// To send to frontend for priority reporting of new objects
	pub file_path_ids_with_new_object: Vec<file_path::id::Type>,

	/// Files that need to be aggregate between many identifier tasks to be processed by the
	/// object processor tasks
	pub file_paths_by_cas_id: HashMap<CasId<'static>, Vec<FilePathToCreateOrLinkObject>>,

	/// Collected metric about time elapsed extracting metadata from file system
	pub extract_metadata_time: Duration,

	/// Collected metric about time spent saving objects on disk
	pub save_db_time: Duration,

	/// Total number of objects already created as they didn't have `cas_id`, like directories or empty files
	pub created_objects_count: u64,

	/// Total number of files that we were able to identify
	pub total_identified_files: u64,

	/// Non critical errors that happened during the task execution
	pub errors: Vec<NonCriticalError>,
}

#[async_trait::async_trait]
impl Task<Error> for Identifier {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		self.with_priority
	}

	#[instrument(
		skip(self, interrupter),
		fields(
			task_id = %self.id,
			location_id = %self.location.id,
			location_path = %self.location_path.display(),
			files_count = %self.file_paths_by_id.len(),
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		// `Processed` is larger than `Interrupt`, but it's much more common
		// so we ignore the size difference to optimize for usage
		#[allow(clippy::large_enum_variant)]
		enum StreamMessage {
			Processed(FilePathPubId, Result<FileMetadata, FileIOError>),
			Interrupt(InterruptionKind),
		}

		let Self {
			location,
			location_path,
			device_id,
			file_paths_by_id,
			file_paths_without_cas_id,
			identified_files,
			output,
			..
		} = self;

		if !file_paths_by_id.is_empty() {
			let start_time = Instant::now();

			let extraction_futures = file_paths_by_id
				.iter()
				.filter_map(|(file_path_id, file_path)| {
					try_iso_file_path_extraction(
						location.id,
						file_path_id.clone(),
						file_path,
						Arc::clone(location_path),
						&mut output.errors,
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

						trace!(
							files_remaining = file_paths_by_id.len(),
							%file_path_pub_id,
							"Processed file;",
						);

						match res {
							Ok(FileMetadata {
								cas_id: Some(cas_id),
								kind,
								..
							}) => {
								identified_files.insert(
									file_path_pub_id,
									IdentifiedFile::new(file_path, cas_id, kind),
								);
							}
							Ok(FileMetadata {
								cas_id: None, kind, ..
							}) => {
								let file_path_for_file_identifier::Data {
									id,
									pub_id,
									date_created,
									..
								} = file_path;
								file_paths_without_cas_id.push(FilePathToCreateOrLinkObject {
									id,
									file_path_pub_id: pub_id.into(),
									kind,
									created_at: date_created,
								});
							}
							Err(e) => {
								handle_non_critical_errors(
									file_path_pub_id,
									&e,
									&mut output.errors,
								);
							}
						}

						if file_paths_by_id.is_empty() {
							trace!("All files have been processed");
							// All files have been processed so we can end this merged stream
							// and don't keep waiting an interrupt signal
							break;
						}
					}

					StreamMessage::Interrupt(kind) => {
						trace!(?kind, "Interrupted;");
						output.extract_metadata_time += start_time.elapsed();
						return Ok(match kind {
							InterruptionKind::Pause => ExecStatus::Paused,
							InterruptionKind::Cancel => ExecStatus::Canceled,
						});
					}
				}
			}

			output.extract_metadata_time = start_time.elapsed();

			output.total_identified_files =
				identified_files.len() as u64 + file_paths_without_cas_id.len() as u64;

			trace!(
				identified_files_count = identified_files.len(),
				"All files have been processed, saving cas_ids to db...;"
			);
			let start_time = Instant::now();
			// Assign cas_id to each file path
			let ((), file_path_ids_with_new_object) = (
				assign_cas_id_to_file_paths(identified_files, &self.db, &self.sync),
				create_objects_and_update_file_paths(
					file_paths_without_cas_id.drain(..),
					&self.db,
					&self.sync,
					*device_id,
				),
			)
				.try_join()
				.await?;

			output.save_db_time = start_time.elapsed();
			output.created_objects_count = file_path_ids_with_new_object.len() as u64;
			output.file_path_ids_with_new_object =
				file_path_ids_with_new_object.into_keys().collect();

			output.file_paths_by_cas_id = identified_files.drain().fold(
				HashMap::new(),
				|mut map,
				 (
					file_path_pub_id,
					IdentifiedFile {
						cas_id,
						kind,
						file_path:
							file_path_for_file_identifier::Data {
								id, date_created, ..
							},
					},
				)| {
					map.entry(cas_id)
						.or_insert_with(|| Vec::with_capacity(1))
						.push(FilePathToCreateOrLinkObject {
							id,
							file_path_pub_id,
							kind,
							created_at: date_created,
						});

					map
				},
			);

			trace!(save_db_time = ?output.save_db_time, "Cas_ids saved to db;");
		} else if !file_paths_without_cas_id.is_empty() {
			let start_time = Instant::now();

			// Assign objects to directories
			let file_path_ids_with_new_object = create_objects_and_update_file_paths(
				file_paths_without_cas_id.drain(..),
				&self.db,
				&self.sync,
				*device_id,
			)
			.await?;

			output.save_db_time = start_time.elapsed();
			output.created_objects_count = file_path_ids_with_new_object.len() as u64;
			output.file_path_ids_with_new_object =
				file_path_ids_with_new_object.into_keys().collect();

			trace!(save_db_time = ?output.save_db_time, "Directories objects saved to db;");
		}

		Ok(ExecStatus::Done(mem::take(output).into_output()))
	}
}

impl Identifier {
	#[must_use]
	pub fn new(
		location: Arc<location::Data>,
		location_path: Arc<PathBuf>,
		file_paths: Vec<file_path_for_file_identifier::Data>,
		with_priority: bool,
		db: Arc<PrismaClient>,
		sync: SyncManager,
		device_id: device::id::Type,
	) -> Self {
		let mut output = Output::default();

		let file_paths_count = file_paths.len();
		let directories_count = file_paths
			.iter()
			.filter(|file_path| file_path.is_dir.is_some_and(identity))
			.count();

		let (file_paths_by_id, file_paths_without_cas_id) = file_paths.into_iter().fold(
			(
				HashMap::with_capacity(file_paths_count - directories_count),
				Vec::with_capacity(directories_count),
			),
			|(mut file_paths_by_id, mut directory_file_paths), file_path| {
				match file_path.is_dir {
					Some(true) => {
						let file_path_for_file_identifier::Data {
							id,
							pub_id,
							date_created,
							..
						} = file_path;
						directory_file_paths.push(FilePathToCreateOrLinkObject {
							id,
							file_path_pub_id: pub_id.into(),
							kind: ObjectKind::Folder,
							created_at: date_created,
						});
					}
					Some(false) => {
						file_paths_by_id.insert(file_path.pub_id.as_slice().into(), file_path);
					}
					None => {
						warn!(%file_path.id, "file path without is_dir field, skipping;");
						output.errors.push(
						file_identifier::NonCriticalFileIdentifierError::FilePathWithoutIsDirField(
							file_path.id,
						)
						.into(),
					);
					}
				};

				(file_paths_by_id, directory_file_paths)
			},
		);

		Self {
			id: TaskId::new_v4(),
			location,
			location_path,
			device_id,
			identified_files: HashMap::with_capacity(file_paths_count - directories_count),
			file_paths_without_cas_id,
			file_paths_by_id,
			output,
			with_priority,
			db,
			sync,
		}
	}
}

#[instrument(skip_all, err, fields(identified_files_count = identified_files.len()))]
async fn assign_cas_id_to_file_paths(
	identified_files: &HashMap<FilePathPubId, IdentifiedFile>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<(), file_identifier::Error> {
	let (ops, queries) = identified_files
		.iter()
		.map(|(pub_id, IdentifiedFile { cas_id, .. })| {
			let (sync_param, db_param) = sync_db_entry!(cas_id, file_path::cas_id);

			(
				sync.shared_update(
					prisma_sync::file_path::SyncId {
						pub_id: pub_id.to_db(),
					},
					[sync_param],
				),
				db.file_path()
					.update(file_path::pub_id::equals(pub_id.to_db()), vec![db_param])
					// We don't need any data here, just the id avoids receiving the entire object
					// as we can't pass an empty select macro call
					.select(file_path::select!({ id })),
			)
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	if !ops.is_empty() && !queries.is_empty() {
		// Assign cas_id to each file path
		sync.write_ops(db, (ops, queries)).await?;
	}

	Ok(())
}

#[instrument(skip(errors))]
fn handle_non_critical_errors(
	file_path_pub_id: FilePathPubId,
	e: &FileIOError,
	errors: &mut Vec<NonCriticalError>,
) {
	let formatted_error = format!("<file_path_pub_id='{file_path_pub_id}', error={e}>");

	#[cfg(target_os = "windows")]
	{
		// Handle case where file is on-demand (NTFS only)
		if e.source.raw_os_error().map_or(false, |code| code == 362) {
			errors.push(
				file_identifier::NonCriticalFileIdentifierError::FailedToExtractMetadataFromOnDemandFile(
					formatted_error,
				)
				.into(),
			);
		} else {
			errors.push(
				file_identifier::NonCriticalFileIdentifierError::FailedToExtractFileMetadata(
					formatted_error,
				)
				.into(),
			);
		}
	}

	#[cfg(not(target_os = "windows"))]
	{
		errors.push(
			file_identifier::NonCriticalFileIdentifierError::FailedToExtractFileMetadata(
				formatted_error,
			)
			.into(),
		);
	}
}

#[instrument(
	skip(location_id, file_path, location_path, errors),
	fields(
		file_path_id = file_path.id,
		materialized_path = ?file_path.materialized_path,
		name = ?file_path.name,
		extension = ?file_path.extension,
	)
)]
fn try_iso_file_path_extraction(
	location_id: location::id::Type,
	file_path_pub_id: FilePathPubId,
	file_path: &file_path_for_file_identifier::Data,
	location_path: Arc<PathBuf>,
	errors: &mut Vec<NonCriticalError>,
) -> Option<(FilePathPubId, IsolatedFilePathData<'static>, Arc<PathBuf>)> {
	match IsolatedFilePathData::try_from((location_id, file_path))
		.map(IsolatedFilePathData::to_owned)
	{
		Ok(iso_file_path) => Some((file_path_pub_id, iso_file_path, location_path)),
		Err(e) => {
			error!(?e, %file_path_pub_id, "Failed to extract isolated file path data;");
			errors.push(
				file_identifier::NonCriticalFileIdentifierError::FailedToExtractIsolatedFilePathData { file_path_pub_id: file_path_pub_id.into(), error: e.to_string() }.into(),

			);
			None
		}
	}
}

#[derive(Serialize, Deserialize)]
struct SaveState {
	id: TaskId,
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	device_id: device::id::Type,
	file_paths_by_id: HashMap<FilePathPubId, file_path_for_file_identifier::Data>,
	identified_files: HashMap<FilePathPubId, IdentifiedFile>,
	file_paths_without_cas_id: Vec<FilePathToCreateOrLinkObject>,
	output: Output,
	with_priority: bool,
}

impl SerializableTask<Error> for Identifier {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, SyncManager);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			location,
			location_path,
			device_id,
			file_paths_by_id,
			identified_files,
			file_paths_without_cas_id,
			output,
			with_priority,
			..
		} = self;
		rmp_serde::to_vec_named(&SaveState {
			id,
			location,
			location_path,
			device_id,
			file_paths_by_id,
			identified_files,
			file_paths_without_cas_id,
			output,
			with_priority,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice::<SaveState>(data).map(
			|SaveState {
			     id,
			     location,
			     location_path,
			     device_id,
			     file_paths_by_id,
			     identified_files,
			     file_paths_without_cas_id,
			     output,
			     with_priority,
			 }| Self {
				id,
				with_priority,
				location,
				location_path,
				file_paths_by_id,
				device_id,
				identified_files,
				file_paths_without_cas_id,
				output,
				db,
				sync,
			},
		)
	}
}
