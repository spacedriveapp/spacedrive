use crate::{
	job::{JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location},
};

use chrono::{DateTime, Utc};
use itertools::Itertools;
use prisma_client_rust::Direction;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ffi::OsStr, path::PathBuf, time::Duration};
use tokio::time::Instant;
use tracing::info;

use super::{
	indexer_rules::IndexerRule,
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
pub struct IndexerJob {}

/// `IndexerJobInit` receives a `location::Data` object to be indexed
#[derive(Serialize, Deserialize, Clone)]
pub struct IndexerJobInit {
	pub location: location::Data,
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
		ctx.progress(
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
	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let location_path = state
			.init
			.location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.unwrap();

		// query db to highers id, so we can increment it for the new files indexed
		#[derive(Deserialize, Serialize, Debug)]
		struct QueryRes {
			id: Option<i32>,
		}

		// TODO: use a select to fetch only the id instead of entire record when prisma supports it
		// grab the next id so we can increment in memory for batch inserting
		let first_file_id = ctx
			.library_ctx()
			.db
			.file_path()
			.find_first(vec![])
			.order_by(file_path::id::order(Direction::Desc))
			.exec()
			.await?
			.map(|r| r.id)
			.unwrap_or(0);

		// let first_file_id = match ctx
		// 	.library_ctx()
		// 	.db
		// 	._query_raw::<QueryRes>(raw!("SELECT MAX(id) id FROM file_paths"))
		// 	.await
		// {
		// 	Ok(rows) => rows[0].id.unwrap_or(0),
		// 	Err(e) => panic!("Error querying for next file id: {:#?}", e),
		// };

		let mut indexer_rules_by_kind = HashMap::new();
		for indexer_rule_in_location in state
			.init
			.location
			.indexer_rules
			.as_ref()
			.expect("critical error: indexer job init received a location object without indexes_rules_in_location being fetched")
			{
			let indexer_rule_data = indexer_rule_in_location.indexer_rule.as_ref()
				.expect("critical error: indexer job init received a indexes_rules_in_location object without indexes_rules being fetched");

			// Yeah, I know that `&**` is weird, but we have TryFrom<&indexer_rule::Data>
			// and this variable is a &Box<indexer_rule::Data>, so we need to dereference twice
			// and then take a fresh reference, such is the life of borrowck
			let indexer_rule = IndexerRule::try_from(&**indexer_rule_data)?;

			indexer_rules_by_kind.entry(indexer_rule.kind).or_insert(vec![]).push(indexer_rule);
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
		let mut dirs_ids = HashMap::new();
		let paths_entries = paths
			.into_iter()
			.zip(first_file_id..(first_file_id + total_paths as i32))
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
						ScanProgress::SavedChunks(i as usize),
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
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let location_path = &state
			.data
			.as_ref()
			.expect("critical error: missing data on job state")
			.location_path;

		let count = ctx
			.library_ctx()
			.db
			.file_path()
			.create_many(
				state.steps[0]
					.iter()
					.map(|entry| {
						let name;
						let extension;

						// if 'entry.path' is a directory, set extension to an empty string to
						// avoid periods in folder names being interpreted as file extensions
						if entry.is_dir {
							extension = "".to_string();
							name = extract_name(entry.path.file_name());
						} else {
							// if the 'entry.path' is not a directory, then get the extension and name.
							extension = extract_name(entry.path.extension());
							name = extract_name(entry.path.file_stem());
						}
						let materialized_path = entry
							.path
							.strip_prefix(location_path)
							.unwrap()
							.to_string_lossy()
							.to_string();

						file_path::create(
							entry.file_id,
							materialized_path,
							name,
							vec![
								file_path::is_dir::set(entry.is_dir),
								file_path::extension::set(Some(extension)),
								file_path::location_id::set(state.init.location.id),
								file_path::parent_id::set(entry.parent_id),
								file_path::date_created::set(entry.created_at.into()),
							],
						)
					})
					.collect(),
			)
			.exec()
			.await?;

		info!("Inserted {count} records");

		Ok(())
	}

	/// Logs some metadata about the indexer job
	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
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

		Ok(())
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
