use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	location::indexer::rules::RuleKind,
	prisma::{file_path, location},
};

use std::{
	collections::HashMap,
	ffi::OsStr,
	hash::{Hash, Hasher},
	path::PathBuf,
	time::Duration,
};

use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::Instant;
use tracing::info;

use super::{
	super::file_path_helper::{get_max_file_path_id, set_max_file_path_id},
	rules::IndexerRule,
	walk::{walk, WalkEntry},
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;
pub const INDEXER_JOB_NAME: &str = "indexer";

#[derive(Clone)]
pub enum ScanProgress {
	ChunkCount(usize),
	SavedChunks(usize),
	Message(String),
}

/// A `IndexerJob` is a stateful job that walks a directory and indexes all files.
/// First it walks the directory and generates a list of files to index, chunked into
/// batches of [`BATCH_SIZE`]. Then for each chunk it write the file metadata to the database.
pub struct IndexerJob;

location::include!(indexer_job_location {
	indexer_rules: select { indexer_rule }
});
file_path::select!(file_path_id_only { id });

/// `IndexerJobInit` receives a `location::Data` object to be indexed
#[derive(Serialize, Deserialize)]
pub struct IndexerJobInit {
	pub location: indexer_job_location::Data,
}

impl Hash for IndexerJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
	}
}
/// `IndexerJobData` contains the state of the indexer job, which includes a `location_path` that
/// is cached and casted on `PathBuf` from `local_path` column in the `location` table. It also
/// contains some metadata for logging purposes.
#[derive(Serialize, Deserialize)]
pub struct IndexerJobData {
	location_path: PathBuf,
	db_write_start: DateTime<Utc>,
	scan_read_time: Duration,
	total_paths: usize,
}

/// `IndexerJobStep` is a type alias, specifying that each step of the [`IndexerJob`] is a vector of
/// `IndexerJobStepEntry`. The size of this vector is given by the [`BATCH_SIZE`] constant.
pub type IndexerJobStep = Vec<IndexerJobStepEntry>;

/// `IndexerJobStepEntry` represents a single file to be indexed, given its metadata to be written
/// on the `file_path` table in the database
#[derive(Serialize, Deserialize)]
pub struct IndexerJobStepEntry {
	path: PathBuf,
	created_at: DateTime<Utc>,
	file_id: i32,
	parent_id: Option<i32>,
	is_dir: bool,
}

impl IndexerJobData {
	fn on_scan_progress(ctx: WorkerContext, progress: Vec<ScanProgress>) {
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

#[async_trait::async_trait]
impl StatefulJob for IndexerJob {
	type Init = IndexerJobInit;
	type Data = IndexerJobData;
	type Step = IndexerJobStep;

	fn name(&self) -> &'static str {
		INDEXER_JOB_NAME
	}

	/// Creates a vector of valid path buffers from a directory, chunked into batches of `BATCH_SIZE`.
	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let location_path = state
			.init
			.location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.unwrap();

		// grab the next id so we can increment in memory for batch inserting
		let first_file_id = get_max_file_path_id(&ctx.library_ctx).await?;

		let mut indexer_rules_by_kind: HashMap<RuleKind, Vec<IndexerRule>> =
			HashMap::with_capacity(state.init.location.indexer_rules.len());
		for location_rule in &state.init.location.indexer_rules {
			let indexer_rule = IndexerRule::try_from(&location_rule.indexer_rule)?;

			indexer_rules_by_kind
				.entry(indexer_rule.kind)
				.or_default()
				.push(indexer_rule);
		}

		let scan_start = Instant::now();
		let inner_ctx = ctx.clone();
		let paths = walk(
			location_path.clone(),
			&indexer_rules_by_kind,
			move |path, total_entries| {
				IndexerJobData::on_scan_progress(
					inner_ctx.clone(),
					vec![
						ScanProgress::Message(format!("Scanning {}", path.display())),
						ScanProgress::ChunkCount(total_entries / BATCH_SIZE),
					],
				);
			},
		)
		.await?;

		let total_paths = paths.len();
		let last_file_id = first_file_id + total_paths as i32;

		// Setting our global state for file_path ids
		set_max_file_path_id(last_file_id);

		let mut dirs_ids = HashMap::new();
		let paths_entries = paths
			.into_iter()
			.zip(first_file_id..last_file_id)
			.map(
				|(
					WalkEntry {
						path,
						is_dir,
						created_at,
					},
					file_id,
				)| {
					let parent_id = if let Some(parent_dir) = path.parent() {
						dirs_ids.get(parent_dir).copied()
					} else {
						None
					};

					dirs_ids.insert(path.clone(), file_id);

					IndexerJobStepEntry {
						path,
						created_at,
						file_id,
						parent_id,
						is_dir,
					}
				},
			)
			.collect::<Vec<_>>();

		let total_entries = paths_entries.len();

		state.data = Some(IndexerJobData {
			location_path,
			db_write_start: Utc::now(),
			scan_read_time: scan_start.elapsed(),
			total_paths: total_entries,
		});

		state.steps = paths_entries
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.enumerate()
			.map(|(i, chunk)| {
				let chunk_steps = chunk.collect::<Vec<_>>();
				IndexerJobData::on_scan_progress(
					ctx.clone(),
					vec![
						ScanProgress::SavedChunks(i),
						ScanProgress::Message(format!(
							"Writing {} of {} to db",
							i * chunk_steps.len(),
							total_entries,
						)),
					],
				);
				chunk_steps
			})
			.collect();

		Ok(())
	}

	/// Process each chunk of entries in the indexer job, writing to the `file_path` table
	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let data = &state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		let db = &ctx.library_ctx.db;

		let location_path = &data.location_path;
		let location_id = state.init.location.id;

		let (sync_stuff, paths): (Vec<_>, Vec<_>) = state.steps[0]
			.iter()
			.map(|entry| {
				let name;
				let extension;

				// if 'entry.path' is a directory, set extension to an empty string to
				// avoid periods in folder names being interpreted as file extensions
				if entry.is_dir {
					extension = None;
					name = extract_name(entry.path.file_name());
				} else {
					// if the 'entry.path' is not a directory, then get the extension and name.
					extension = Some(extract_name(entry.path.extension()).to_lowercase());
					name = extract_name(entry.path.file_stem());
				}
				let mut materialized_path = entry
					.path
					.strip_prefix(location_path)
					.unwrap()
					.to_str()
					.expect("Found non-UTF-8 path")
					.to_string();

				if entry.is_dir && !materialized_path.ends_with('/') {
					materialized_path += "/";
				}

				use file_path::*;

				(
					(
						json!({
							"id": entry.file_id,
							"location_id": state.init.location.pub_id,
						}),
						[
							("materialized_path", json!(materialized_path.clone())),
							("name", json!(name.clone())),
							("is_dir", json!(entry.is_dir)),
							("extension", json!(extension.clone())),
							("parent_id", json!(entry.parent_id)),
							("date_created", json!(entry.created_at)),
						],
					),
					file_path::create_unchecked(
						entry.file_id,
						location_id,
						materialized_path,
						name,
						vec![
							is_dir::set(entry.is_dir),
							extension::set(extension),
							parent_id::set(entry.parent_id),
							date_created::set(entry.created_at.into()),
						],
					),
				)
			})
			.unzip();

		let count = ctx
			.library_ctx
			.sync
			.write_op(
				db,
				ctx.library_ctx
					.sync
					.owned_create_many("FilePath", sync_stuff, true),
				db.file_path().create_many(paths).skip_duplicates(),
			)
			.await?;

		info!("Inserted {count} records");

		Ok(())
	}

	/// Logs some metadata about the indexer job
	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		info!(
			"scan of {} completed in {:?}. {:?} files found. db write completed in {:?}",
			state.init.location.local_path.as_ref().unwrap(),
			data.scan_read_time,
			data.total_paths,
			(Utc::now() - data.db_write_start)
				.to_std()
				.expect("critical error: non-negative duration"),
		);

		Ok(Some(serde_json::to_value(state)?))
	}
}

/// Extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
	os_string
		.unwrap_or_default()
		.to_str()
		.unwrap_or_default()
		.to_owned()
}
