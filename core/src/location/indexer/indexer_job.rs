use crate::{
	job::{JobError, JobInitData, JobResult, JobState, StatefulJob, WorkerContext},
	location::{
		file_path_helper::{
			ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
			file_path_just_id_materialized_path, file_path_to_isolate,
			filter_existing_file_path_params,
			isolated_file_path_data::extract_normalized_materialized_path_str,
			IsolatedFilePathData,
		},
		LocationId,
	},
	prisma::{file_path, location, PrismaClient},
	util::db::{chain_optional_iter, uuid_to_bytes},
};

use std::{collections::VecDeque, path::Path, sync::Arc, time::Duration};

use chrono::Utc;
use futures::Future;
use itertools::Itertools;
use tokio::time::Instant;
use tracing::error;
use uuid::Uuid;

use super::{
	execute_indexer_step, finalize_indexer,
	rules::aggregate_rules_by_kind,
	walk::{walk, ToWalkEntry, WalkResult, WalkedEntry},
	IndexerError, IndexerJobData, IndexerJobInit, IndexerJobStepInput, IndexerJobStepOutput,
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
	type Step = IndexerJobStepInput;

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

		let db = Arc::clone(&ctx.library.db);

		let rules_by_kind = aggregate_rules_by_kind(state.init.location.indexer_rules.iter())
			.map_err(IndexerError::from)?;

		let (to_walk_path, maybe_sub_iso_file_path) =
			if let Some(ref sub_path) = state.init.sub_path {
				let full_path = ensure_sub_path_is_in_location(location_path, sub_path)
					.await
					.map_err(IndexerError::from)?;
				ensure_sub_path_is_directory(location_path, sub_path)
					.await
					.map_err(IndexerError::from)?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, location_path, &full_path, true)
						.map_err(IndexerError::from)?;

				if ctx
					.library
					.db
					.file_path()
					.count(filter_existing_file_path_params(&sub_iso_file_path))
					.exec()
					.await
					.map_err(IndexerError::from)?
					== 0
				{
					return Err(IndexerError::SubPathNotFound(sub_path.clone().into()).into());
				}

				(full_path, Some(sub_iso_file_path))
			} else {
				(location_path.to_path_buf(), None)
			};

		let scan_start = Instant::now();
		let WalkResult {
			walked,
			to_walk,
			removed_count,
			errors,
		} = {
			let ctx = &mut ctx; // Borrow outside of closure so it's not moved
			walk(
				&to_walk_path,
				&rules_by_kind,
				|path, total_entries| {
					IndexerJobData::on_scan_progress(
						ctx,
						vec![
							ScanProgress::Message(format!("Scanning {}", path.display())),
							ScanProgress::ChunkCount(total_entries / BATCH_SIZE),
						],
					);
				},
				|found_paths| async move {
					db.file_path()
						.find_many(found_paths)
						.select(file_path_to_isolate::select())
						.exec()
						.await
						.map_err(Into::into)
				},
				|path, unique_location_id_materialized_path_name_extension_params| async move {
					db.file_path()
						.delete_many(vec![
							file_path::location_id::equals(location_id),
							file_path::materialized_path::equals(
								extract_normalized_materialized_path_str(
									location_id,
									location_path,
									path,
								)?,
							),
							file_path::WhereParam::Not(
								unique_location_id_materialized_path_name_extension_params,
							),
						])
						.exec()
						.await
						.map_err(Into::into)
				},
				|path, is_dir| {
					IsolatedFilePathData::new(location_id, location_path, path, is_dir)
						.map_err(Into::into)
				},
				50_000,
			)
			.await?
		};

		let mut total_paths = 0;

		state.steps = walked
			.chunks(BATCH_SIZE)
			.into_iter()
			.enumerate()
			.map(move |(i, chunk)| {
				let chunk_steps = chunk.collect::<Vec<_>>();
				IndexerJobData::on_scan_progress(
					&mut ctx,
					vec![
						ScanProgress::SavedChunks(i),
						ScanProgress::Message(format!("Writing {} to db", i * chunk_steps.len(),)),
					],
				);

				total_paths += chunk_steps.len() as u64;

				IndexerJobStepInput::Save(chunk_steps)
			})
			.chain(to_walk.map(IndexerJobStepInput::Walk))
			.collect();

		IndexerJobData::on_scan_progress(
			&mut ctx,
			vec![ScanProgress::Message(format!(
				"Starting saving {total_paths} files or directories, \
					there still {} directories to index",
				state.steps.len() as u64 - total_paths
			))],
		);

		state.data = Some(IndexerJobData {
			indexed_path: to_walk_path,
			rules_by_kind,
			db_write_time: Duration::ZERO,
			scan_read_time: scan_start.elapsed(),
			total_paths,
			indexed_count: 0,
			removed_count,
		});

		Ok(())
	}

	/// Process each chunk of entries in the indexer job, writing to the `file_path` table
	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let mut data = state
			.data
			.as_mut()
			.expect("critical error: missing data on job state");

		match execute_indexer_step(&state.init.location, &state.steps[0], ctx).await? {
			IndexerJobStepOutput::Save(indexed_count, elapsed_time) => {
				data.indexed_count += indexed_count;
				data.db_write_time += elapsed_time;
			}
			IndexerJobStepOutput::Walk {
				walked,
				to_walk,
				removed_count,
				elapsed_time,
			} => {
				data.removed_count += removed_count;
				data.scan_read_time += elapsed_time;

				let old_total = data.total_paths;
				let old_steps_count = state.steps.len() as u64;

				state.steps.extend(
					walked
						.chunks(BATCH_SIZE)
						.into_iter()
						.enumerate()
						.map(move |(i, chunk)| {
							let chunk_steps = chunk.collect::<Vec<_>>();
							IndexerJobData::on_scan_progress(
								&mut ctx,
								vec![
									ScanProgress::SavedChunks(i),
									ScanProgress::Message(format!(
										"Writing {} to db",
										i * chunk_steps.len(),
									)),
								],
							);

							data.total_paths += chunk_steps.len() as u64;

							IndexerJobStepInput::Save(chunk_steps)
						})
						.chain(to_walk.map(IndexerJobStepInput::Walk)),
				);

				IndexerJobData::on_scan_progress(
					&mut ctx,
					vec![ScanProgress::Message(format!(
						"Scanned more {} files or directories; {} more directories to scan",
						data.total_paths - old_total,
						state.steps.len() as u64 - old_steps_count - data.total_paths
					))],
				);
			}
		}

		Ok(())
	}

	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		finalize_indexer(&state.init.location.path, state, ctx)
	}
}
