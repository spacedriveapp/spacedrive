use crate::{
	library::Library,
	prisma::{file_path, location, PrismaClient},
	sync,
	util::{db::uuid_to_bytes, error::FileIOError},
};

use std::path::Path;

use chrono::Utc;
use rspc::ErrorCode;
use sd_prisma::prisma_sync;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tracing::trace;

use super::{
	file_path_helper::{file_path_just_pub_id, FilePathError, IsolatedFilePathData},
	location_with_indexer_rules,
};

pub mod indexer_job;
pub mod rules;
mod shallow;
mod walk;

use rules::IndexerRuleError;
use walk::WalkedEntry;

pub use indexer_job::IndexerJobInit;
pub use shallow::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct IndexerJobSaveStep {
	chunk_idx: usize,
	walked: Vec<WalkedEntry>,
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
	save_step: &IndexerJobSaveStep,
	library: &Library,
) -> Result<i64, IndexerError> {
	let Library { sync, db, .. } = &library;

	let (sync_stuff, paths): (Vec<_>, Vec<_>) = save_step
		.walked
		.iter()
		.map(|entry| {
			let IsolatedFilePathData {
				materialized_path,
				is_dir,
				name,
				extension,
				..
			} = &entry.iso_file_path;

			use file_path::*;

			let pub_id = uuid_to_bytes(entry.pub_id);

			let (sync_params, db_params): (Vec<_>, Vec<_>) = [
				(
					(
						location::NAME,
						json!(prisma_sync::location::SyncId {
							pub_id: pub_id.clone()
						}),
					),
					location_id::set(Some(location.id)),
				),
				(
					(materialized_path::NAME, json!(materialized_path)),
					materialized_path::set(Some(materialized_path.to_string())),
				),
				((name::NAME, json!(name)), name::set(Some(name.to_string()))),
				((is_dir::NAME, json!(*is_dir)), is_dir::set(Some(*is_dir))),
				(
					(extension::NAME, json!(extension)),
					extension::set(Some(extension.to_string())),
				),
				(
					(
						size_in_bytes_bytes::NAME,
						json!(entry.metadata.size_in_bytes.to_be_bytes().to_vec()),
					),
					size_in_bytes_bytes::set(Some(
						entry.metadata.size_in_bytes.to_be_bytes().to_vec(),
					)),
				),
				(
					(inode::NAME, json!(entry.metadata.inode.to_le_bytes())),
					inode::set(Some(entry.metadata.inode.to_le_bytes().into())),
				),
				(
					(device::NAME, json!(entry.metadata.device.to_le_bytes())),
					device::set(Some(entry.metadata.device.to_le_bytes().into())),
				),
				(
					(date_created::NAME, json!(entry.metadata.created_at)),
					date_created::set(Some(entry.metadata.created_at.into())),
				),
				(
					(date_modified::NAME, json!(entry.metadata.modified_at)),
					date_modified::set(Some(entry.metadata.modified_at.into())),
				),
				(
					(date_indexed::NAME, json!(Utc::now())),
					date_indexed::set(Some(Utc::now().into())),
				),
			]
			.into_iter()
			.unzip();

			(
				sync.unique_shared_create(
					sync::file_path::SyncId {
						pub_id: uuid_to_bytes(entry.pub_id),
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
				sync_stuff,
				db.file_path().create_many(paths).skip_duplicates(),
			),
		)
		.await?;

	trace!("Inserted {count} records");

	Ok(count)
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
	to_remove: impl IntoIterator<Item = file_path_just_pub_id::Data>,
	db: &PrismaClient,
) -> Result<u64, IndexerError> {
	db.file_path()
		.delete_many(vec![file_path::pub_id::in_vec(
			to_remove.into_iter().map(|data| data.pub_id).collect(),
		)])
		.exec()
		.await
		.map(|count| count as u64)
		.map_err(Into::into)
}

// TODO: Change this macro to a fn when we're able to return
// `impl Fn(Vec<file_path::WhereParam>) -> impl Future<Output = Result<Vec<file_path_to_isolate::Data>, IndexerError>>`
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
						.find_many(founds.collect::<Vec<_>>())
						.select($crate::location::file_path_helper::file_path_to_isolate::select())
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
		|iso_file_path, unique_location_id_materialized_path_name_extension_params| async {
			let iso_file_path: $crate::location::file_path_helper::IsolatedFilePathData<'static> =
				iso_file_path;

			// FIXME: Can't pass this chunks variable direct to _batch because of lifetime issues
			let chunks = unique_location_id_materialized_path_name_extension_params
				.into_iter()
				.chunks(200)
				.into_iter()
				.map(|unique_params| {
					$db.file_path()
						.find_many(vec![
							$crate::prisma::file_path::location_id::equals(Some($location_id)),
							$crate::prisma::file_path::materialized_path::equals(Some(
								iso_file_path.materialized_path_for_children().expect(
									"the received isolated file path must be from a directory",
								),
							)),
							::prisma_client_rust::operator::not(vec![
								::prisma_client_rust::operator::or(unique_params.collect()),
							]),
						])
						.select($crate::location::file_path_helper::file_path_just_pub_id::select())
				})
				.collect::<::std::vec::Vec<_>>();

			$db._batch(chunks)
				.await
				.map(|to_remove| {
					// This is an intersection between all sets
					let mut sets = to_remove
						.into_iter()
						.map(|fetched_vec| {
							fetched_vec
								.into_iter()
								.map(|fetched| {
									::uuid::Uuid::from_slice(&fetched.pub_id)
										.expect("file_path.pub_id is invalid!")
								})
								.collect::<::std::collections::HashSet<_>>()
						})
						.collect::<Vec<_>>();

					let mut intersection = ::std::collections::HashSet::new();
					while let Some(set) = sets.pop() {
						for pub_id in set {
							// Remove returns true if the element was present in the set
							if sets.iter_mut().all(|set| set.remove(&pub_id)) {
								intersection.insert(pub_id);
							}
						}
					}

					intersection
						.into_iter()
						.map(|pub_id| {
							$crate::location::file_path_helper::file_path_just_pub_id::Data {
								pub_id: pub_id.as_bytes().to_vec(),
							}
						})
						.collect()
				})
				.map_err(::std::convert::Into::into)
		}
	}};
}
