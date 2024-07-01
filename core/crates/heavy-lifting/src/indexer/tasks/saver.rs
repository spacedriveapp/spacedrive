use crate::{indexer, Error};

use sd_core_file_path_helper::{FilePathMetadata, IsolatedFilePathDataParts};
use sd_core_sync::Manager as SyncManager;

use sd_prisma::{
	prisma::{file_path, location, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_entry, OperationFactory};
use sd_task_system::{ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId};
use sd_utils::{
	db::{inode_to_db, size_in_bytes_to_db},
	msgpack,
};

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{instrument, trace, Level};

use super::walker::WalkedEntry;

#[derive(Debug)]
pub struct Saver {
	// Task control
	id: TaskId,
	is_shallow: bool,

	// Received input args
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	walked_entries: Vec<WalkedEntry>,

	// Dependencies
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
}

/// [`Save`] Task output
#[derive(Debug)]
pub struct Output {
	/// Number of records inserted on database
	pub saved_count: u64,
	/// Time spent saving records
	pub save_duration: Duration,
}

#[async_trait::async_trait]
impl Task<Error> for Saver {
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
			location_id = %self.location_id,
			to_save_count = %self.walked_entries.len(),
			is_shallow = self.is_shallow,
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
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
					let IsolatedFilePathDataParts {
						materialized_path,
						is_dir,
						name,
						extension,
						..
					} = iso_file_path.to_parts();

					assert!(
						maybe_object_id.is_none(),
						"Object ID must be None as this tasks only created \
						new file_paths and they were not identified yet"
					);

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
						sync_db_entry!(materialized_path, materialized_path),
						sync_db_entry!(name, name),
						sync_db_entry!(is_dir, is_dir),
						sync_db_entry!(extension, extension),
						sync_db_entry!(size_in_bytes_to_db(size_in_bytes), size_in_bytes_bytes),
						sync_db_entry!(inode_to_db(inode), inode),
						sync_db_entry!(created_at, date_created),
						sync_db_entry!(modified_at, date_modified),
						sync_db_entry!(Utc::now(), date_indexed),
						sync_db_entry!(hidden, hidden),
					]
					.into_iter()
					.unzip();

					(
						sync.shared_create(
							prisma_sync::file_path::SyncId {
								pub_id: pub_id.to_db(),
							},
							sync_params,
						),
						create_unchecked(pub_id.into(), db_params),
					)
				},
			)
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
			.map_err(indexer::Error::from)? as u64;

		let save_duration = start_time.elapsed();

		trace!(saved_count, "Inserted records;");

		Ok(ExecStatus::Done(
			Output {
				saved_count,
				save_duration,
			}
			.into_output(),
		))
	}
}

impl Saver {
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
struct SaveState {
	id: TaskId,
	is_shallow: bool,

	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	walked_entries: Vec<WalkedEntry>,
}

impl SerializableTask<Error> for Saver {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, Arc<SyncManager>);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			is_shallow,
			location_id,
			location_pub_id,
			walked_entries,
			..
		} = self;
		rmp_serde::to_vec_named(&SaveState {
			id,
			is_shallow,
			location_id,
			location_pub_id,
			walked_entries,
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
			     location_id,
			     location_pub_id,
			     walked_entries,
			 }| Self {
				id,
				is_shallow,
				location_id,
				location_pub_id,
				walked_entries,
				db,
				sync,
			},
		)
	}
}
