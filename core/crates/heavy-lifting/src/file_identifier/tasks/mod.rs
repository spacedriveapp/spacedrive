use crate::file_identifier;

use sd_core_prisma_helpers::{file_path_id, FilePathPubId, ObjectPubId};
use sd_core_sync::SyncManager;

use sd_file_ext::kind::ObjectKind;
use sd_prisma::{
	prisma::{device, file_path, object, PrismaClient},
	prisma_sync,
};
use sd_sync::{option_sync_db_entry, sync_db_entry, sync_entry, CRDTOperation, OperationFactory};
use sd_utils::chain_optional_iter;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, FixedOffset};
use prisma_client_rust::Select;
use serde::{Deserialize, Serialize};
use tracing::{instrument, trace, Level};

pub mod identifier;
pub mod object_processor;

pub use identifier::Identifier;
pub use object_processor::ObjectProcessor;

/// This object has all needed data to create a new `object` for a `file_path` or link an existing one.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct FilePathToCreateOrLinkObject {
	id: file_path::id::Type,
	file_path_pub_id: FilePathPubId,
	kind: ObjectKind,
	created_at: Option<DateTime<FixedOffset>>,
}

#[instrument(skip(sync, db))]
fn connect_file_path_to_object<'db>(
	file_path_pub_id: &FilePathPubId,
	object_pub_id: &ObjectPubId,
	db: &'db PrismaClient,
	sync: &SyncManager,
) -> (CRDTOperation, Select<'db, file_path_id::Data>) {
	trace!("Connecting");

	(
		sync.shared_update(
			prisma_sync::file_path::SyncId {
				pub_id: file_path_pub_id.to_db(),
			},
			[sync_entry!(
				prisma_sync::object::SyncId {
					pub_id: object_pub_id.to_db(),
				},
				file_path::object
			)],
		),
		db.file_path()
			.update(
				file_path::pub_id::equals(file_path_pub_id.to_db()),
				vec![file_path::object::connect(object::pub_id::equals(
					object_pub_id.to_db(),
				))],
			)
			// selecting just id to avoid fetching the whole object
			.select(file_path_id::select()),
	)
}

#[instrument(skip_all, ret(level = Level::TRACE), err)]
async fn create_objects_and_update_file_paths(
	files_and_kinds: impl IntoIterator<Item = FilePathToCreateOrLinkObject> + Send,
	db: &PrismaClient,
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<HashMap<file_path::id::Type, ObjectPubId>, file_identifier::Error> {
	trace!("Preparing objects");
	let (object_create_args, file_path_args) = files_and_kinds
		.into_iter()
		.map(
			|FilePathToCreateOrLinkObject {
			     id,
			     file_path_pub_id,
			     kind,
			     created_at,
			 }| {
				let object_pub_id = ObjectPubId::new();

				let kind = kind as i32;

				let device_pub_id = sync.device_pub_id.to_db();

				let (sync_params, db_params) = chain_optional_iter(
					[
						(
							sync_entry!(
								prisma_sync::device::SyncId {
									pub_id: device_pub_id,
								},
								object::device
							),
							object::device_id::set(Some(device_id)),
						),
						sync_db_entry!(kind, object::kind),
					],
					[option_sync_db_entry!(created_at, object::date_created)],
				)
				.into_iter()
				.unzip::<_, _, Vec<_>, Vec<_>>();

				(
					(
						sync.shared_create(
							prisma_sync::object::SyncId {
								pub_id: object_pub_id.to_db(),
							},
							sync_params,
						),
						object::create_unchecked(object_pub_id.to_db(), db_params),
					),
					(
						(id, object_pub_id.clone()),
						connect_file_path_to_object(&file_path_pub_id, &object_pub_id, db, sync),
					),
				)
			},
		)
		.unzip::<_, _, Vec<_>, Vec<_>>();

	let (mut object_pub_id_by_file_path_id, file_path_update_args) = file_path_args
		.into_iter()
		.unzip::<_, _, HashMap<_, _>, Vec<_>>(
	);

	let new_objects_count = object_create_args.len();
	if new_objects_count > 0 {
		trace!(new_objects_count, "Creating new Objects!;",);

		// create new object records with assembled values
		let created_objects_count = sync
			.write_ops(db, {
				let (sync, db_params) = object_create_args
					.into_iter()
					.unzip::<_, _, Vec<_>, Vec<_>>();

				(sync, db.object().create_many(db_params))
			})
			.await?;

		trace!(%created_objects_count, "Created new Objects;");

		if created_objects_count > 0 {
			let file_paths_to_update_count = file_path_update_args.len();
			if file_paths_to_update_count > 0 {
				trace!(
					file_paths_to_update_count,
					"Updating file paths with created objects"
				);

				let updated_file_path_ids = sync
					.write_ops(
						db,
						file_path_update_args
							.into_iter()
							.unzip::<_, _, Vec<_>, Vec<_>>(),
					)
					.await
					.map(|file_paths| {
						file_paths
							.into_iter()
							.map(|file_path_id::Data { id }| id)
							.collect::<HashSet<_>>()
					})?;

				object_pub_id_by_file_path_id
					.retain(|file_path_id, _| updated_file_path_ids.contains(file_path_id));
			}

			Ok(object_pub_id_by_file_path_id)
		} else {
			trace!("No objects created, skipping file path updates");
			Ok(HashMap::new())
		}
	} else {
		trace!("No objects to create, skipping file path updates");
		Ok(HashMap::new())
	}
}
