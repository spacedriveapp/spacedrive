use crate::{file_identifier::FileIdentifierError, Error};

use sd_core_prisma_helpers::{
	file_path_for_file_identifier, file_path_pub_id, object_for_file_identifier,
};
use sd_core_sync::Manager as SyncManager;

use sd_prisma::{
	prisma::{file_path, object, PrismaClient},
	prisma_sync,
};
use sd_sync::{CRDTOperation, OperationFactory};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};
use sd_utils::{msgpack, uuid_to_bytes};

use std::{
	collections::{HashMap, HashSet},
	mem,
	sync::Arc,
	time::Duration,
};

use prisma_client_rust::Select;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{debug, trace};
use uuid::Uuid;

use super::IdentifiedFile;

#[derive(Debug)]
pub struct ObjectProcessorTask {
	id: TaskId,
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
	identified_files: HashMap<Uuid, IdentifiedFile>,
	metrics: ObjectProcessorTaskMetrics,
	stage: Stage,
	is_shallow: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveState {
	id: TaskId,
	identified_files: HashMap<Uuid, IdentifiedFile>,
	metrics: ObjectProcessorTaskMetrics,
	stage: Stage,
	is_shallow: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ObjectProcessorTaskMetrics {
	pub assign_cas_ids_time: Duration,
	pub fetch_existing_objects_time: Duration,
	pub assign_to_existing_object_time: Duration,
	pub create_object_time: Duration,
	pub created_objects_count: u64,
	pub linked_objects_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
enum Stage {
	Starting,
	FetchExistingObjects,
	AssignFilePathsToExistingObjects {
		existing_objects_by_cas_id: HashMap<String, object_for_file_identifier::Data>,
	},
	CreateObjects,
}

impl ObjectProcessorTask {
	fn new(
		identified_files: HashMap<Uuid, IdentifiedFile>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
		is_shallow: bool,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			db,
			sync,
			identified_files,
			stage: Stage::Starting,
			metrics: ObjectProcessorTaskMetrics::default(),
			is_shallow,
		}
	}

	pub fn new_deep(
		identified_files: HashMap<Uuid, IdentifiedFile>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
	) -> Self {
		Self::new(identified_files, db, sync, false)
	}

	pub fn new_shallow(
		identified_files: HashMap<Uuid, IdentifiedFile>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
	) -> Self {
		Self::new(identified_files, db, sync, true)
	}
}

#[async_trait::async_trait]
impl Task<Error> for ObjectProcessorTask {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		self.is_shallow
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		let Self {
			db,
			sync,
			identified_files,
			stage,
			metrics:
				ObjectProcessorTaskMetrics {
					assign_cas_ids_time,
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
					let start = Instant::now();
					assign_cas_id_to_file_paths(identified_files, db, sync).await?;
					*assign_cas_ids_time = start.elapsed();
					*stage = Stage::FetchExistingObjects;
				}

				Stage::FetchExistingObjects => {
					let start = Instant::now();
					let existing_objects_by_cas_id =
						fetch_existing_objects_by_cas_id(identified_files, db).await?;
					*fetch_existing_objects_time = start.elapsed();
					*stage = Stage::AssignFilePathsToExistingObjects {
						existing_objects_by_cas_id,
					};
				}

				Stage::AssignFilePathsToExistingObjects {
					existing_objects_by_cas_id,
				} => {
					let start = Instant::now();
					let assigned_file_path_pub_ids = assign_existing_objects_to_file_paths(
						identified_files,
						existing_objects_by_cas_id,
						db,
						sync,
					)
					.await?;
					*assign_to_existing_object_time = start.elapsed();
					*linked_objects_count = assigned_file_path_pub_ids.len() as u64;

					debug!(
						"Found {} existing Objects, linked file paths to them",
						existing_objects_by_cas_id.len()
					);

					for file_path_pub_id::Data { pub_id } in assigned_file_path_pub_ids {
						let pub_id = Uuid::from_slice(&pub_id).expect("uuid bytes are invalid");
						trace!("Assigned file path <file_path_pub_id={pub_id}> to existing object");

						identified_files
							.remove(&pub_id)
							.expect("file_path must be here");
					}

					*stage = Stage::CreateObjects;

					if identified_files.is_empty() {
						// No objects to be created, we're good to finish already
						break;
					}
				}

				Stage::CreateObjects => {
					let start = Instant::now();
					*created_objects_count = create_objects(identified_files, db, sync).await?;
					*create_object_time = start.elapsed();

					break;
				}
			}

			check_interruption!(interrupter);
		}

		Ok(ExecStatus::Done(mem::take(&mut self.metrics).into_output()))
	}
}

async fn assign_cas_id_to_file_paths(
	identified_files: &HashMap<Uuid, IdentifiedFile>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<(), FileIdentifierError> {
	// Assign cas_id to each file path
	sync.write_ops(
		db,
		identified_files
			.iter()
			.map(|(pub_id, IdentifiedFile { cas_id, .. })| {
				(
					sync.shared_update(
						prisma_sync::file_path::SyncId {
							pub_id: uuid_to_bytes(*pub_id),
						},
						file_path::cas_id::NAME,
						msgpack!(cas_id),
					),
					db.file_path()
						.update(
							file_path::pub_id::equals(uuid_to_bytes(*pub_id)),
							vec![file_path::cas_id::set(cas_id.clone())],
						)
						// We don't need any data here, just the id avoids receiving the entire object
						// as we can't pass an empty select macro call
						.select(file_path::select!({ id })),
				)
			})
			.unzip::<_, _, _, Vec<_>>(),
	)
	.await?;

	Ok(())
}

async fn fetch_existing_objects_by_cas_id(
	identified_files: &HashMap<Uuid, IdentifiedFile>,
	db: &PrismaClient,
) -> Result<HashMap<String, object_for_file_identifier::Data>, FileIdentifierError> {
	// Retrieves objects that are already connected to file paths with the same id
	db.object()
		.find_many(vec![object::file_paths::some(vec![
			file_path::cas_id::in_vec(
				identified_files
					.values()
					.filter_map(|IdentifiedFile { cas_id, .. }| cas_id.as_ref())
					.cloned()
					.collect::<HashSet<_>>()
					.into_iter()
					.collect(),
			),
		])])
		.select(object_for_file_identifier::select())
		.exec()
		.await
		.map_err(Into::into)
		.map(|objects| {
			objects
				.into_iter()
				.filter_map(|object| {
					object
						.file_paths
						.first()
						.and_then(|file_path| file_path.cas_id.clone())
						.map(|cas_id| (cas_id, object))
				})
				.collect()
		})
}

async fn assign_existing_objects_to_file_paths(
	identified_files: &HashMap<Uuid, IdentifiedFile>,
	objects_by_cas_id: &HashMap<String, object_for_file_identifier::Data>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<Vec<file_path_pub_id::Data>, FileIdentifierError> {
	// Attempt to associate each file path with an object that has been
	// connected to file paths with the same cas_id
	sync.write_ops(
		db,
		identified_files
			.iter()
			.filter_map(|(pub_id, IdentifiedFile { cas_id, .. })| {
				objects_by_cas_id
					// Filtering out files without cas_id due to being empty
					.get(cas_id.as_ref()?)
					.map(|object| (*pub_id, object))
			})
			.map(|(pub_id, object)| {
				connect_file_path_to_object(
					pub_id,
					// SAFETY: This pub_id is generated by the uuid lib, but we have to store bytes in sqlite
					Uuid::from_slice(&object.pub_id).expect("uuid bytes are invalid"),
					sync,
					db,
				)
			})
			.unzip::<_, _, Vec<_>, Vec<_>>(),
	)
	.await
	.map_err(Into::into)
}

fn connect_file_path_to_object<'db>(
	file_path_pub_id: Uuid,
	object_pub_id: Uuid,
	sync: &SyncManager,
	db: &'db PrismaClient,
) -> (CRDTOperation, Select<'db, file_path_pub_id::Data>) {
	trace!("Connecting <file_path_pub_id={file_path_pub_id}> to <object_pub_id={object_pub_id}'>");

	let vec_id = object_pub_id.as_bytes().to_vec();

	(
		sync.shared_update(
			prisma_sync::file_path::SyncId {
				pub_id: uuid_to_bytes(file_path_pub_id),
			},
			file_path::object::NAME,
			msgpack!(prisma_sync::object::SyncId {
				pub_id: vec_id.clone()
			}),
		),
		db.file_path()
			.update(
				file_path::pub_id::equals(uuid_to_bytes(file_path_pub_id)),
				vec![file_path::object::connect(object::pub_id::equals(vec_id))],
			)
			.select(file_path_pub_id::select()),
	)
}

async fn create_objects(
	identified_files: &HashMap<Uuid, IdentifiedFile>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<u64, FileIdentifierError> {
	trace!("Creating {} new Objects", identified_files.len(),);

	let (object_create_args, file_path_update_args) = identified_files
		.iter()
		.map(
			|(
				file_path_pub_id,
				IdentifiedFile {
					file_path: file_path_for_file_identifier::Data { date_created, .. },
					kind,
					..
				},
			)| {
				let object_pub_id = Uuid::new_v4();

				let kind = *kind as i32;

				let (sync_params, db_params) = [
					(
						(object::date_created::NAME, msgpack!(date_created)),
						object::date_created::set(*date_created),
					),
					(
						(object::kind::NAME, msgpack!(kind)),
						object::kind::set(Some(kind)),
					),
				]
				.into_iter()
				.unzip::<_, _, Vec<_>, Vec<_>>();

				(
					(
						sync.shared_create(
							prisma_sync::object::SyncId {
								pub_id: uuid_to_bytes(object_pub_id),
							},
							sync_params,
						),
						object::create_unchecked(uuid_to_bytes(object_pub_id), db_params),
					),
					connect_file_path_to_object(*file_path_pub_id, object_pub_id, sync, db),
				)
			},
		)
		.unzip::<_, _, Vec<_>, Vec<_>>();

	// create new object records with assembled values
	let total_created_files = sync
		.write_ops(db, {
			let (sync, db_params) = object_create_args
				.into_iter()
				.unzip::<_, _, Vec<_>, Vec<_>>();

			(
				sync.into_iter().flatten().collect(),
				db.object().create_many(db_params),
			)
		})
		.await?;

	trace!("Created {total_created_files} new Objects");

	if total_created_files > 0 {
		trace!("Updating file paths with created objects");

		sync.write_ops(
			db,
			file_path_update_args
				.into_iter()
				.unzip::<_, _, Vec<_>, Vec<_>>(),
		)
		.await?;

		trace!("Updated file paths with created objects");
	}

	#[allow(clippy::cast_sign_loss)] // SAFETY: We're sure the value is positive
	Ok(total_created_files as u64)
}

impl SerializableTask<Error> for ObjectProcessorTask {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, Arc<SyncManager>);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			identified_files,
			metrics,
			stage,
			is_shallow,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			id,
			identified_files,
			metrics,
			stage,
			is_shallow,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveState {
			     id,
			     identified_files,
			     metrics,
			     stage,
			     is_shallow,
			 }| Self {
				id,
				db,
				sync,
				identified_files,
				metrics,
				stage,
				is_shallow,
			},
		)
	}
}
