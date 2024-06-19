use crate::{indexer, Error};

use sd_core_file_path_helper::{FilePathMetadata, IsolatedFilePathDataParts};
use sd_core_sync::Manager as SyncManager;

use sd_prisma::{
	prisma::{file_path, object, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_entry, OperationFactory};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};
use sd_utils::{
	chain_optional_iter,
	db::{inode_to_db, size_in_bytes_to_db},
	msgpack,
};

use std::{collections::HashSet, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{instrument, trace, Level};

use super::walker::WalkedEntry;

#[derive(Debug)]
pub struct Updater {
	// Task control
	id: TaskId,
	is_shallow: bool,

	// Received input args
	walked_entries: Vec<WalkedEntry>,

	// Inner state
	object_ids_that_should_be_unlinked: HashSet<object::id::Type>,

	// Dependencies
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
}

/// [`Update`] Task output
#[derive(Debug)]
pub struct Output {
	/// Number of records updated on database
	pub updated_count: u64,
	/// Time spent updating records
	pub update_duration: Duration,
}

#[async_trait::async_trait]
impl Task<Error> for Updater {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		// If we're running in shallow mode, then we want priority
		self.is_shallow
	}

	#[instrument(
		skip_all,
		fields(
			task_id = %self.id,
			to_update_count = %self.walked_entries.len(),
			is_shallow = self.is_shallow,
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
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
			.map(
				|WalkedEntry {
				     pub_id,
				     maybe_object_id,
				     iso_file_path,
				     metadata:
				         FilePathMetadata {
				             inode,
				             size_in_bytes,
				             created_at,
				             modified_at,
				             hidden,
				         },
				 }| {
					let IsolatedFilePathDataParts { is_dir, .. } = &iso_file_path.to_parts();

					let should_unlink_object = maybe_object_id.map_or(false, |object_id| {
						object_ids_that_should_be_unlinked.contains(&object_id)
					});

					let (sync_params, db_params) = chain_optional_iter(
						[
							((cas_id::NAME, msgpack!(nil)), cas_id::set(None)),
							sync_db_entry!(*is_dir, is_dir),
							sync_db_entry!(size_in_bytes_to_db(size_in_bytes), size_in_bytes_bytes),
							sync_db_entry!(inode_to_db(inode), inode),
							sync_db_entry!(created_at, date_created),
							sync_db_entry!(modified_at, date_modified),
							sync_db_entry!(hidden, hidden),
						],
						[
							// As this file was updated while Spacedrive was offline, we mark the object_id and cas_id as null
							// So this file_path will be updated at file identifier job
							should_unlink_object.then_some((
								(object_id::NAME, msgpack!(nil)),
								object::disconnect(),
							)),
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
										pub_id: pub_id.to_db(),
									},
									field,
									value,
								)
							})
							.collect::<Vec<_>>(),
						db.file_path()
							.update(file_path::pub_id::equals(pub_id.into()), db_params)
							// selecting id to avoid fetching whole object from database
							.select(file_path::select!({ id })),
					)
				},
			)
			.unzip::<_, _, Vec<_>, Vec<_>>();

		let updated = sync
			.write_ops(
				db,
				(sync_stuff.into_iter().flatten().collect(), paths_to_update),
			)
			.await
			.map_err(indexer::Error::from)?;

		let update_duration = start_time.elapsed();

		trace!(?updated, "Updated records;");

		Ok(ExecStatus::Done(
			Output {
				updated_count: updated.len() as u64,
				update_duration,
			}
			.into_output(),
		))
	}
}

impl Updater {
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

async fn fetch_objects_ids_to_unlink(
	walked_entries: &[WalkedEntry],
	object_ids_that_should_be_unlinked: &mut HashSet<object::id::Type>,
	db: &PrismaClient,
) -> Result<(), indexer::Error> {
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

#[derive(Debug, Serialize, Deserialize)]
struct SaveState {
	id: TaskId,
	is_shallow: bool,

	walked_entries: Vec<WalkedEntry>,

	object_ids_that_should_be_unlinked: HashSet<object::id::Type>,
}

impl SerializableTask<Error> for Updater {
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

		rmp_serde::to_vec_named(&SaveState {
			id,
			is_shallow,
			walked_entries,
			object_ids_that_should_be_unlinked,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveState {
			     id,
			     is_shallow,
			     walked_entries,
			     object_ids_that_should_be_unlinked,
			 }| Self {
				id,
				is_shallow,
				walked_entries,
				object_ids_that_should_be_unlinked,
				db,
				sync,
			},
		)
	}
}
