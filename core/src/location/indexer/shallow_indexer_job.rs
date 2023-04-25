use crate::{
	job::{JobError, JobInitData, JobResult, JobState, StatefulJob, WorkerContext},
	location::file_path_helper::{
		ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_just_id_materialized_path, filter_existing_file_path_params,
		filter_file_paths_by_many_full_path_params, retain_file_paths_in_location,
		MaterializedPath,
	},
	prisma::location,
};

use std::{
	collections::{HashMap, HashSet},
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use chrono::Utc;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::error;
use uuid::Uuid;

use super::{
	execute_indexer_step, finalize_indexer, location_with_indexer_rules,
	rules::{IndexerRule, RuleKind},
	walk::walk_single_dir,
	IndexerError, IndexerJobData, IndexerJobStep, IndexerJobStepEntry, ScanProgress,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

/// `ShallowIndexerJobInit` receives a `location::Data` object to be indexed
/// and possibly a `sub_path` to be indexed. The `sub_path` is used when
/// we want do index just a part of a location.
#[derive(Serialize, Deserialize)]
pub struct ShallowIndexerJobInit {
	pub location: location_with_indexer_rules::Data,
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

impl JobInitData for ShallowIndexerJobInit {
	type Job = ShallowIndexerJob;
}

#[async_trait::async_trait]
impl StatefulJob for ShallowIndexerJob {
	type Init = ShallowIndexerJobInit;
	type Data = IndexerJobData;
	type Step = IndexerJobStep;

	const NAME: &'static str = "shallow_indexer";
	const IS_BACKGROUND: bool = true;

	fn new() -> Self {
		Self {}
	}

	/// Creates a vector of valid path buffers from a directory, chunked into batches of `BATCH_SIZE`.
	async fn init(
		&self,
		mut ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let location_id = state.init.location.id;
		let location_path = Path::new(&state.init.location.path);

		let db = ctx.library.db.clone();

		let mut indexer_rules_by_kind: HashMap<RuleKind, Vec<IndexerRule>> =
			HashMap::with_capacity(state.init.location.indexer_rules.len());

		for location_rule in &state.init.location.indexer_rules {
			let indexer_rule = IndexerRule::try_from(&location_rule.indexer_rule)?;

			indexer_rules_by_kind
				.entry(indexer_rule.kind)
				.or_default()
				.push(indexer_rule);
		}

		let (to_walk_path, parent_file_path) = if state.init.sub_path != Path::new("") {
			let full_path = ensure_sub_path_is_in_location(location_path, &state.init.sub_path)
				.await
				.map_err(IndexerError::from)?;
			ensure_sub_path_is_directory(location_path, &state.init.sub_path)
				.await
				.map_err(IndexerError::from)?;

			let materialized_path =
				MaterializedPath::new(location_id, location_path, &full_path, true)
					.map_err(IndexerError::from)?;

			(
				full_path,
				db.file_path()
					.find_first(filter_existing_file_path_params(&materialized_path))
					.select(file_path_just_id_materialized_path::select())
					.exec()
					.await
					.map_err(IndexerError::from)?
					.expect("Sub path should already exist in the database"),
			)
		} else {
			(
				location_path.to_path_buf(),
				db.file_path()
					.find_first(filter_existing_file_path_params(
						&MaterializedPath::new(location_id, location_path, location_path, true)
							.map_err(IndexerError::from)?,
					))
					.select(file_path_just_id_materialized_path::select())
					.exec()
					.await
					.map_err(IndexerError::from)?
					.expect("Location root path should already exist in the database"),
			)
		};

		let scan_start = Instant::now();
		let found_paths = {
			let ctx = &mut ctx; // Borrow outside of closure so it's not moved
			walk_single_dir(
				&to_walk_path,
				&indexer_rules_by_kind,
				|path, total_entries| {
					IndexerJobData::on_scan_progress(
						ctx,
						vec![
							ScanProgress::Message(format!("Scanning {}", path.display())),
							ScanProgress::ChunkCount(total_entries / BATCH_SIZE),
						],
					);
				},
			)
			.await?
		};

		let (already_existing_file_paths, mut to_retain): (HashSet<_>, Vec<_>) = db
			.file_path()
			.find_many(
				filter_file_paths_by_many_full_path_params(
					&location::Data::from(&state.init.location),
					&found_paths
						.iter()
						.map(|entry| &entry.path)
						.collect::<Vec<_>>(),
				)
				.await
				.map_err(IndexerError::from)?,
			)
			.select(file_path_just_id_materialized_path::select())
			.exec()
			.await?
			.into_iter()
			.map(|file_path| {
				(
					file_path.materialized_path,
					Uuid::from_slice(&file_path.pub_id).unwrap(),
				)
			})
			.unzip();

		let parent_pub_id = Uuid::from_slice(&parent_file_path.pub_id).unwrap();

		// Adding our parent path id
		to_retain.push(parent_pub_id);

		// Removing all other file paths that are not in the filesystem anymore
		let removed_paths =
			retain_file_paths_in_location(location_id, to_retain, Some(parent_file_path), &db)
				.await
				.map_err(IndexerError::from)?;

		// Filter out paths that are already in the databases
		let new_paths = found_paths
			.into_iter()
			.filter_map(|entry| {
				MaterializedPath::new(location_id, location_path, &entry.path, entry.is_dir)
					.map_or_else(
						|e| {
							error!("Failed to create materialized path: {e}");
							None
						},
						|materialized_path| {
							(!already_existing_file_paths
								.contains::<str>(materialized_path.as_ref()))
							.then_some(IndexerJobStepEntry {
								full_path: entry.path,
								materialized_path,
								file_id: Uuid::new_v4(),
								parent_id: Some(parent_pub_id),
								metadata: entry.metadata,
							})
						},
					)
			})
			// Sadly we have to collect here to be able to check the length so we can set
			// the max file path id later
			.collect::<Vec<_>>();

		let total_paths = new_paths.len();

		state.data = Some(IndexerJobData {
			indexed_path: to_walk_path,
			db_write_start: Utc::now(),
			scan_read_time: scan_start.elapsed(),
			total_paths,
			indexed_paths: 0,
			removed_paths,
		});

		state.steps = new_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.enumerate()
			.map(|(i, chunk)| {
				let chunk_steps = chunk.collect::<Vec<_>>();
				IndexerJobData::on_scan_progress(
					&mut ctx,
					vec![
						ScanProgress::SavedChunks(i),
						ScanProgress::Message(format!(
							"Writing {} of {} to db",
							i * chunk_steps.len(),
							total_paths,
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
		finalize_indexer(&state.init.location.path, state, ctx)
	}
}
