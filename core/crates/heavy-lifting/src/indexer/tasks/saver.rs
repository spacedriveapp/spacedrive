use crate::{indexer::IndexerError, Error};

use sd_core_file_path_helper::IsolatedFilePathDataParts;
use sd_core_sync::Manager as SyncManager;

use sd_prisma::{
	prisma::{file_path, location, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_entry, OperationFactory};
use sd_task_system::{ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId};
use sd_utils::{db::inode_to_db, msgpack};

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::trace;

use super::walker::WalkedEntry;

#[derive(Debug)]
pub struct SaveTask {
	id: TaskId,
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	walked_entries: Vec<WalkedEntry>,
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
	is_shallow: bool,
}

impl SaveTask {
	#[must_use]
	pub fn new_deep(
		location_id: location::id::Type,
		location_pub_id: location::pub_id::Type,
		walked_entries: Vec<WalkedEntry>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			location_id,
			location_pub_id,
			walked_entries,
			db,
			sync,
			is_shallow: false,
		}
	}

	#[must_use]
	pub fn new_shallow(
		location_id: location::id::Type,
		location_pub_id: location::pub_id::Type,
		walked_entries: Vec<WalkedEntry>,
		db: Arc<PrismaClient>,
		sync: Arc<SyncManager>,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			location_id,
			location_pub_id,
			walked_entries,
			db,
			sync,
			is_shallow: true,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveTaskSaveState {
	id: TaskId,
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	walked_entries: Vec<WalkedEntry>,
	is_shallow: bool,
}

impl SerializableTask<Error> for SaveTask {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, Arc<SyncManager>);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			location_id,
			location_pub_id,
			walked_entries,
			is_shallow,
			..
		} = self;
		rmp_serde::to_vec_named(&SaveTaskSaveState {
			id,
			location_id,
			location_pub_id,
			walked_entries,
			is_shallow,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveTaskSaveState {
			     id,
			     location_id,
			     location_pub_id,
			     walked_entries,
			     is_shallow,
			 }| Self {
				id,
				location_id,
				location_pub_id,
				walked_entries,
				db,
				sync,
				is_shallow,
			},
		)
	}
}

#[derive(Debug)]
pub struct SaveTaskOutput {
	pub saved_count: u64,
	pub save_duration: Duration,
}

#[async_trait::async_trait]
impl Task<Error> for SaveTask {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		// If we're running in shallow mode, then we want priority
		self.is_shallow
	}

	async fn run(&mut self, _: &Interrupter) -> Result<ExecStatus, Error> {
		use file_path::{
			create_unchecked, date_created, date_indexed, date_modified, extension, hidden, inode,
			is_dir, location, location_id, materialized_path, name, size_in_bytes_bytes,
		};

		let start_time = Instant::now();

		let Self {
			location_id,
			location_pub_id,
			walked_entries,
			db,
			sync,
			..
		} = self;

		let (sync_stuff, paths): (Vec<_>, Vec<_>) = walked_entries
			.drain(..)
			.map(|entry| {
				let IsolatedFilePathDataParts {
					materialized_path,
					is_dir,
					name,
					extension,
					..
				} = entry.iso_file_path.to_parts();

				let pub_id = sd_utils::uuid_to_bytes(entry.pub_id);

				let (sync_params, db_params): (Vec<_>, Vec<_>) = [
					(
						(
							location::NAME,
							msgpack!(prisma_sync::location::SyncId {
								pub_id: location_pub_id.clone()
							}),
						),
						location_id::set(Some(*location_id)),
					),
					sync_db_entry!(materialized_path.to_string(), materialized_path),
					sync_db_entry!(name.to_string(), name),
					sync_db_entry!(is_dir, is_dir),
					sync_db_entry!(extension.to_string(), extension),
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
					{
						let v = Utc::now().into();
						sync_db_entry!(v, date_indexed)
					},
					sync_db_entry!(entry.metadata.hidden, hidden),
				]
				.into_iter()
				.unzip();

				(
					sync.shared_create(
						prisma_sync::file_path::SyncId {
							pub_id: sd_utils::uuid_to_bytes(entry.pub_id),
						},
						sync_params,
					),
					create_unchecked(pub_id, db_params),
				)
			})
			.unzip();

		#[allow(clippy::cast_sign_loss)]
		let saved_count = sync
			.write_ops(
				db,
				(
					sync_stuff.into_iter().flatten().collect(),
					db.file_path().create_many(paths).skip_duplicates(),
				),
			)
			.await
			.map_err(IndexerError::from)? as u64;

		trace!("Inserted {saved_count} records");

		Ok(ExecStatus::Done(
			SaveTaskOutput {
				saved_count,
				save_duration: start_time.elapsed(),
			}
			.into_output(),
		))
	}
}
