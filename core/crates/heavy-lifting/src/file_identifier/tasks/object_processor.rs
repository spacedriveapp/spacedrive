use crate::{file_identifier, Error};

use sd_core_prisma_helpers::{object_for_file_identifier, CasId, ObjectPubId};
use sd_core_sync::Manager as SyncManager;

use sd_prisma::prisma::{file_path, object, PrismaClient};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};

use std::{collections::HashMap, mem, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{instrument, trace, Level};

use super::{
	connect_file_path_to_object, create_objects_and_update_file_paths, FilePathToCreateOrLinkObject,
};

#[derive(Debug)]
pub struct ObjectProcessor {
	// Task control
	id: TaskId,
	with_priority: bool,

	// Received input args
	file_paths_by_cas_id: HashMap<CasId, Vec<FilePathToCreateOrLinkObject>>,

	// Inner state
	stage: Stage,

	// Out collector
	output: Output,

	// Dependencies
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
}

#[derive(Debug, Serialize, Deserialize)]
enum Stage {
	Starting,
	AssignFilePathsToExistingObjects {
		existing_objects_by_cas_id: HashMap<CasId, ObjectPubId>,
	},
	CreateObjects,
}

/// Output from the `[ObjectProcessor]` task
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Output {
	/// To send to frontend for priority reporting of new objects
	pub file_path_ids_with_new_object: Vec<file_path::id::Type>,

	/// Time elapsed fetching existing `objects` from db to be linked to `file_paths`
	pub fetch_existing_objects_time: Duration,

	/// Time spent linking `file_paths` to already existing `objects`
	pub assign_to_existing_object_time: Duration,

	/// Time spent creating new `objects`
	pub create_object_time: Duration,

	/// Number of new `objects` created
	pub created_objects_count: u64,

	/// Number of `objects` that were linked to `file_paths`
	pub linked_objects_count: u64,
}

#[async_trait::async_trait]
impl Task<Error> for ObjectProcessor {
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
			cas_ids_count = %self.file_paths_by_cas_id.len(),
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		let Self {
			db,
			sync,
			file_paths_by_cas_id,
			stage,
			output:
				Output {
					file_path_ids_with_new_object,
					fetch_existing_objects_time,
					assign_to_existing_object_time,
					create_object_time,
					created_objects_count,
					linked_objects_count,
				},
			..
		} = self;

		loop {
			match stage {
				Stage::Starting => {
					trace!("Starting object processor task");
					let start = Instant::now();
					let existing_objects_by_cas_id =
						fetch_existing_objects_by_cas_id(file_paths_by_cas_id.keys(), db).await?;
					*fetch_existing_objects_time = start.elapsed();

					trace!(
						elapsed_time = ?fetch_existing_objects_time,
						existing_objects_count = existing_objects_by_cas_id.len(),
						"Fetched existing Objects",
					);
					*stage = Stage::AssignFilePathsToExistingObjects {
						existing_objects_by_cas_id,
					};
				}

				Stage::AssignFilePathsToExistingObjects {
					existing_objects_by_cas_id,
				} => {
					trace!(
						existing_objects_to_link = existing_objects_by_cas_id.len(),
						"Assigning file paths to existing Objects",
					);
					let start = Instant::now();
					*linked_objects_count = assign_existing_objects_to_file_paths(
						file_paths_by_cas_id,
						existing_objects_by_cas_id,
						db,
						sync,
					)
					.await?;
					*assign_to_existing_object_time = start.elapsed();

					trace!(
						existing_objects_to_link = existing_objects_by_cas_id.len(),
						%linked_objects_count,
						"Found existing Objects, linked file paths to them",
					);

					*stage = Stage::CreateObjects;

					if file_paths_by_cas_id.is_empty() {
						trace!("No more objects to be created, finishing task");
						// No objects to be created, we're good to finish already
						break;
					}
				}

				Stage::CreateObjects => {
					trace!(
						creating_count = file_paths_by_cas_id.len(),
						"Creating new Objects"
					);
					let start = Instant::now();
					*file_path_ids_with_new_object = create_objects_and_update_file_paths(
						mem::take(file_paths_by_cas_id).into_values().flatten(),
						db,
						sync,
					)
					.await?;
					*create_object_time = start.elapsed();

					*created_objects_count = file_path_ids_with_new_object.len() as u64;

					trace!(%created_objects_count, ?create_object_time, "Created new Objects");

					break;
				}
			}

			check_interruption!(interrupter);
		}

		Ok(ExecStatus::Done(mem::take(&mut self.output).into_output()))
	}
}

impl ObjectProcessor {
	#[must_use]
	pub fn new(
		file_paths_by_cas_id: HashMap<CasId, Vec<FilePathToCreateOrLinkObject>>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
		with_priority: bool,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			db,
			sync,
			file_paths_by_cas_id,
			stage: Stage::Starting,
			output: Output::default(),
			with_priority,
		}
	}
}

/// Retrieves objects that are already connected to file paths with the same cas_id
#[instrument(skip_all, err)]
async fn fetch_existing_objects_by_cas_id<'cas_id, Iter>(
	cas_ids: Iter,
	db: &PrismaClient,
) -> Result<HashMap<CasId, ObjectPubId>, file_identifier::Error>
where
	Iter: IntoIterator<Item = &'cas_id CasId> + Send,
	Iter::IntoIter: Send,
{
	async fn inner(
		stringed_cas_ids: Vec<String>,
		db: &PrismaClient,
	) -> Result<HashMap<CasId, ObjectPubId>, file_identifier::Error> {
		db.object()
			.find_many(vec![object::file_paths::some(vec![
				file_path::cas_id::in_vec(stringed_cas_ids),
				file_path::object_id::not(None),
			])])
			.select(object_for_file_identifier::select())
			.exec()
			.await
			.map_err(Into::into)
			.map(|objects| {
				objects
					.into_iter()
					.filter_map(|object_for_file_identifier::Data { pub_id, file_paths }| {
						file_paths
							.first()
							.and_then(|file_path| file_path.cas_id.as_ref())
							.map(|cas_id| (cas_id.into(), pub_id.into()))
					})
					.collect()
			})
	}

	let stringed_cas_ids = cas_ids.into_iter().map(Into::into).collect::<Vec<_>>();

	trace!(
		cas_ids_count = stringed_cas_ids.len(),
		"Fetching existing objects by cas_ids",
	);

	inner(stringed_cas_ids, db).await
}

/// Attempt to associate each file path with an object that has been
/// connected to file paths with the same cas_id
#[instrument(skip_all, err, fields(identified_files_count = file_paths_by_cas_id.len()))]
async fn assign_existing_objects_to_file_paths(
	file_paths_by_cas_id: &mut HashMap<CasId, Vec<FilePathToCreateOrLinkObject>>,
	objects_by_cas_id: &HashMap<CasId, ObjectPubId>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<u64, file_identifier::Error> {
	sync.write_ops(
		db,
		objects_by_cas_id
			.iter()
			.flat_map(|(cas_id, object_pub_id)| {
				file_paths_by_cas_id
					.remove(cas_id)
					.map(|file_paths| {
						file_paths.into_iter().map(
							|FilePathToCreateOrLinkObject {
							     file_path_pub_id, ..
							 }| {
								connect_file_path_to_object(
									&file_path_pub_id,
									object_pub_id,
									db,
									sync,
								)
							},
						)
					})
					.expect("must be here")
			})
			.unzip::<_, _, Vec<_>, Vec<_>>(),
	)
	.await
	.map(|file_paths| file_paths.len() as u64)
	.map_err(Into::into)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveState {
	id: TaskId,
	file_paths_by_cas_id: HashMap<CasId, Vec<FilePathToCreateOrLinkObject>>,
	stage: Stage,
	output: Output,
	with_priority: bool,
}

impl SerializableTask<Error> for ObjectProcessor {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, Arc<SyncManager>);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			file_paths_by_cas_id,
			stage,
			output,
			with_priority,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			id,
			file_paths_by_cas_id,
			stage,
			output,
			with_priority,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveState {
			     id,
			     file_paths_by_cas_id,
			     stage,
			     output,
			     with_priority,
			 }| Self {
				id,
				with_priority,
				file_paths_by_cas_id,
				stage,
				output,
				db,
				sync,
			},
		)
	}
}
