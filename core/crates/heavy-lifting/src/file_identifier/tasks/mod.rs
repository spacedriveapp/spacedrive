use crate::file_identifier;

use sd_core_prisma_helpers::{file_path_id, FilePathPubId, ObjectPubId};
use sd_core_sync::Manager as SyncManager;

use sd_file_ext::kind::ObjectKind;
use sd_prisma::{
	prisma::{file_path, object, PrismaClient},
	prisma_sync,
};
use sd_sync::{CRDTOperation, OperationFactory};
use sd_utils::msgpack;

use chrono::{DateTime, FixedOffset};
use prisma_client_rust::Select;
use serde::{Deserialize, Serialize};
use tracing::{instrument, trace, Level};

pub mod identifier;
pub mod object_processor;

pub use identifier::Identifier;
pub use object_processor::ObjectProcessor;

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
			file_path::object::NAME,
			msgpack!(prisma_sync::object::SyncId {
				pub_id: object_pub_id.to_db(),
			}),
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
) -> Result<Vec<file_path::id::Type>, file_identifier::Error> {
	trace!("Creating new Objects!");

	let (object_create_args, file_path_update_args) = files_and_kinds
		.into_iter()
		.map(
			|FilePathToCreateOrLinkObject {
			     file_path_pub_id,
			     created_at,
			     kind,
			     ..
			 }| {
				let object_pub_id = ObjectPubId::new();

				let kind = kind as i32;

				let (sync_params, db_params) = [
					(
						(object::date_created::NAME, msgpack!(created_at)),
						object::date_created::set(created_at),
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
								pub_id: object_pub_id.to_db(),
							},
							sync_params,
						),
						object::create_unchecked(object_pub_id.to_db(), db_params),
					),
					connect_file_path_to_object(&file_path_pub_id, &object_pub_id, db, sync),
				)
			},
		)
		.unzip::<_, _, Vec<_>, Vec<_>>();

	// create new object records with assembled values
	let created_objects_count = sync
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

	trace!(%created_objects_count, "Created new Objects");

	if created_objects_count > 0 {
		trace!("Updating file paths with created objects");

		sync.write_ops(
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
				.collect()
		})
		.map_err(Into::into)
	} else {
		trace!("No objects created, skipping file path updates");
		Ok(vec![])
	}
}
