use crate::{utils::sub_path, OuterContext};

use sd_core_file_path_helper::{FilePathError, IsolatedFilePathData};
use sd_core_prisma_helpers::{
	file_path_pub_and_cas_ids, file_path_to_isolate_with_pub_id, file_path_walker,
};
use sd_core_sync::{DevicePubId, SyncManager};

use sd_prisma::{
	prisma::{file_path, indexer_rule, location, PrismaClient, SortOrder},
	prisma_sync,
};
use sd_sync::{sync_db_entry, OperationFactory};
use sd_utils::{
	db::{size_in_bytes_from_db, size_in_bytes_to_db, MissingFieldError},
	error::{FileIOError, NonUtf8PathError},
	from_bytes_to_uuid,
};

use std::{
	collections::{HashMap, HashSet},
	hash::BuildHasher,
	mem,
	path::{Path, PathBuf},
	sync::Arc,
};

use itertools::Itertools;
use prisma_client_rust::{operator::or, QueryError, Select};
use rspc::ErrorCode;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{instrument, warn};

pub mod job;
mod shallow;
mod tasks;

pub use shallow::shallow;

use tasks::walker;

/// `BATCH_SIZE` is the number of files to index at each task, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	// Not Found errors
	#[error("indexer rule not found: <id='{0}'>")]
	IndexerRuleNotFound(indexer_rule::id::Type),
	#[error(transparent)]
	SubPath(#[from] sub_path::Error),
	#[error("device not found: <device_pub_id='{0}'")]
	DeviceNotFound(DevicePubId),

	// Internal Errors
	#[error("database error: {0}")]
	Database(#[from] QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error(transparent)]
	IsoFilePath(#[from] FilePathError),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),
	#[error("missing field on database: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("failed to deserialized stored tasks for job resume: {0}")]
	DeserializeTasks(#[from] rmp_serde::decode::Error),

	// Mixed errors
	#[error(transparent)]
	Rules(#[from] sd_core_indexer_rules::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::IndexerRuleNotFound(_) => {
				Self::with_cause(ErrorCode::NotFound, e.to_string(), e)
			}

			Error::SubPath(sub_path_err) => sub_path_err.into(),

			Error::Rules(rule_err) => rule_err.into(),

			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NonCriticalIndexerError {
	#[error("failed to read directory entry: {0}")]
	FailedDirectoryEntry(String),
	#[error("failed to fetch metadata: {0}")]
	Metadata(String),
	#[error("error applying indexer rule: {0}")]
	IndexerRule(String),
	#[error("error trying to extract file path metadata from a file: {0}")]
	FilePathMetadata(String),
	#[error("failed to fetch file paths ids from existing files on database: {0}")]
	FetchAlreadyExistingFilePathIds(String),
	#[error("failed to fetch file paths to be removed from database: {0}")]
	FetchFilePathsToRemove(String),
	#[error("error constructing isolated file path: {0}")]
	IsoFilePath(String),
	#[error("failed to dispatch new task to keep walking a directory: {0}")]
	DispatchKeepWalking(String),
	#[error("missing file_path data on database: {0}")]
	MissingFilePathData(String),
}

fn chunk_db_queries<'db, 'iso>(
	iso_file_paths: impl IntoIterator<Item = &'iso IsolatedFilePathData<'iso>>,
	db: &'db PrismaClient,
) -> Vec<Select<'db, Vec<file_path_to_isolate_with_pub_id::Data>>> {
	iso_file_paths
		.into_iter()
		.chunks(200)
		.into_iter()
		.map(|paths_chunk| {
			db.file_path()
				.find_many(vec![or(paths_chunk
					.into_iter()
					.map(file_path::WhereParam::from)
					.collect())])
				.select(file_path_to_isolate_with_pub_id::select())
		})
		.collect::<Vec<_>>()
}

#[allow(clippy::missing_panics_doc)] // Can't actually panic as we use the hashmap to fetch entries from db
async fn update_directory_sizes(
	iso_paths_and_sizes: HashMap<IsolatedFilePathData<'_>, u64, impl BuildHasher + Send>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<(), Error> {
	let (ops, queries) = db
		._batch(chunk_db_queries(iso_paths_and_sizes.keys(), db))
		.await?
		.into_iter()
		.flatten()
		.map(|file_path| {
			let size_bytes = iso_paths_and_sizes
				.get(&IsolatedFilePathData::try_from(&file_path)?)
				.map(|size| size_in_bytes_to_db(*size))
				.expect("must be here");

			let (sync_param, db_param) = sync_db_entry!(size_bytes, file_path::size_in_bytes_bytes);

			Ok((
				sync.shared_update(
					prisma_sync::file_path::SyncId {
						pub_id: file_path.pub_id.clone(),
					},
					[sync_param],
				),
				db.file_path()
					.update(file_path::pub_id::equals(file_path.pub_id), vec![db_param])
					.select(file_path::select!({ id })),
			))
		})
		.collect::<Result<Vec<_>, Error>>()?
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

	if !ops.is_empty() && !queries.is_empty() {
		sync.write_ops(db, (ops, queries)).await?;
	}

	Ok(())
}

async fn update_location_size(
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	ctx: &impl OuterContext,
) -> Result<(), Error> {
	let db = ctx.db();
	let sync = ctx.sync();

	let total_size = size_in_bytes_to_db(
		db.file_path()
			.find_many(vec![
				file_path::location_id::equals(Some(location_id)),
				file_path::materialized_path::equals(Some("/".to_string())),
			])
			.select(file_path::select!({ size_in_bytes_bytes }))
			.exec()
			.await?
			.into_iter()
			.filter_map(|file_path| {
				file_path
					.size_in_bytes_bytes
					.map(|size_in_bytes_bytes| size_in_bytes_from_db(&size_in_bytes_bytes))
			})
			.sum::<u64>(),
	);

	let (sync_param, db_param) = sync_db_entry!(total_size, location::size_in_bytes);

	sync.write_op(
		db,
		sync.shared_update(
			prisma_sync::location::SyncId {
				pub_id: location_pub_id,
			},
			[sync_param],
		),
		db.location()
			.update(location::id::equals(location_id), vec![db_param])
			.select(location::select!({ id })),
	)
	.await?;

	ctx.invalidate_query("locations.list");
	ctx.invalidate_query("locations.get");

	Ok(())
}

async fn remove_non_existing_file_paths(
	to_remove: Vec<file_path_pub_and_cas_ids::Data>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<u64, Error> {
	#[allow(clippy::cast_sign_loss)]
	let (sync_params, db_params): (Vec<_>, Vec<_>) = to_remove
		.into_iter()
		.map(|file_path| {
			(
				sync.shared_delete(prisma_sync::file_path::SyncId {
					pub_id: file_path.pub_id,
				}),
				file_path.id,
			)
		})
		.unzip();

	if sync_params.is_empty() {
		return Ok(0);
	}

	sync.write_ops(
		db,
		(
			sync_params,
			db.file_path()
				.delete_many(vec![file_path::id::in_vec(db_params)]),
		),
	)
	.await
	.map(
		#[allow(clippy::cast_sign_loss)]
		|count| count as u64,
	)
	.map_err(Into::into)
}

#[instrument(
	skip(base_path, location_path, db, sync, errors),
	fields(
		base_path = %base_path.as_ref().display(),
		location_path = %location_path.as_ref().display(),
	),
	err,
)]
#[allow(clippy::missing_panics_doc)] // Can't actually panic as we only deal with directories
pub async fn reverse_update_directories_sizes(
	base_path: impl AsRef<Path> + Send,
	location_id: location::id::Type,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
	sync: &SyncManager,
	errors: &mut Vec<crate::NonCriticalError>,
) -> Result<(), Error> {
	let location_path = location_path.as_ref();

	let ancestors = base_path
		.as_ref()
		.ancestors()
		.take_while(|&ancestor| ancestor != location_path)
		.map(|ancestor| {
			IsolatedFilePathData::new(location_id, location_path, ancestor, true).map(
				|iso_file_path| {
					let materialized_path = iso_file_path
						.materialized_path_for_children()
						.expect("each ancestor is a directory");

					(iso_file_path, materialized_path)
				},
			)
		})
		.collect::<Result<HashMap<_, _>, _>>()?;

	let mut pub_id_by_ancestor_materialized_path = db
		._batch(chunk_db_queries(ancestors.keys(), db))
		.await?
		.into_iter()
		.flatten()
		.filter_map(|mut file_path| {
			let pub_id = mem::take(&mut file_path.pub_id);
			IsolatedFilePathData::try_from(file_path)
				.map_err(|e| {
					errors.push(
						NonCriticalIndexerError::MissingFilePathData(format!(
							"Found a file_path missing data: <pub_id='{:#?}'>, error: {e:#?}",
							from_bytes_to_uuid(&pub_id)
						))
						.into(),
					);
				})
				.map(|iso_file_path| {
					(
						iso_file_path
							.materialized_path_for_children()
							.expect("we know it's a directory"),
						(pub_id, 0),
					)
				})
				.ok()
		})
		.collect::<HashMap<_, _>>();

	compute_sizes(
		location_id,
		ancestors.values().cloned().collect(),
		&mut pub_id_by_ancestor_materialized_path,
		db,
		errors,
	)
	.await?;

	let (sync_ops, update_queries) = ancestors
		.into_values()
		.filter_map(|materialized_path| {
			if let Some((pub_id, size)) =
				pub_id_by_ancestor_materialized_path.remove(&materialized_path)
			{
				let size_bytes = size_in_bytes_to_db(size);

				let (sync_param, db_param) =
					sync_db_entry!(size_bytes, file_path::size_in_bytes_bytes);

				Some((
					sync.shared_update(
						prisma_sync::file_path::SyncId {
							pub_id: pub_id.clone(),
						},
						[sync_param],
					),
					db.file_path()
						.update(file_path::pub_id::equals(pub_id), vec![db_param])
						.select(file_path::select!({ id })),
				))
			} else {
				warn!("Got a missing ancestor for a file_path in the database, ignoring...");
				None
			}
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	if !sync_ops.is_empty() && !update_queries.is_empty() {
		sync.write_ops(db, (sync_ops, update_queries)).await?;
	}

	Ok(())
}

async fn compute_sizes(
	location_id: location::id::Type,
	materialized_paths: Vec<String>,
	pub_id_by_ancestor_materialized_path: &mut HashMap<String, (file_path::pub_id::Type, u64)>,
	db: &PrismaClient,
	errors: &mut Vec<crate::NonCriticalError>,
) -> Result<(), QueryError> {
	for file_path in db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(location_id)),
			file_path::materialized_path::in_vec(materialized_paths),
		])
		.select(file_path::select!({ pub_id materialized_path size_in_bytes_bytes }))
		.exec()
		.await?
	{
		if let Some(materialized_path) = file_path.materialized_path {
			if let Some((_, size)) =
				pub_id_by_ancestor_materialized_path.get_mut(&materialized_path)
			{
				*size += file_path.size_in_bytes_bytes.map_or_else(
					|| {
						warn!("Got a directory missing its size in bytes");
						0
					},
					|size_in_bytes_bytes| size_in_bytes_from_db(&size_in_bytes_bytes),
				);
			}
		} else {
			errors.push(
				NonCriticalIndexerError::MissingFilePathData(format!(
						"Corrupt database possessing a file_path entry without materialized_path: <pub_id='{:#?}'>",
						from_bytes_to_uuid(&file_path.pub_id)
					))
				.into(),
			);
		}
	}

	Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IsoFilePathFactory {
	pub location_id: location::id::Type,
	pub location_path: Arc<PathBuf>,
}

impl walker::IsoFilePathFactory for IsoFilePathFactory {
	fn build(
		&self,
		path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<IsolatedFilePathData<'static>, FilePathError> {
		IsolatedFilePathData::new(self.location_id, self.location_path.as_ref(), path, is_dir)
	}
}

#[derive(Debug, Clone)]
struct WalkerDBProxy {
	location_id: location::id::Type,
	db: Arc<PrismaClient>,
}

impl walker::WalkerDBProxy for WalkerDBProxy {
	async fn fetch_file_paths(
		&self,
		found_paths: Vec<file_path::WhereParam>,
	) -> Result<Vec<file_path_walker::Data>, Error> {
		// Each found path is a AND with 4 terms, and SQLite has a expression tree limit of 1000 terms
		// so we will use chunks of 200 just to be safe
		self.db
			._batch(
				found_paths
					.into_iter()
					.chunks(200)
					.into_iter()
					.map(|founds| {
						self.db
							.file_path()
							.find_many(vec![or(founds.collect::<Vec<_>>())])
							.select(file_path_walker::select())
					})
					.collect::<Vec<_>>(),
			)
			.await
			.map(|fetched| fetched.into_iter().flatten().collect::<Vec<_>>())
			.map_err(Into::into)
	}

	async fn fetch_file_paths_to_remove(
		&self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		mut existing_inodes: HashSet<Vec<u8>>,
		unique_location_id_materialized_path_name_extension_params: Vec<file_path::WhereParam>,
	) -> Result<Vec<file_path_pub_and_cas_ids::Data>, NonCriticalIndexerError> {
		// NOTE: This batch size can be increased if we wish to trade memory for more performance
		const BATCH_SIZE: i64 = 1000;

		let founds_ids = {
			let found_chunks = self
				.db
				._batch(
					unique_location_id_materialized_path_name_extension_params
						.into_iter()
						.chunks(200)
						.into_iter()
						.map(|unique_params| {
							self.db
								.file_path()
								.find_many(vec![or(unique_params.collect())])
								.select(file_path::select!({ id inode }))
						})
						.collect::<Vec<_>>(),
				)
				.await
				.map_err(|e| {
					NonCriticalIndexerError::FetchAlreadyExistingFilePathIds(e.to_string())
				})?;

			found_chunks
				.into_iter()
				.flatten()
				.map(|file_path| {
					if let Some(inode) = file_path.inode {
						existing_inodes.remove(&inode);
					}
					file_path.id
				})
				.collect::<HashSet<_>>()
		};

		let mut to_remove = vec![];
		let mut cursor = 1;

		loop {
			let materialized_path_param = file_path::materialized_path::equals(Some(
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("the received isolated file path must be from a directory"),
			));

			let found = self
				.db
				.file_path()
				.find_many(vec![
					file_path::location_id::equals(Some(self.location_id)),
					if existing_inodes.is_empty() {
						materialized_path_param
					} else {
						or(vec![
							materialized_path_param,
							file_path::inode::in_vec(existing_inodes.iter().cloned().collect()),
						])
					},
				])
				.order_by(file_path::id::order(SortOrder::Asc))
				.take(BATCH_SIZE)
				.cursor(file_path::id::equals(cursor))
				.select(file_path::select!({ id pub_id cas_id inode }))
				.exec()
				.await
				.map_err(|e| NonCriticalIndexerError::FetchFilePathsToRemove(e.to_string()))?;

			#[allow(clippy::cast_possible_truncation)] // Safe because we are using a constant
			let should_stop = found.len() < BATCH_SIZE as usize;

			if let Some(last) = found.last() {
				cursor = last.id;
			} else {
				break;
			}

			to_remove.extend(found.into_iter().filter_map(|file_path| {
				if let Some(inode) = file_path.inode {
					existing_inodes.remove(&inode);
				}

				(!founds_ids.contains(&file_path.id)).then_some(file_path_pub_and_cas_ids::Data {
					id: file_path.id,
					pub_id: file_path.pub_id,
					cas_id: file_path.cas_id,
				})
			}));

			if should_stop {
				break;
			}
		}

		Ok(to_remove)
	}
}
