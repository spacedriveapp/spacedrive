use crate::{
	file_paths_db_fetcher_fn,
	job::{JobError, JobInitData, JobResult, JobState, StatefulJob, WorkerContext},
	location::file_path_helper::{
		check_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		IsolatedFilePathData,
	},
	to_remove_db_fetcher_fn,
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	sync::Arc,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use super::{
	execute_indexer_save_step, finalize_indexer, iso_file_path_factory,
	location_with_indexer_rules, remove_non_existing_file_paths, rules::aggregate_rules_by_kind,
	update_notifier_fn, walk::walk_single_dir, IndexerError, IndexerJobData, IndexerJobSaveStep,
	ScanProgress,
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
	type Step = IndexerJobSaveStep;

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

		let db = Arc::clone(&ctx.library.db);

		let rules_by_kind = aggregate_rules_by_kind(state.init.location.indexer_rules.iter())
			.map_err(IndexerError::from)?;

		let (add_root, to_walk_path) = if state.init.sub_path != Path::new("") {
			let full_path = ensure_sub_path_is_in_location(location_path, &state.init.sub_path)
				.await
				.map_err(IndexerError::from)?;
			ensure_sub_path_is_directory(location_path, &state.init.sub_path)
				.await
				.map_err(IndexerError::from)?;

			(
				!check_file_path_exists::<IndexerError>(
					&IsolatedFilePathData::new(location_id, location_path, &full_path, true)
						.map_err(IndexerError::from)?,
					&db,
				)
				.await?,
				full_path,
			)
		} else {
			(false, location_path.to_path_buf())
		};

		let scan_start = Instant::now();
		let (walked, to_remove, errors) = {
			let ctx = &mut ctx;
			walk_single_dir(
				&to_walk_path,
				&rules_by_kind,
				update_notifier_fn(BATCH_SIZE, ctx),
				file_paths_db_fetcher_fn!(&db),
				to_remove_db_fetcher_fn!(location_id, location_path, &db),
				iso_file_path_factory(location_id, location_path),
				add_root,
			)
			.await?
		};

		let db_delete_start = Instant::now();
		// TODO pass these uuids to sync system
		let removed_count = remove_non_existing_file_paths(to_remove, &db).await?;
		let db_delete_time = db_delete_start.elapsed();

		let total_paths = &mut 0;

		state.steps.extend(
			walked
				.chunks(BATCH_SIZE)
				.into_iter()
				.enumerate()
				.map(|(i, chunk)| {
					let chunk_steps = chunk.collect::<Vec<_>>();

					*total_paths += chunk_steps.len() as u64;

					IndexerJobSaveStep {
						chunk_idx: i,
						walked: chunk_steps,
					}
				}),
		);

		ctx.library.orphan_remover.invoke().await;

		IndexerJobData::on_scan_progress(
			&mut ctx,
			vec![ScanProgress::Message(format!(
				"Saving {total_paths} files or directories"
			))],
		);

		state.data = Some(IndexerJobData {
			indexed_path: to_walk_path,
			rules_by_kind,
			db_write_time: db_delete_time,
			scan_read_time: scan_start.elapsed(),
			total_paths: *total_paths,
			indexed_count: 0,
			removed_count,
			total_save_steps: state.steps.len() as u64,
		});

		if !errors.is_empty() {
			Err(JobError::StepCompletedWithErrors(
				errors.into_iter().map(|e| format!("{e}")).collect(),
			))
		} else {
			Ok(())
		}
	}

	/// Process each chunk of entries in the indexer job, writing to the `file_path` table
	async fn execute_step(
		&self,
		mut ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let data = state
			.data
			.as_mut()
			.expect("critical error: missing data on job state");

		execute_indexer_save_step(&state.init.location, &state.steps[0], data, &mut ctx)
			.await
			.map(|(indexed_paths, elapsed_time)| {
				data.indexed_count += indexed_paths;
				data.db_write_time += elapsed_time;
			})
			.map_err(Into::into)
	}

	/// Logs some metadata about the indexer job
	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		finalize_indexer(&state.init.location.path, state, ctx)
	}
}
