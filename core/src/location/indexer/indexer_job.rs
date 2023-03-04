use crate::{
	finalize_indexer,
	job::{JobError, JobResult, JobState, StatefulJob, WorkerContext},
	location::file_path_helper::{get_existing_file_path, MaterializedPath},
};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use chrono::Utc;
use itertools::Itertools;
use tokio::time::Instant;

use super::{
	ensure_sub_path_is_directory, ensure_sub_path_is_in_location, execute_indexer_step,
	rules::{IndexerRule, RuleKind},
	walk::{walk, WalkEntry},
	IndexerError, IndexerJobData, IndexerJobInit, IndexerJobStep, IndexerJobStepEntry,
	ScanProgress,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;
pub const INDEXER_JOB_NAME: &str = "indexer";

/// A `IndexerJob` is a stateful job that walks a directory and indexes all files.
/// First it walks the directory and generates a list of files to index, chunked into
/// batches of [`BATCH_SIZE`]. Then for each chunk it write the file metadata to the database.
pub struct IndexerJob;

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
		let (last_file_path_id_manager, db, ..) = (
			Arc::clone(&ctx.library_ctx.last_file_path_id_manager),
			Arc::clone(&ctx.library_ctx.db),
		);

		// grab the next id so we can increment in memory for batch inserting
		let first_file_id = last_file_path_id_manager
			.get_max_file_path_id(state.init.location.id, &db)
			.await
			.map_err(IndexerError::from)?
			+ 1;

		let mut indexer_rules_by_kind: HashMap<RuleKind, Vec<IndexerRule>> =
			HashMap::with_capacity(state.init.location.indexer_rules.len());
		for location_rule in &state.init.location.indexer_rules {
			let indexer_rule = IndexerRule::try_from(&location_rule.indexer_rule)?;

			indexer_rules_by_kind
				.entry(indexer_rule.kind)
				.or_default()
				.push(indexer_rule);
		}

		let mut dirs_ids = HashMap::new();

		let to_walk_path = if let Some(ref sub_path) = state.init.sub_path {
			let full_path =
				ensure_sub_path_is_in_location(&state.init.location.path, sub_path).await?;
			ensure_sub_path_is_directory(&state.init.location.path, sub_path).await?;

			let parent = get_existing_file_path(
				state.init.location.id,
				MaterializedPath::new(
					state.init.location.id,
					&state.init.location.path,
					&full_path,
					true,
				)
				.map_err(IndexerError::from)?,
				&db,
			)
			.await
			.map_err(IndexerError::from)?
			.expect("Sub path should already exist in the database");

			// If we're operating with a sub_path, then we have to put its id on `dirs_ids` map
			dirs_ids.insert(
				PathBuf::from(&state.init.location.path).join(&parent.materialized_path),
				parent.id,
			);

			full_path
		} else {
			PathBuf::from(&state.init.location.path)
		};

		let scan_start = Instant::now();

		let paths = walk(
			to_walk_path,
			&indexer_rules_by_kind,
			|path, total_entries| {
				IndexerJobData::on_scan_progress(
					&ctx,
					vec![
						ScanProgress::Message(format!("Scanning {}", path.display())),
						ScanProgress::ChunkCount(total_entries / BATCH_SIZE),
					],
				);
			},
			// if we're not using a sub_path, then its a full indexing and we must include root dir
			state.init.sub_path.is_none(),
		)
		.await?;

		let total_paths = paths.len();
		let last_file_id = first_file_id + total_paths as i32;

		// Setting our global state for file_path ids
		last_file_path_id_manager
			.set_max_file_path_id(state.init.location.id, last_file_id)
			.await;

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

					MaterializedPath::new(
						state.init.location.id,
						&state.init.location.path,
						&path,
						is_dir,
					)
					.map(|materialized_path| IndexerJobStepEntry {
						materialized_path,
						created_at,
						file_id,
						parent_id,
					})
				},
			)
			.collect::<Result<Vec<_>, _>>()
			.map_err(IndexerError::from)?;

		let total_entries = paths_entries.len();

		state.data = Some(IndexerJobData {
			db_write_start: Utc::now(),
			scan_read_time: scan_start.elapsed(),
			total_paths: total_entries,
			indexed_paths: 0,
		});

		state.steps = paths_entries
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.enumerate()
			.map(|(i, chunk)| {
				let chunk_steps = chunk.collect::<Vec<_>>();
				IndexerJobData::on_scan_progress(
					&ctx,
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
		execute_indexer_step(&state.init.location, &state.steps[0], ctx)
			.await
			.map(|indexed_paths| {
				state
					.data
					.as_mut()
					.expect("critical error: missing data on job state")
					.indexed_paths = indexed_paths;
			})
	}

	/// Logs some metadata about the indexer job
	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		finalize_indexer!(state, ctx)
	}
}
