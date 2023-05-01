use crate::{
	invalidate_query,
	job::{JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::Library,
	prisma::file_path,
	sync,
	util::{db::uuid_to_bytes, error::FileIOError},
};

use std::{
	collections::HashMap,
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	time::Duration,
};

use rspc::ErrorCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tokio::time::Instant;
use tracing::info;

use super::{
	file_path_helper::{FilePathError, IsolatedFilePathData},
	location_with_indexer_rules,
};

pub mod indexer_job;
pub mod rules;
pub mod shallow_indexer_job;
mod walk;

use rules::IndexerRuleError;
use walk::{ToWalkEntry, WalkedEntry};

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
	rules_by_kind: HashMap<rules::RuleKind, Vec<rules::IndexerRule>>,
	db_write_time: Duration,
	scan_read_time: Duration,
	total_paths: u64,
	indexed_count: u64,
	removed_count: u64,
}

/// `IndexerJobStepInput` defines the action that should be executed in the current step
#[derive(Serialize, Deserialize, Debug)]
pub enum IndexerJobStepInput {
	/// `IndexerJobStepEntry`. The size of this vector is given by the [`BATCH_SIZE`] constant.
	Save(Vec<WalkedEntry>),
	Walk(ToWalkEntry),
}

#[derive(Debug)]
pub enum IndexerJobStepOutput<Walked, ToWalk>
where
	Walked: Iterator<Item = WalkedEntry>,
	ToWalk: Iterator<Item = ToWalkEntry>,
{
	Save(u64, Duration),
	Walk {
		walked: Walked,
		to_walk: ToWalk,
		removed_count: u64,
		elapsed_time: Duration,
	},
}

impl IndexerJobData {
	fn on_scan_progress(ctx: &mut WorkerContext, progress: Vec<ScanProgress>) {
		ctx.progress_debounced(
			progress
				.iter()
				.map(|p| match p.clone() {
					ScanProgress::ChunkCount(c) => JobReportUpdate::TaskCount(c),
					ScanProgress::SavedChunks(p) => JobReportUpdate::CompletedTaskCount(p),
					ScanProgress::Message(m) => JobReportUpdate::Message(m),
				})
				.collect(),
		)
	}
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
	#[error("indexer rule not found: <id={0}>")]
	IndexerRuleNotFound(i32),
	#[error("received sub path not in database: <path='{}'", .0.display())]
	SubPathNotFound(Box<Path>),

	// Internal Errors
	#[error("database error")]
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

async fn execute_indexer_step(
	location: &location_with_indexer_rules::Data,
	step: &IndexerJobStepInput,
	ctx: WorkerContext,
) -> Result<
	IndexerJobStepOutput<impl Iterator<Item = WalkedEntry>, impl Iterator<Item = ToWalkEntry>>,
	IndexerError,
> {
	match step {
		IndexerJobStepInput::Save(step) => execute_indexer_save_step(location, step, ctx)
			.await
			.map(|(indexed_count, elapsed_time)| {
				IndexerJobStepOutput::Save(indexed_count, elapsed_time)
			}),
		IndexerJobStepInput::Walk(path) => {
			execute_indexer_walk_step(location, path, ctx).await.map(
				|(walked, to_walk, removed_count, elapsed_time)| IndexerJobStepOutput::Walk {
					walked,
					to_walk,
					removed_count,
					elapsed_time,
				},
			)
		}
	}
}

async fn execute_indexer_save_step(
	location: &location_with_indexer_rules::Data,
	save_step: &[WalkedEntry],
	ctx: WorkerContext,
) -> Result<(u64, Duration), IndexerError> {
	let start_time = Instant::now();
	let Library { sync, db, .. } = &ctx.library;

	let (sync_stuff, paths): (Vec<_>, Vec<_>) = save_step
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

			(
				sync.unique_shared_create(
					sync::file_path::SyncId {
						pub_id: uuid_to_bytes(entry.pub_id),
					},
					[
						("materialized_path", json!(materialized_path)),
						("name", json!(name)),
						("is_dir", json!(*is_dir)),
						("extension", json!(extension)),
						(
							"size_in_bytes",
							json!(entry.metadata.size_in_bytes.to_string()),
						),
						("inode", json!(entry.metadata.inode.to_le_bytes())),
						("device", json!(entry.metadata.device.to_le_bytes())),
						("date_created", json!(entry.metadata.created_at)),
						("date_modified", json!(entry.metadata.modified_at)),
					],
				),
				file_path::create_unchecked(
					uuid_to_bytes(entry.pub_id),
					location.id,
					materialized_path.to_string(),
					name.to_string(),
					extension.to_string(),
					entry.metadata.inode.to_le_bytes().into(),
					entry.metadata.device.to_le_bytes().into(),
					vec![
						is_dir::set(*is_dir),
						size_in_bytes::set(entry.metadata.size_in_bytes.to_string()),
						date_created::set(entry.metadata.created_at.into()),
						date_modified::set(entry.metadata.modified_at.into()),
					],
				),
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

	Ok((count as u64, start_time.elapsed()))
}

async fn execute_indexer_walk_step(
	location: &location_with_indexer_rules::Data,
	walk_step: &ToWalkEntry,
	ctx: WorkerContext,
) -> Result<
	(
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = ToWalkEntry>,
		u64,
		Duration
	),
	IndexerError,
> {
	todo!()
}

fn finalize_indexer<SJob, Init>(
	location_path: impl AsRef<Path>,
	state: &JobState<SJob>,
	ctx: WorkerContext,
) -> JobResult
where
	SJob: StatefulJob<Init = Init, Data = IndexerJobData, Step = IndexerJobStepInput>,
	Init: Serialize + DeserializeOwned + Send + Sync + Hash,
{
	let data = state
		.data
		.as_ref()
		.expect("critical error: missing data on job state");

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
		invalidate_query!(ctx.library, "locations.getExplorerData");
	}

	Ok(Some(serde_json::to_value(state)?))
}
