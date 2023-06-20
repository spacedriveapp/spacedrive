use crate::{
	extract_job_data_mut, file_paths_db_fetcher_fn,
	job::{JobError, JobInitData, JobResult, JobState, StatefulJob, WorkerContext},
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		IsolatedFilePathData,
	},
	to_remove_db_fetcher_fn,
	util::db::maybe_missing,
};

use std::{path::Path, sync::Arc};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use super::{
	execute_indexer_save_step, finalize_indexer, iso_file_path_factory,
	remove_non_existing_file_paths,
	rules::IndexerRule,
	update_notifier_fn,
	walk::{keep_walking, walk, ToWalkEntry, WalkResult},
	IndexerError, IndexerJobData, IndexerJobInit, IndexerJobSaveStep, ScanProgress,
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

/// `IndexerJobStepInput` defines the action that should be executed in the current step
#[derive(Serialize, Deserialize, Debug)]
pub enum IndexerJobStepInput {
	/// `IndexerJobStepEntry`. The size of this vector is given by the [`BATCH_SIZE`] constant.
	Save(IndexerJobSaveStep),
	Walk(ToWalkEntry),
}

#[async_trait::async_trait]
impl StatefulJob for IndexerJob {
	type Init = IndexerJobInit;
	type Data = IndexerJobData;
	type Step = IndexerJobStepInput;

	const NAME: &'static str = "indexer";

	fn new() -> Self {
		Self {}
	}

	/// Creates a vector of valid path buffers from a directory, chunked into batches of `BATCH_SIZE`.
	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let location_id = state.init.location.id;
		let location_path =
			maybe_missing(&state.init.location.path, "location.path").map(Path::new)?;

		let db = Arc::clone(&ctx.library.db);

		let indexer_rules = state
			.init
			.location
			.indexer_rules
			.iter()
			.map(|rule| IndexerRule::try_from(&rule.indexer_rule))
			.collect::<Result<Vec<_>, _>>()
			.map_err(IndexerError::from)?;

		let to_walk_path = if let Some(ref sub_path) = state.init.sub_path {
			let full_path = ensure_sub_path_is_in_location(location_path, sub_path)
				.await
				.map_err(IndexerError::from)?;
			ensure_sub_path_is_directory(location_path, sub_path)
				.await
				.map_err(IndexerError::from)?;

			ensure_file_path_exists(
				sub_path,
				&IsolatedFilePathData::new(location_id, location_path, &full_path, true)
					.map_err(IndexerError::from)?,
				&db,
				IndexerError::SubPathNotFound,
			)
			.await?;

			full_path
		} else {
			location_path.to_path_buf()
		};

		let scan_start = Instant::now();
		let WalkResult {
			walked,
			to_walk,
			to_remove,
			errors,
		} = {
			walk(
				&to_walk_path,
				&indexer_rules,
				update_notifier_fn(BATCH_SIZE, ctx),
				file_paths_db_fetcher_fn!(&db),
				to_remove_db_fetcher_fn!(location_id, location_path, &db),
				iso_file_path_factory(location_id, location_path),
				50_000,
			)
			.await?
		};
		let scan_read_time = scan_start.elapsed();

		let db_delete_start = Instant::now();
		// TODO pass these uuids to sync system
		let removed_count = remove_non_existing_file_paths(to_remove, &db).await?;
		let db_delete_time = db_delete_start.elapsed();

		let total_paths = &mut 0;
		let to_walk_count = to_walk.len();

		state.steps.extend(
			walked
				.chunks(BATCH_SIZE)
				.into_iter()
				.enumerate()
				.map(|(i, chunk)| {
					let chunk_steps = chunk.collect::<Vec<_>>();

					*total_paths += chunk_steps.len() as u64;

					IndexerJobStepInput::Save(IndexerJobSaveStep {
						chunk_idx: i,
						walked: chunk_steps,
					})
				})
				.chain(to_walk.into_iter().map(IndexerJobStepInput::Walk)),
		);

		IndexerJobData::on_scan_progress(
			ctx,
			vec![ScanProgress::Message(format!(
				"Starting saving {total_paths} files or directories, \
					there still {to_walk_count} directories to index",
			))],
		);

		state.data = Some(IndexerJobData {
			indexed_path: to_walk_path,
			indexer_rules,
			db_write_time: db_delete_time,
			scan_read_time,
			total_paths: *total_paths,
			indexed_count: 0,
			removed_count,
			total_save_steps: state.steps.len() as u64 - to_walk_count as u64,
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
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let data = extract_job_data_mut!(state);

		match &state.steps[0] {
			IndexerJobStepInput::Save(step) => {
				let start_time = Instant::now();

				IndexerJobData::on_scan_progress(
					ctx,
					vec![
						ScanProgress::SavedChunks(step.chunk_idx),
						ScanProgress::Message(format!(
							"Writing chunk {} of {} to database",
							step.chunk_idx, data.total_save_steps
						)),
					],
				);

				let count =
					execute_indexer_save_step(&state.init.location, step, &ctx.library.clone())
						.await?;

				data.indexed_count += count as u64;
				data.db_write_time += start_time.elapsed();
			}
			IndexerJobStepInput::Walk(to_walk_entry) => {
				let location_id = state.init.location.id;
				let location_path =
					maybe_missing(&state.init.location.path, "location.path").map(Path::new)?;

				let db = Arc::clone(&ctx.library.db);

				let scan_start = Instant::now();

				let WalkResult {
					walked,
					to_walk,
					to_remove,
					errors,
				} = {
					keep_walking(
						to_walk_entry,
						&data.indexer_rules,
						update_notifier_fn(BATCH_SIZE, ctx),
						file_paths_db_fetcher_fn!(&db),
						to_remove_db_fetcher_fn!(location_id, location_path, &db),
						iso_file_path_factory(location_id, location_path),
					)
					.await?
				};

				data.scan_read_time += scan_start.elapsed();

				let db_delete_time = Instant::now();
				// TODO pass these uuids to sync system
				data.removed_count += remove_non_existing_file_paths(to_remove, &db).await?;
				data.db_write_time += db_delete_time.elapsed();

				let _old_total = data.total_paths;
				let _old_steps_count = state.steps.len() as u64;

				state.steps.extend(
					walked
						.chunks(BATCH_SIZE)
						.into_iter()
						.enumerate()
						.map(|(i, chunk)| {
							let chunk_steps = chunk.collect::<Vec<_>>();
							data.total_paths += chunk_steps.len() as u64;

							IndexerJobStepInput::Save(IndexerJobSaveStep {
								chunk_idx: i,
								walked: chunk_steps,
							})
						})
						.chain(to_walk.into_iter().map(IndexerJobStepInput::Walk)),
				);

				// IndexerJobData::on_scan_progress(
				// 	&mut ctx,
				// 	vec![ScanProgress::Message(format!(
				// 		"Scanned more {} files or directories; {} more directories to scan",
				// 		data.total_paths - old_total,
				// 		state.steps.len() as u64 - old_steps_count - data.total_paths
				// 	))],
				// );

				if !errors.is_empty() {
					return Err(JobError::StepCompletedWithErrors(
						errors.into_iter().map(|e| format!("{e}")).collect(),
					));
				}
			}
		}

		Ok(())
	}

	async fn finalize(&mut self, ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
		let location_path =
			maybe_missing(&state.init.location.path, "location.path").map(Path::new)?;

		finalize_indexer(location_path, state, ctx)
	}
}
