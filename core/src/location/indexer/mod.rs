use crate::library::Library;

use sd_file_path_helper::{
	file_path_pub_and_cas_ids, FilePathError, IsolatedFilePathData, IsolatedFilePathDataParts,
};
use sd_prisma::{
	prisma::{file_path, location, object as prisma_object, PrismaClient},
	prisma_sync,
};
use sd_sync::*;
use sd_utils::{db::inode_to_db, error::FileIOError, from_bytes_to_uuid};

use std::{collections::HashMap, path::Path};

use chrono::Utc;
use futures_concurrency::future::TryJoin;
use itertools::Itertools;
use prisma_client_rust::operator::or;
use rspc::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tracing::{trace, warn};

use super::location_with_indexer_rules;

pub mod old_indexer_job;
mod old_shallow;
pub mod rules;
mod old_walk;

use rules::IndexerRuleError;
use old_walk::WalkedEntry;

pub use old_indexer_job::OldIndexerJobInit;
pub use old_shallow::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct OldIndexerJobSaveStep {
	chunk_idx: usize,
	walked: Vec<WalkedEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OldIndexerJobUpdateStep {
	chunk_idx: usize,
	to_update: Vec<WalkedEntry>,
}

/// Error type for the indexer module
#[derive(Error, Debug)]
pub enum IndexerError {
	// Not Found errors
	#[error("indexer rule not found: <id='{0}'>")]
	IndexerRuleNotFound(i32),
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal Errors
	#[error("Database Error: {}", .0.to_string())]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),

	// Mixed errors
	#[error(transparent)]
	IndexerRules(#[from] IndexerRuleError),
}

impl From<IndexerError> for rspc::Error {
	fn from(err: IndexerError) -> Self {
		match err {
			IndexerError::IndexerRuleNotFound(_) | IndexerError::SubPathNotFound(_) => {
				rspc::Error::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			IndexerError::IndexerRules(rule_err) => rule_err.into(),

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

async fn execute_indexer_save_step(
	location: &location_with_indexer_rules::Data,
	save_step: &OldIndexerJobSaveStep,
	library: &Library,
) -> Result<i64, IndexerError> {
	let Library { sync, db, .. } = library;

	let (sync_stuff, paths): (Vec<_>, Vec<_>) = save_step
		.walked
		.iter()
		.map(|entry| {
			let IsolatedFilePathDataParts {
				materialized_path,
				is_dir,
				name,
				extension,
				..
			} = &entry.iso_file_path.to_parts();

			use file_path::*;

			let pub_id = sd_utils::uuid_to_bytes(entry.pub_id);

			let (sync_params, db_params): (Vec<_>, Vec<_>) = [
				(
					(
						location::NAME,
						json!(prisma_sync::location::SyncId {
							pub_id: location.pub_id.clone()
						}),
					),
					location_id::set(Some(location.id)),
				),
				sync_db_entry!(materialized_path.to_string(), materialized_path),
				sync_db_entry!(name.to_string(), name),
				sync_db_entry!(*is_dir, is_dir),
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
				file_path::create_unchecked(pub_id, db_params),
			)
		})
		.unzip();

	let count = sync
		.write_ops(
			db,
			(
				sync_stuff.into_iter().flatten().collect(),
				db.file_path().create_many(paths).skip_duplicates(),
			),
		)
		.await?;

	trace!("Inserted {count} records");

	Ok(count)
}

async fn execute_indexer_update_step(
	update_step: &OldIndexerJobUpdateStep,
	Library { sync, db, .. }: &Library,
) -> Result<i64, IndexerError> {
	let (sync_stuff, paths_to_update): (Vec<_>, Vec<_>) = update_step
		.to_update
		.iter()
		.map(|entry| async move {
			let IsolatedFilePathDataParts { is_dir, .. } = &entry.iso_file_path.to_parts();

			let pub_id = sd_utils::uuid_to_bytes(entry.pub_id);

			let should_unlink_object = if let Some(object_id) = entry.maybe_object_id {
				db.object()
					.count(vec![prisma_object::id::equals(object_id)])
					.exec()
					.await? > 1
			} else {
				false
			};

			use file_path::*;

			let (sync_params, db_params): (Vec<_>, Vec<_>) = [
				// As this file was updated while Spacedrive was offline, we mark the object_id and cas_id as null
				// So this file_path will be updated at file identifier job
				should_unlink_object.then_some((
					(object_id::NAME, serde_json::Value::Null),
					object::disconnect(),
				)),
				Some(((cas_id::NAME, serde_json::Value::Null), cas_id::set(None))),
				Some(sync_db_entry!(*is_dir, is_dir)),
				Some(sync_db_entry!(
					entry.metadata.size_in_bytes.to_be_bytes().to_vec(),
					size_in_bytes_bytes
				)),
				Some(sync_db_entry!(inode_to_db(entry.metadata.inode), inode)),
				Some({
					let v = entry.metadata.created_at.into();
					sync_db_entry!(v, date_created)
				}),
				Some({
					let v = entry.metadata.modified_at.into();
					sync_db_entry!(v, date_modified)
				}),
				Some(sync_db_entry!(entry.metadata.hidden, hidden)),
			]
			.into_iter()
			.flatten()
			.unzip();

			Ok::<_, IndexerError>((
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
			))
		})
		.collect::<Vec<_>>()
		.try_join()
		.await?
		.into_iter()
		.unzip();

	let updated = sync
		.write_ops(
			db,
			(sync_stuff.into_iter().flatten().collect(), paths_to_update),
		)
		.await?;

	trace!("Updated {updated:?} records");

	Ok(updated.len() as i64)
}

fn iso_file_path_factory(
	location_id: location::id::Type,
	location_path: &Path,
) -> impl Fn(&Path, bool) -> Result<IsolatedFilePathData<'static>, IndexerError> + '_ {
	move |path, is_dir| {
		IsolatedFilePathData::new(location_id, location_path, path, is_dir).map_err(Into::into)
	}
}

async fn remove_non_existing_file_paths(
	to_remove: impl IntoIterator<Item = file_path_pub_and_cas_ids::Data>,
	db: &PrismaClient,
	sync: &sd_core_sync::Manager,
) -> Result<u64, IndexerError> {
	let (sync_params, db_params): (Vec<_>, Vec<_>) = to_remove
		.into_iter()
		.map(|d| {
			(
				sync.shared_delete(prisma_sync::file_path::SyncId { pub_id: d.pub_id }),
				d.id,
			)
		})
		.unzip();

	sync.write_ops(
		db,
		(
			sync_params,
			db.file_path()
				.delete_many(vec![file_path::id::in_vec(db_params)]),
		),
	)
	.await?;

	Ok(0)
}

// TODO: Change this macro to a fn when we're able to return
// `impl Fn(Vec<file_path::WhereParam>) -> impl Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>>`
// Maybe when TAITs arrive
#[macro_export]
macro_rules! file_paths_db_fetcher_fn {
	($db:expr) => {{
		|found_paths| async {
			// Each found path is a AND with 4 terms, and SQLite has a expression tree limit of 1000 terms
			// so we will use chunks of 200 just to be safe

			// FIXME: Can't pass this chunks variable direct to _batch because of lifetime issues
			let chunks = found_paths
				.into_iter()
				.chunks(200)
				.into_iter()
				.map(|founds| {
					$db.file_path()
						.find_many(vec![::prisma_client_rust::operator::or(
							founds.collect::<Vec<_>>(),
						)])
						.select(::sd_file_path_helper::file_path_walker::select())
				})
				.collect::<Vec<_>>();

			$db._batch(chunks)
				.await
				.map(|fetched| fetched.into_iter().flatten().collect::<Vec<_>>())
				.map_err(Into::into)
		}
	}};
}

// TODO: Change this macro to a fn when we're able to return
// `impl Fn(&Path, Vec<file_path::WhereParam>) -> impl Future<Output = Result<Vec<file_path_just_pub_id::Data>, IndexerError>>`
// Maybe when TAITs arrive
// FIXME: (fogodev) I was receiving this error here https://github.com/rust-lang/rust/issues/74497
#[macro_export]
macro_rules! to_remove_db_fetcher_fn {
	($location_id:expr, $db:expr) => {{
		|parent_iso_file_path, unique_location_id_materialized_path_name_extension_params| async {
			let location_id: ::sd_prisma::prisma::location::id::Type = $location_id;
			let db: &::sd_prisma::prisma::PrismaClient = $db;
			let parent_iso_file_path: ::sd_file_path_helper::IsolatedFilePathData<
				'static,
			> = parent_iso_file_path;
			let unique_location_id_materialized_path_name_extension_params: ::std::vec::Vec<
				::sd_prisma::prisma::file_path::WhereParam,
			> = unique_location_id_materialized_path_name_extension_params;

			// FIXME: Can't pass this chunks variable direct to _batch because of lifetime issues
			let chunks = unique_location_id_materialized_path_name_extension_params
				.into_iter()
				.chunks(200)
				.into_iter()
				.map(|unique_params| {
					db.file_path()
						.find_many(vec![::prisma_client_rust::operator::or(
							unique_params.collect(),
						)])
						.select(::sd_prisma::prisma::file_path::select!({ id }))
				})
				.collect::<::std::vec::Vec<_>>();

			let founds_ids = db._batch(chunks).await.map(|founds_chunk| {
				founds_chunk
					.into_iter()
					.map(|file_paths| file_paths.into_iter().map(|file_path| file_path.id))
					.flatten()
					.collect::<::std::collections::HashSet<_>>()
			})?;

			// NOTE: This batch size can be increased if we wish to trade memory for more performance
			const BATCH_SIZE: i64 = 1000;

			let mut to_remove = vec![];
			let mut cursor = 1;

			loop {
				let found = $db.file_path()
					.find_many(vec![
						::sd_prisma::prisma::file_path::location_id::equals(Some(location_id)),
						::sd_prisma::prisma::file_path::materialized_path::equals(Some(
							parent_iso_file_path
								.materialized_path_for_children()
								.expect("the received isolated file path must be from a directory"),
						)),
					])
					.order_by(::sd_prisma::prisma::file_path::id::order(::sd_prisma::prisma::SortOrder::Asc))
					.take(BATCH_SIZE)
					.cursor(::sd_prisma::prisma::file_path::id::equals(cursor))
					.select(::sd_prisma::prisma::file_path::select!({ id pub_id cas_id }))
					.exec()
					.await?;

				let should_stop = (found.len() as i64) < BATCH_SIZE;

				if let Some(last) = found.last() {
					cursor = last.id;
				} else {
					break;
				}

				to_remove.extend(
					found
						.into_iter()
						.filter(|file_path| !founds_ids.contains(&file_path.id))
						.map(|file_path| ::sd_file_path_helper::file_path_pub_and_cas_ids::Data {
							id: file_path.id,
							pub_id: file_path.pub_id,
							cas_id: file_path.cas_id,
						}),
				);

				if should_stop {
					break;
				}
			}

			Ok(to_remove)
		}
	}};
}

pub async fn reverse_update_directories_sizes(
	base_path: impl AsRef<Path>,
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), FilePathError> {
	let base_path = base_path.as_ref();
	let location_path = location_path.as_ref();

	let Library { sync, db, .. } = library;

	let ancestors = base_path
		.ancestors()
		.take_while(|&ancestor| ancestor != location_path)
		.map(|ancestor| IsolatedFilePathData::new(location_id, location_path, ancestor, true))
		.collect::<Result<Vec<_>, _>>()?;

	let chunked_queries = ancestors
		.iter()
		.chunks(200)
		.into_iter()
		.map(|ancestors_iso_file_paths_chunk| {
			db.file_path()
				.find_many(vec![or(ancestors_iso_file_paths_chunk
					.into_iter()
					.map(file_path::WhereParam::from)
					.collect::<Vec<_>>())])
				.select(file_path::select!({ pub_id materialized_path name }))
		})
		.collect::<Vec<_>>();

	let mut pub_id_by_ancestor_materialized_path = db
		._batch(chunked_queries)
		.await?
		.into_iter()
		.flatten()
		.filter_map(
			|file_path| match (file_path.materialized_path, file_path.name) {
				(Some(materialized_path), Some(name)) => {
					Some((format!("{materialized_path}{name}/"), (file_path.pub_id, 0)))
				}
				_ => {
					warn!(
						"Found a file_path missing its materialized_path or name: <pub_id='{:#?}'>",
						from_bytes_to_uuid(&file_path.pub_id)
					);
					None
				}
			},
		)
		.collect::<HashMap<_, _>>();

	db.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(location_id)),
			file_path::materialized_path::in_vec(
				ancestors
					.iter()
					.map(|ancestor_iso_file_path| {
						ancestor_iso_file_path
							.materialized_path_for_children()
							.expect("each ancestor is a directory")
					})
					.collect(),
			),
		])
		.select(file_path::select!({ materialized_path size_in_bytes_bytes }))
		.exec()
		.await?
		.into_iter()
		.for_each(|file_path| {
			if let Some(materialized_path) = file_path.materialized_path {
				if let Some((_, size)) =
					pub_id_by_ancestor_materialized_path.get_mut(&materialized_path)
				{
					*size += file_path
						.size_in_bytes_bytes
						.map(|size_in_bytes_bytes| {
							u64::from_be_bytes([
								size_in_bytes_bytes[0],
								size_in_bytes_bytes[1],
								size_in_bytes_bytes[2],
								size_in_bytes_bytes[3],
								size_in_bytes_bytes[4],
								size_in_bytes_bytes[5],
								size_in_bytes_bytes[6],
								size_in_bytes_bytes[7],
							])
						})
						.unwrap_or_else(|| {
							warn!("Got a directory missing its size in bytes");
							0
						});
				}
			} else {
				warn!("Corrupt database possessing a file_path entry without materialized_path");
			}
		});

	let to_sync_and_update = ancestors
		.into_iter()
		.filter_map(|ancestor_iso_file_path| {
			if let Some((pub_id, size)) = pub_id_by_ancestor_materialized_path.remove(
				&ancestor_iso_file_path
					.materialized_path_for_children()
					.expect("each ancestor is a directory"),
			) {
				let size_bytes = size.to_be_bytes().to_vec();

				Some((
					sync.shared_update(
						prisma_sync::file_path::SyncId {
							pub_id: pub_id.clone(),
						},
						file_path::size_in_bytes_bytes::NAME,
						json!(size_bytes.clone()),
					),
					db.file_path().update(
						file_path::pub_id::equals(pub_id),
						vec![file_path::size_in_bytes_bytes::set(Some(size_bytes))],
					),
				))
			} else {
				warn!("Got a missing ancestor for a file_path in the database, maybe we have a corruption");
				None
			}
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	sync.write_ops(db, to_sync_and_update).await?;

	Ok(())
}
