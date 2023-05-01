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
	collections::{HashMap, VecDeque},
	path::Path,
};

use chrono::Utc;
use itertools::Itertools;
use tokio::time::Instant;
use tracing::error;
use uuid::Uuid;

use super::{
	execute_indexer_step, finalize_indexer,
	rules::{IndexerRule, RuleKind},
	walk::walk,
	IndexerError, IndexerJobData, IndexerJobInit, IndexerJobStep, IndexerJobStepEntry,
	ScanProgress,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

/// A `IndexerJob` is a stateful job that walks a directory and indexes all files.
/// First it walks the directory and generates a list of files to index, chunked into
/// batches of [`BATCH_SIZE`]. Then for each chunk it write the file metadata to the database.
pub struct IndexerJob;

impl JobInitData for IndexerJobInit {
	type Job = IndexerJob;
}

#[async_trait::async_trait]
impl StatefulJob for IndexerJob {
	type Init = IndexerJobInit;
	type Data = IndexerJobData;
	type Step = IndexerJobStep;

	const NAME: &'static str = "indexer";

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

		let (to_walk_path, maybe_parent_file_path) = if let Some(ref sub_path) = state.init.sub_path
		{
			let full_path = ensure_sub_path_is_in_location(location_path, sub_path)
				.await
				.map_err(IndexerError::from)?;
			ensure_sub_path_is_directory(location_path, sub_path)
				.await
				.map_err(IndexerError::from)?;

			let sub_path_file_path = ctx
				.library
				.db
				.file_path()
				.find_first(filter_existing_file_path_params(
					&MaterializedPath::new(location_id, location_path, &full_path, true)
						.map_err(IndexerError::from)?,
				))
				.select(file_path_just_id_materialized_path::select())
				.exec()
				.await
				.map_err(IndexerError::from)?
				.expect("Sub path should already exist in the database");

			// If we're operating with a sub_path, then we have to put its id on `dirs_ids` map
			dirs_ids.insert(
				full_path.clone(),
				Uuid::from_slice(&sub_path_file_path.pub_id).unwrap(),
			);

			(full_path, Some(sub_path_file_path))
		} else {
			(location_path.to_path_buf(), None)
		};

		let scan_start = Instant::now();
		let found_paths = {
			let ctx = &mut ctx; // Borrow outside of closure so it's not moved
			walk(
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
				// if we're not using a sub_path, then its a full indexing and we must include root dir
				state.init.sub_path.is_none(),
			)
			.await?
		};

		// NOTE:
		// As we're passing the list of currently existing file paths to the `find_many_file_paths_by_full_path` query,
		// it means that `dirs_ids` contains just paths that still exists on the filesystem.
		dirs_ids.extend(
			ctx.library
				.db
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
						location_path.join(&MaterializedPath::from((
							location_id,
							&file_path.materialized_path,
						))),
						Uuid::from_slice(&file_path.pub_id).unwrap(),
					)
				}),
		);

		// Removing all other file paths that are not in the filesystem anymore
		let removed_paths = retain_file_paths_in_location(
			location_id,
			dirs_ids.values().copied().collect(),
			maybe_parent_file_path,
			&ctx.library.db,
		)
		.await
		.map_err(IndexerError::from)?;

		let mut new_paths = found_paths
			.into_iter()
			.filter_map(|entry| {
				MaterializedPath::new(
					location_id,
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
						(!dirs_ids.contains_key(&entry.path)).then(|| {
							IndexerJobStepEntry {
								materialized_path,
								file_id: Uuid::new_v4(), // To be set later
								parent_id: entry.path.parent().and_then(|parent_dir| {
									/***************************************************************
									 * If we're dealing with a new path which its parent already   *
									 * exist, we fetch its parent id from our `dirs_ids` map       *
									 **************************************************************/
									dirs_ids.get(parent_dir).copied()
								}),
								full_path: entry.path,
								metadata: entry.metadata,
							}
						})
					},
				)
			})
			.collect::<Vec<_>>();

		new_paths.iter_mut().for_each(|entry| {
			// If the `parent_id` is still none here, is because the parent of this entry is also
			// a new one in the DB
			if entry.parent_id.is_none() {
				entry.parent_id = entry
					.full_path
					.parent()
					.and_then(|parent_dir| dirs_ids.get(parent_dir).copied());
			}

			dirs_ids.insert(entry.full_path.clone(), entry.file_id);
		});

		let total_paths = new_paths.len();

		state.data = Some(IndexerJobData {
			indexed_path: to_walk_path,
			db_write_start: Utc::now(),
			scan_read_time: scan_start.elapsed(),
			total_paths,
			indexed_paths: 0,
			removed_paths,
		});

		state.steps = VecDeque::with_capacity(new_paths.len() / BATCH_SIZE);

		for (i, chunk) in new_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.enumerate()
		{
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

			state.steps.push_back(chunk_steps);
		}

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

	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		finalize_indexer(&state.init.location.path, state, ctx)
	}
}
