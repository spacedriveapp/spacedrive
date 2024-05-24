use crate::file_identifier;

use chrono::{DateTime, FixedOffset};
use sd_core_prisma_helpers::{
	file_path_for_file_identifier, file_path_to_create_object, CasId, FilePathPubId, ObjectPubId,
};
use sd_core_sync::Manager as SyncManager;

use sd_file_ext::kind::ObjectKind;
use sd_prisma::{
	prisma::{file_path, object, PrismaClient},
	prisma_sync,
};
use sd_sync::OperationFactory;
use sd_utils::msgpack;

use serde::{Deserialize, Serialize};
use tracing::{instrument, trace, Level};

pub mod extract_file_metadata;
pub mod object_processor;

pub use extract_file_metadata::ExtractFileMetadataTask;
pub use object_processor::ObjectProcessorTask;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct IdentifiedFile {
	file_path: file_path_for_file_identifier::Data,
	cas_id: CasId,
	kind: ObjectKind,
}

impl IdentifiedFile {
	pub fn new(
		file_path: file_path_for_file_identifier::Data,
		cas_id: impl Into<CasId>,
		kind: ObjectKind,
	) -> Self {
		Self {
			file_path,
			cas_id: cas_id.into(),
			kind,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct ObjectToCreateOrLink {
	file_path_pub_id: FilePathPubId,
	kind: ObjectKind,
	created_at: Option<DateTime<FixedOffset>>,
}

#[instrument(skip_all, ret(level = Level::TRACE), err)]
async fn create_objects(
	files_and_kinds: impl IntoIterator<Item = &(file_path_to_create_object::Data, ObjectKind)>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<u64, file_identifier::Error> {
	trace!("Creating new Objects!");

	let (object_create_args, file_path_update_args) = files_and_kinds
		.into_iter()
		.map(
			|(
				file_path_to_create_object::Data {
					pub_id: file_path_pub_id,
					date_created,
				},
				kind,
			)| {
				let object_pub_id = ObjectPubId::new();

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
								pub_id: object_pub_id.to_db(),
							},
							sync_params,
						),
						object::create_unchecked(object_pub_id.to_db(), db_params),
					),
					(
						sync.shared_update(
							prisma_sync::file_path::SyncId {
								pub_id: file_path_pub_id.clone(),
							},
							file_path::object::NAME,
							msgpack!(prisma_sync::object::SyncId {
								pub_id: object_pub_id.to_db()
							}),
						),
						db.file_path()
							.update(
								file_path::pub_id::equals(file_path_pub_id.clone()),
								vec![file_path::object::connect(object::pub_id::equals(
									object_pub_id.into(),
								))],
							)
							// selecting just id to avoid fetching the whole object
							.select(file_path::select!({ id })),
					),
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

	trace!(%total_created_files, "Created new Objects");

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
