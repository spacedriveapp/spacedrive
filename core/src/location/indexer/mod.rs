use crate::{
	extract_job_data, invalidate_query,
	job::{JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::Library,
	prisma::{file_path, location, PrismaClient},
	sync,
	util::{db::uuid_to_bytes, error::FileIOError},
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	time::Duration,
};

use rspc::ErrorCode;
use sd_prisma::prisma_sync;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tracing::info;

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

pub use shallow::*;

/// `IndexerJobInit` receives a `location::Data` object to be indexed
/// and possibly a `sub_path` to be indexed. The `sub_path` is used when
/// we want do index just a part of a location.
#[derive(Serialize, Deserialize)]
pub struct IndexerJobInit {
	pub location: location_with_indexer_rules::Data,
	pub sub_path: Option<PathBuf>,
}

impl Hash for IndexerJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}
/// `IndexerJobData` contains the state of the indexer job, which includes a `location_path` that
/// is cached and casted on `PathBuf` from `local_path` column in the `location` table. It also
/// contains some metadata for logging purposes.
#[derive(Serialize, Deserialize)]
pub struct IndexerJobData {
	indexed_path: PathBuf,
	indexer_rules: Vec<rules::IndexerRule>,
	db_write_time: Duration,
	scan_read_time: Duration,
	total_paths: u64,
	total_save_steps: u64,
	indexed_count: u64,
	removed_count: u64,
}

impl IndexerJobData {
	fn on_scan_progress(ctx: &mut WorkerContext, progress: Vec<ScanProgress>) {
		ctx.progress(
			progress
				.iter()
				.map(|p| match p.clone() {
					ScanProgress::ChunkCount(c) => JobReportUpdate::TaskCount(c),
					ScanProgress::SavedChunks(p) => JobReportUpdate::CompletedTaskCount(p),
					ScanProgress::Message(m) => {
						// println!("MESSAGE: {:?}", m);
						JobReportUpdate::Message(m)
					}
				})
				.collect(),
		)
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IndexerJobSaveStep {
	chunk_idx: usize,
	walked: Vec<WalkedEntry>,
}

#[derive(Clone)]
pub enum ScanProgress {
	ChunkCount(usize),
	SavedChunks(usize),
	Message(String),
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
						size_in_bytes::NAME,
						json!(entry.metadata.size_in_bytes.to_string()),
					),
					size_in_bytes::set(Some(entry.metadata.size_in_bytes.to_string())),
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

	info!("Inserted {count} records");

	Ok(count)
}

fn finalize_indexer<SJob, Init, Step>(
	location_path: impl AsRef<Path>,
	state: &JobState<SJob>,
	ctx: &mut WorkerContext,
) -> JobResult
where
	SJob: StatefulJob<Init = Init, Data = IndexerJobData, Step = Step>,
	Init: Serialize + DeserializeOwned + Send + Sync + Hash,
	Step: Serialize + DeserializeOwned + Send + Sync,
{
	let data = extract_job_data!(state);

	info!(
		"scan of {} completed in {:?}. {} new files found, \
			indexed {} files in db. db write completed in {:?}",
		location_path.as_ref().display(),
		data.scan_read_time,
		data.total_paths,
		data.indexed_count,
		data.db_write_time,
	);

	if data.indexed_count > 0 || data.removed_count > 0 {
		invalidate_query!(ctx.library, "search.paths");
	}

	Ok(Some(serde_json::to_value(state)?))
}

fn update_notifier_fn(batch_size: usize, ctx: &mut WorkerContext) -> impl FnMut(&Path, usize) + '_ {
	move |path, total_entries| {
		IndexerJobData::on_scan_progress(
			ctx,
			vec![
				ScanProgress::Message(format!(
					"Scanning: {:?}",
					path.file_name().unwrap_or(path.as_os_str())
				)),
				ScanProgress::ChunkCount(total_entries / batch_size),
			],
		);
	}
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
			$db.file_path()
				.find_many(found_paths)
				.select($crate::location::file_path_helper::file_path_to_isolate::select())
				.exec()
				.await
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
	($location_id:expr, $location_path:expr, $db:expr) => {{
		|iso_file_path, unique_location_id_materialized_path_name_extension_params| async {
			let iso_file_path: $crate::location::file_path_helper::IsolatedFilePathData<'static> =
				iso_file_path;
			$db.file_path()
				.find_many(vec![
					$crate::prisma::file_path::location_id::equals(Some($location_id)),
					$crate::prisma::file_path::materialized_path::equals(Some(
						iso_file_path
							.materialized_path_for_children()
							.expect("the received isolated file path must be from a directory"),
					)),
					::prisma_client_rust::operator::not(
						unique_location_id_materialized_path_name_extension_params,
					),
				])
				.select($crate::location::file_path_helper::file_path_just_pub_id::select())
				.exec()
				.await
				.map_err(Into::into)
		}
	}};
}
