use crate::{
	job::JobError,
	library::Library,
	location::file_path_helper::{
		file_path_for_file_identifier, FilePathError, IsolatedFilePathData,
	},
	object::{cas::generate_cas_id, object_for_file_identifier},
	prisma::{file_path, location, object, PrismaClient},
	util::{db::maybe_missing, error::FileIOError},
};

use sd_file_ext::{extensions::Extension, kind::ObjectKind};

use sd_prisma::prisma_sync;
use sd_sync::{CRDTOperation, OperationFactory};
use sd_utils::uuid_to_bytes;

use std::{
	collections::{HashMap, HashSet},
	fmt::Debug,
	path::Path,
};

use futures::future::join_all;
use serde_json::json;
use tokio::fs;
use tracing::{error, trace};
use uuid::Uuid;

pub mod file_identifier_job;
mod shallow;

pub use shallow::*;

// we break these jobs into chunks of 100 to improve performance
const CHUNK_SIZE: usize = 100;

#[derive(thiserror::Error, Debug)]
pub enum FileIdentifierJobError {
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal Errors
	#[error(transparent)]
	FilePathError(#[from] FilePathError),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
	pub cas_id: Option<String>,
	pub kind: ObjectKind,
	pub fs_metadata: std::fs::Metadata,
}

impl FileMetadata {
	/// Assembles `create_unchecked` params for a given file path
	pub async fn new(
		location_path: impl AsRef<Path>,
		iso_file_path: &IsolatedFilePathData<'_>, // TODO: use dedicated CreateUnchecked type
	) -> Result<FileMetadata, FileIOError> {
		let path = location_path.as_ref().join(iso_file_path);

		let fs_metadata = fs::metadata(&path)
			.await
			.map_err(|e| FileIOError::from((&path, e)))?;

		assert!(
			!fs_metadata.is_dir(),
			"We can't generate cas_id for directories"
		);

		// derive Object kind
		let kind = Extension::resolve_conflicting(&path, false)
			.await
			.map(Into::into)
			.unwrap_or(ObjectKind::Unknown);

		let cas_id = if fs_metadata.len() != 0 {
			generate_cas_id(&path, fs_metadata.len())
				.await
				.map(Some)
				.map_err(|e| FileIOError::from((&path, e)))?
		} else {
			// We can't do shit with empty files
			None
		};

		trace!("Analyzed file: {path:?} {cas_id:?} {kind:?}");

		Ok(FileMetadata {
			cas_id,
			kind,
			fs_metadata,
		})
	}
}

async fn identifier_job_step(
	Library { db, sync, .. }: &Library,
	location: &location::Data,
	file_paths: &[file_path_for_file_identifier::Data],
) -> Result<(usize, usize), JobError> {
	let location_path = maybe_missing(&location.path, "location.path").map(Path::new)?;

	let file_paths_metadatas = join_all(
		file_paths
			.iter()
			.filter_map(|file_path| {
				IsolatedFilePathData::try_from((location.id, file_path))
					.map(|iso_file_path| (iso_file_path, file_path))
					.map_err(|e| error!("Failed to extract isolated file path data: {e:#?}"))
					.ok()
			})
			.map(|(iso_file_path, file_path)| async move {
				FileMetadata::new(&location_path, &iso_file_path)
					.await
					.map(|metadata| {
						(
							// SAFETY: This should never happen
							Uuid::from_slice(&file_path.pub_id)
								.expect("file_path.pub_id is invalid!"),
							(metadata, file_path),
						)
					})
					.map_err(|e| {
						if e.source
							.raw_os_error()
							.map(|code| code == 362)
							.unwrap_or(false)
						{
							error!("Attempted to extract metadata from on-demand file: {e:#?}");
						} else {
							error!("Failed to extract file metadata: {e:#?}")
						}
					})
					.ok()
			}),
	)
	.await
	.into_iter()
	.flatten()
	.collect::<HashMap<_, _>>();

	let unique_cas_ids = file_paths_metadatas
		.values()
		.filter_map(|(metadata, _)| metadata.cas_id.clone())
		.collect::<HashSet<_>>()
		.into_iter()
		.collect();

	// Assign cas_id to each file path
	sync.write_ops(
		db,
		file_paths_metadatas
			.iter()
			.map(|(pub_id, (metadata, _))| {
				(
					sync.shared_update(
						prisma_sync::file_path::SyncId {
							pub_id: sd_utils::uuid_to_bytes(*pub_id),
						},
						file_path::cas_id::NAME,
						json!(&metadata.cas_id),
					),
					db.file_path().update(
						file_path::pub_id::equals(sd_utils::uuid_to_bytes(*pub_id)),
						vec![file_path::cas_id::set(metadata.cas_id.clone())],
					),
				)
			})
			.unzip::<_, _, _, Vec<_>>(),
	)
	.await?;

	// Retrieves objects that are already connected to file paths with the same id
	let existing_objects = db
		.object()
		.find_many(vec![object::file_paths::some(vec![
			file_path::cas_id::in_vec(unique_cas_ids),
		])])
		.select(object_for_file_identifier::select())
		.exec()
		.await?;

	let existing_object_cas_ids = existing_objects
		.iter()
		.flat_map(|object| {
			object
				.file_paths
				.iter()
				.filter_map(|file_path| file_path.cas_id.as_ref())
		})
		.collect::<HashSet<_>>();

	// Attempt to associate each file path with an object that has been
	// connected to file paths with the same cas_id
	let updated_file_paths = sync
		.write_ops(
			db,
			file_paths_metadatas
				.iter()
				.filter_map(|(pub_id, (metadata, file_path))| {
					// Filtering out files without cas_id due to being empty
					metadata
						.cas_id
						.is_some()
						.then_some((pub_id, (metadata, file_path)))
				})
				.flat_map(|(pub_id, (metadata, _))| {
					existing_objects
						.iter()
						.find(|object| {
							object
								.file_paths
								.iter()
								.any(|file_path| file_path.cas_id == metadata.cas_id)
						})
						.map(|object| (*pub_id, object))
				})
				.map(|(pub_id, object)| {
					let (crdt_op, db_op) = file_path_object_connect_ops(
						pub_id,
						// SAFETY: This pub_id is generated by the uuid lib, but we have to store bytes in sqlite
						Uuid::from_slice(&object.pub_id).expect("uuid bytes are invalid"),
						sync,
						db,
					);

					(crdt_op, db_op.select(file_path::select!({ pub_id })))
				})
				.unzip::<_, _, Vec<_>, Vec<_>>(),
		)
		.await?;

	trace!(
		"Found {} existing Objects in Library, linking file paths...",
		existing_objects.len()
	);

	// extract objects that don't already exist in the database
	let file_paths_requiring_new_object = file_paths_metadatas
		.into_iter()
		.filter(|(_, (FileMetadata { cas_id, .. }, _))| {
			cas_id
				.as_ref()
				.map(|cas_id| !existing_object_cas_ids.contains(cas_id))
				.unwrap_or(true)
		})
		.collect::<Vec<_>>();

	let total_created = if !file_paths_requiring_new_object.is_empty() {
		trace!(
			"Creating {} new Objects in Library",
			file_paths_requiring_new_object.len(),
		);

		let (object_create_args, file_path_update_args): (Vec<_>, Vec<_>) =
			file_paths_requiring_new_object
				.iter()
				.map(
					|(
						file_path_pub_id,
						(
							FileMetadata { kind, .. },
							file_path_for_file_identifier::Data { date_created, .. },
						),
					)| {
						let object_pub_id = Uuid::new_v4();
						let sync_id = || prisma_sync::object::SyncId {
							pub_id: sd_utils::uuid_to_bytes(object_pub_id),
						};

						let kind = *kind as i32;

						let (sync_params, db_params): (Vec<_>, Vec<_>) = [
							(
								(object::date_created::NAME, json!(date_created)),
								object::date_created::set(*date_created),
							),
							(
								(object::kind::NAME, json!(kind)),
								object::kind::set(Some(kind)),
							),
						]
						.into_iter()
						.unzip();

						let object_creation_args = (
							sync.shared_create(sync_id(), sync_params),
							object::create_unchecked(uuid_to_bytes(object_pub_id), db_params),
						);

						(object_creation_args, {
							let (crdt_op, db_op) = file_path_object_connect_ops(
								*file_path_pub_id,
								object_pub_id,
								sync,
								db,
							);

							(crdt_op, db_op.select(file_path::select!({ pub_id })))
						})
					},
				)
				.unzip();

		// create new object records with assembled values
		let total_created_files = sync
			.write_ops(db, {
				let (sync, db_params): (Vec<_>, Vec<_>) = object_create_args.into_iter().unzip();

				(
					sync.into_iter().flatten().collect(),
					db.object().create_many(db_params),
				)
			})
			.await
			.unwrap_or_else(|e| {
				error!("Error inserting files: {:#?}", e);
				0
			});

		trace!("Created {} new Objects in Library", total_created_files);

		if total_created_files > 0 {
			trace!("Updating file paths with created objects");

			sync.write_ops(db, {
				let data: (Vec<_>, Vec<_>) = file_path_update_args.into_iter().unzip();

				data
			})
			.await?;

			trace!("Updated file paths with created objects");
		}

		total_created_files as usize
	} else {
		0
	};

	Ok((total_created, updated_file_paths.len()))
}

fn file_path_object_connect_ops<'db>(
	file_path_id: Uuid,
	object_id: Uuid,
	sync: &crate::sync::Manager,
	db: &'db PrismaClient,
) -> (CRDTOperation, file_path::UpdateQuery<'db>) {
	#[cfg(debug_assertions)]
	trace!("Connecting <FilePath id={file_path_id}> to <Object pub_id={object_id}'>");

	let vec_id = object_id.as_bytes().to_vec();

	(
		sync.shared_update(
			prisma_sync::file_path::SyncId {
				pub_id: sd_utils::uuid_to_bytes(file_path_id),
			},
			file_path::object::NAME,
			json!(prisma_sync::object::SyncId {
				pub_id: vec_id.clone()
			}),
		),
		db.file_path().update(
			file_path::pub_id::equals(sd_utils::uuid_to_bytes(file_path_id)),
			vec![file_path::object::connect(object::pub_id::equals(vec_id))],
		),
	)
}

async fn process_identifier_file_paths(
	location: &location::Data,
	file_paths: &[file_path_for_file_identifier::Data],
	step_number: usize,
	cursor: file_path::id::Type,
	library: &Library,
	orphan_count: usize,
) -> Result<(usize, usize, file_path::id::Type), JobError> {
	trace!(
		"Processing {:?} orphan Paths. ({} completed of {})",
		file_paths.len(),
		step_number,
		orphan_count
	);

	let (total_objects_created, total_objects_linked) =
		identifier_job_step(library, location, file_paths).await?;

	Ok((
		total_objects_created,
		total_objects_linked,
		// returns a new cursor to the last row of this chunk or the current one
		file_paths
			.last()
			.map(|last_row| last_row.id)
			.unwrap_or(cursor),
	))
}
