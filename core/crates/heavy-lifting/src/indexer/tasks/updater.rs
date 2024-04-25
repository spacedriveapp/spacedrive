use crate::{indexer::IndexerError, Error};

use sd_core_file_path_helper::IsolatedFilePathDataParts;
use sd_core_sync::Manager as SyncManager;

use sd_prisma::{
	prisma::{file_path, object, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_entry, OperationFactory};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};
use sd_utils::{chain_optional_iter, db::inode_to_db, msgpack};

use std::{collections::HashSet, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::trace;

use super::walker::WalkedEntry;

#[derive(Debug)]
pub struct UpdateTask {
	id: TaskId,
	walked_entries: Vec<WalkedEntry>,
	object_ids_that_should_be_unlinked: HashSet<object::id::Type>,
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
	is_shallow: bool,
}

impl UpdateTask {
	#[must_use]
	pub fn new_deep(
		walked_entries: Vec<WalkedEntry>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			walked_entries,
			db,
			sync,
			object_ids_that_should_be_unlinked: HashSet::new(),
			is_shallow: false,
		}
	}

	#[must_use]
	pub fn new_shallow(
		walked_entries: Vec<WalkedEntry>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			walked_entries,
			db,
			sync,
			object_ids_that_should_be_unlinked: HashSet::new(),
			is_shallow: true,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateTaskSaveState {
	id: TaskId,
	walked_entries: Vec<WalkedEntry>,
	object_ids_that_should_be_unlinked: HashSet<object::id::Type>,
	is_shallow: bool,
}

impl SerializableTask<Error> for UpdateTask {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, Arc<SyncManager>);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			walked_entries,
			object_ids_that_should_be_unlinked,
			is_shallow,
			..
		} = self;

		rmp_serde::to_vec_named(&UpdateTaskSaveState {
			id,
			walked_entries,
			object_ids_that_should_be_unlinked,
			is_shallow,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|UpdateTaskSaveState {
			     id,
			     walked_entries,
			     object_ids_that_should_be_unlinked,
			     is_shallow,
			 }| Self {
				id,
				walked_entries,
				object_ids_that_should_be_unlinked,
				db,
				sync,
				is_shallow,
			},
		)
	}
}

#[derive(Debug)]
pub struct UpdateTaskOutput {
	pub updated_count: u64,
	pub update_duration: Duration,
}

#[async_trait::async_trait]
impl Task<Error> for UpdateTask {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		// If we're running in shallow mode, then we want priority
		self.is_shallow
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		use file_path::{
			cas_id, date_created, date_modified, hidden, inode, is_dir, object, object_id,
			size_in_bytes_bytes,
		};

		let start_time = Instant::now();

		let Self {
			walked_entries,
			db,
			sync,
			object_ids_that_should_be_unlinked,
			..
		} = self;

		fetch_objects_ids_to_unlink(walked_entries, object_ids_that_should_be_unlinked, db).await?;

		check_interruption!(interrupter);

		let (sync_stuff, paths_to_update) = walked_entries
			.drain(..)
			.map(|entry| {
				let IsolatedFilePathDataParts { is_dir, .. } = &entry.iso_file_path.to_parts();

				let pub_id = sd_utils::uuid_to_bytes(entry.pub_id);

				let should_unlink_object = entry.maybe_object_id.map_or(false, |object_id| {
					object_ids_that_should_be_unlinked.contains(&object_id)
				});

				let (sync_params, db_params) = chain_optional_iter(
					[
						((cas_id::NAME, msgpack!(nil)), cas_id::set(None)),
						sync_db_entry!(*is_dir, is_dir),
						sync_db_entry!(
							entry.metadata.size_in_bytes.to_be_bytes().to_vec(),
							size_in_bytes_bytes
						),
						sync_db_entry!(inode_to_db(entry.metadata.inode), inode),
						{
							let v = entry.metadata.created_at.into();
							sync_db_entry!(v, date_created)
						},
						{
							let v = entry.metadata.modified_at.into();
							sync_db_entry!(v, date_modified)
						},
						sync_db_entry!(entry.metadata.hidden, hidden),
					],
					[
						// As this file was updated while Spacedrive was offline, we mark the object_id and cas_id as null
						// So this file_path will be updated at file identifier job
						should_unlink_object
							.then_some(((object_id::NAME, msgpack!(nil)), object::disconnect())),
					],
				)
				.into_iter()
				.unzip::<_, _, Vec<_>, Vec<_>>();

				(
					sync_params
						.into_iter()
						.map(|(field, value)| {
							sync.shared_update(
								prisma_sync::file_path::SyncId {
									pub_id: pub_id.clone(),
								},
								field,
								value,
							)
						})
						.collect::<Vec<_>>(),
					db.file_path()
						.update(file_path::pub_id::equals(pub_id), db_params)
						.select(file_path::select!({ id })),
				)
			})
			.unzip::<_, _, Vec<_>, Vec<_>>();

		let updated = sync
			.write_ops(
				db,
				(sync_stuff.into_iter().flatten().collect(), paths_to_update),
			)
			.await
			.map_err(IndexerError::from)?;

		trace!("Updated {updated:?} records");

		Ok(ExecStatus::Done(
			UpdateTaskOutput {
				updated_count: updated.len() as u64,
				update_duration: start_time.elapsed(),
			}
			.into_output(),
		))
	}
}

async fn fetch_objects_ids_to_unlink(
	walked_entries: &[WalkedEntry],
	object_ids_that_should_be_unlinked: &mut HashSet<object::id::Type>,
	db: &PrismaClient,
) -> Result<(), IndexerError> {
	if object_ids_that_should_be_unlinked.is_empty() {
		// First we consult which file paths we should unlink
		let object_ids = walked_entries
			.iter()
			.filter_map(|entry| entry.maybe_object_id)
			.collect::<HashSet<_>>() // Removing possible duplicates
			.into_iter()
			.collect::<Vec<_>>();

		*object_ids_that_should_be_unlinked = db
			._batch(
				object_ids
					.iter()
					.map(|object_id| {
						db.file_path()
							.count(vec![file_path::object_id::equals(Some(*object_id))])
					})
					.collect::<Vec<_>>(),
			)
			.await?
			.into_iter()
			.zip(object_ids)
			.filter_map(|(count, object_id)| (count > 1).then_some(object_id))
			.collect::<HashSet<_>>();
	}

	Ok(())
}
