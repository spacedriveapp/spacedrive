use crate::{
	finalize_indexer,
	job::{JobError, JobResult, JobState, StatefulJob, WorkerContext},
	location::file_path_helper::{
		get_existing_file_path, get_many_file_paths_by_full_path, get_max_file_path_id,
		set_max_file_path_id, MaterializedPath,
	},
	prisma::file_path,
};

use std::{
	collections::HashMap,
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use chrono::Utc;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::error;

use super::{
	ensure_sub_path_is_directory, ensure_sub_path_is_in_location, execute_indexer_step,
	indexer_job_location,
	rules::{IndexerRule, RuleKind},
	walk::walk_single_dir,
	IndexerError, IndexerJobData, IndexerJobStep, IndexerJobStepEntry, ScanProgress,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;
pub const SHALLOW_INDEXER_JOB_NAME: &str = "shallow_indexer";

/// `ShallowIndexerJobInit` receives a `location::Data` object to be indexed
/// and possibly a `sub_path` to be indexed. The `sub_path` is used when
/// we want do index just a part of a location.
#[derive(Serialize, Deserialize)]
pub struct ShallowIndexerJobInit {
	pub location: indexer_job_location::Data,
	pub sub_path: PathBuf,
}

impl Hash for ShallowIndexerJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		self.sub_path.hash(state);
	}
}

/// A `ShallowIndexerJob` is a stateful job that indexes all files in a directory, without checking inner directories.
/// First it checks the directory and generates a list of files to index, chunked into
/// batches of [`BATCH_SIZE`]. Then for each chunk it write the file metadata to the database.
pub struct ShallowIndexerJob;

#[async_trait::async_trait]
impl StatefulJob for ShallowIndexerJob {
	type Init = ShallowIndexerJobInit;
	type Data = IndexerJobData;
	type Step = IndexerJobStep;

	fn name(&self) -> &'static str {
		SHALLOW_INDEXER_JOB_NAME
	}

	/// Creates a vector of valid path buffers from a directory, chunked into batches of `BATCH_SIZE`.
	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		// grab the next id so we can increment in memory for batch inserting
		let first_file_id = get_max_file_path_id(&ctx.library_ctx)
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

		let (to_walk_path, parent_id) = if state.init.sub_path != Path::new("") {
			let full_path =
				ensure_sub_path_is_in_location(&state.init.location.path, &state.init.sub_path)
					.await?;
			ensure_sub_path_is_directory(&state.init.location.path, &state.init.sub_path).await?;

			(
				Path::new(&state.init.sub_path).join(&state.init.sub_path),
				get_existing_file_path(
					state.init.location.id,
					MaterializedPath::new(
						state.init.location.id,
						&state.init.location.path,
						&full_path,
						true,
					)
					.map_err(IndexerError::from)?,
					&ctx.library_ctx,
				)
				.await
				.map_err(IndexerError::from)?
				.expect("Sub path should already exist in the database")
				.id,
			)
		} else {
			(
				PathBuf::from(&state.init.location.path),
				ctx.library_ctx
					.db
					.file_path()
					.find_first(vec![
						file_path::location_id::equals(state.init.location.id),
						file_path::materialized_path::equals("/".to_string()),
					])
					.exec()
					.await?
					.expect("Location root path should already exist in the database")
					.id,
			)
		};

		let scan_start = Instant::now();
		let found_paths = walk_single_dir(
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
		)
		.await?;

		let already_existing_file_paths_by_path = get_many_file_paths_by_full_path(
			&state.init.location,
			&found_paths
				.iter()
				.map(|entry| &entry.path)
				.collect::<Vec<_>>(),
			&ctx.library_ctx,
		)
		.await
		.map_err(IndexerError::from)?
		.into_iter()
		.map(|file_path| (file_path.materialized_path.clone(), file_path))
		.collect::<HashMap<_, _>>();

		// Filter out paths that are already in the databases
		let mut found_paths = found_paths
			.into_iter()
			.filter_map(|entry| {
				MaterializedPath::new(
					state.init.location.id,
					&state.init.location.path,
					&entry.path,
					entry.is_dir,
				)
				.map_or_else(
					|e| {
						error!("Failed to create materialized path: {e}");
						None
					},
					|materialized_path| {
						(!already_existing_file_paths_by_path
							.contains_key(materialized_path.as_ref()))
						.then_some(IndexerJobStepEntry {
							full_path: entry.path,
							materialized_path,
							created_at: entry.created_at,
							file_id: 0, // To be set later
							parent_id: Some(parent_id),
							is_dir: entry.is_dir,
						})
					},
				)
			})
			// Sadly we have to collect here to be able to check the length so we can set
			// the max file path id later
			.collect::<Vec<_>>();

		let total_paths = found_paths.len();
		let last_file_id = first_file_id + total_paths as i32;

		// Setting our global state for file_path ids
		set_max_file_path_id(last_file_id);

		found_paths
			.iter_mut()
			.zip(first_file_id..last_file_id)
			.for_each(|(entry, file_id)| {
				entry.file_id = file_id;
			});

		let total_entries = found_paths.len();

		state.data = Some(IndexerJobData {
			db_write_start: Utc::now(),
			scan_read_time: scan_start.elapsed(),
			total_paths: total_entries,
			indexed_paths: 0,
		});

		state.steps = found_paths
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
