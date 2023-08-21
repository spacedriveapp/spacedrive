use crate::{
	file_paths_db_fetcher_fn, invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobReportUpdate, JobResult, JobRunMetadata,
		JobStepOutput, StatefulJob, WorkerContext,
	},
	location::{
		file_path_helper::{
			ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
			IsolatedFilePathData,
		},
		location_with_indexer_rules,
	},
	to_remove_db_fetcher_fn,
	util::db::maybe_missing,
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::Instant;
use tracing::info;

use super::{
	execute_indexer_save_step, execute_indexer_update_step, iso_file_path_factory,
	remove_non_existing_file_paths,
	rules::IndexerRule,
	walk::{keep_walking, walk, ToWalkEntry, WalkResult},
	IndexerError, IndexerJobSaveStep, IndexerJobUpdateStep,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

/// `IndexerJobInit` receives a `location::Data` object to be indexed
/// and possibly a `sub_path` to be indexed. The `sub_path` is used when
/// we want do index just a part of a location.
#[derive(Serialize, Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
pub struct IndexerJobData {
	indexed_path: PathBuf,
	indexer_rules: Vec<IndexerRule>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct IndexerJobRunMetadata {
	db_write_time: Duration,
	scan_read_time: Duration,
	total_paths: u64,
	total_updated_paths: u64,
	total_save_steps: u64,
	total_update_steps: u64,
	indexed_count: u64,
	updated_count: u64,
	removed_count: u64,
}

impl JobRunMetadata for IndexerJobRunMetadata {
	fn update(&mut self, new_data: Self) {
		self.db_write_time += new_data.db_write_time;
		self.scan_read_time += new_data.scan_read_time;
		self.total_paths += new_data.total_paths;
		self.total_updated_paths += new_data.total_updated_paths;
		self.total_save_steps += new_data.total_save_steps;
		self.total_update_steps += new_data.total_update_steps;
		self.indexed_count += new_data.indexed_count;
		self.removed_count += new_data.removed_count;
	}
}

#[derive(Clone)]
pub enum ScanProgress {
	ChunkCount(usize),
	SavedChunks(usize),
	UpdatedChunks(usize),
	Message(String),
}

impl IndexerJobData {
	fn on_scan_progress(ctx: &WorkerContext, progress: Vec<ScanProgress>) {
		ctx.progress(
			progress
				.into_iter()
				.map(|p| match p {
					ScanProgress::ChunkCount(c) => JobReportUpdate::TaskCount(c),
					ScanProgress::SavedChunks(p) | ScanProgress::UpdatedChunks(p) => {
						JobReportUpdate::CompletedTaskCount(p)
					}
					ScanProgress::Message(m) => JobReportUpdate::Message(m),
				})
				.collect(),
		)
	}
}

/// `IndexerJobStepInput` defines the action that should be executed in the current step
#[derive(Serialize, Deserialize, Debug)]
pub enum IndexerJobStepInput {
	Save(IndexerJobSaveStep),
	Walk(ToWalkEntry),
	Update(IndexerJobUpdateStep),
}

/// A `IndexerJob` is a stateful job that walks a directory and indexes all files.
/// First it walks the directory and generates a list of files to index, chunked into
/// batches of [`BATCH_SIZE`]. Then for each chunk it write the file metadata to the database.
#[async_trait::async_trait]
impl StatefulJob for IndexerJobInit {
	type Data = IndexerJobData;
	type Step = IndexerJobStepInput;
	type RunMetadata = IndexerJobRunMetadata;

	const NAME: &'static str = "indexer";

	/// Creates a vector of valid path buffers from a directory, chunked into batches of `BATCH_SIZE`.
	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let location_id = init.location.id;
		let location_path = maybe_missing(&init.location.path, "location.path").map(Path::new)?;

		let db = Arc::clone(&ctx.library.db);

		let indexer_rules = init
			.location
			.indexer_rules
			.iter()
			.map(|rule| IndexerRule::try_from(&rule.indexer_rule))
			.collect::<Result<Vec<_>, _>>()
			.map_err(IndexerError::from)?;

		let to_walk_path = match &init.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
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
			}
			_ => location_path.to_path_buf(),
		};

		let scan_start = Instant::now();
		let WalkResult {
			walked,
			to_update,
			to_walk,
			to_remove,
			errors,
		} = walk(
			&to_walk_path,
			&indexer_rules,
			update_notifier_fn(ctx),
			file_paths_db_fetcher_fn!(&db),
			to_remove_db_fetcher_fn!(location_id, &db),
			iso_file_path_factory(location_id, location_path),
			50_000,
		)
		.await?;
		let scan_read_time = scan_start.elapsed();
		let to_remove = to_remove.collect::<Vec<_>>();

		ctx.node
			.thumbnail_remover
			.remove_cas_ids(
				to_remove
					.iter()
					.filter_map(|file_path| file_path.cas_id.clone())
					.collect::<Vec<_>>(),
			)
			.await;

		let db_delete_start = Instant::now();
		// TODO pass these uuids to sync system
		let removed_count = remove_non_existing_file_paths(to_remove, &db).await?;
		let db_delete_time = db_delete_start.elapsed();

		let total_new_paths = &mut 0;
		let total_updated_paths = &mut 0;
		let to_walk_count = to_walk.len();
		let to_save_chunks = &mut 0;
		let to_update_chunks = &mut 0;

		let steps = walked
			.chunks(BATCH_SIZE)
			.into_iter()
			.enumerate()
			.map(|(i, chunk)| {
				let chunk_steps = chunk.collect::<Vec<_>>();

				*total_new_paths += chunk_steps.len() as u64;
				*to_save_chunks += 1;

				IndexerJobStepInput::Save(IndexerJobSaveStep {
					chunk_idx: i,
					walked: chunk_steps,
				})
			})
			.chain(
				to_update
					.chunks(BATCH_SIZE)
					.into_iter()
					.enumerate()
					.map(|(i, chunk)| {
						let chunk_updates = chunk.collect::<Vec<_>>();

						*total_updated_paths += chunk_updates.len() as u64;
						*to_update_chunks += 1;

						IndexerJobStepInput::Update(IndexerJobUpdateStep {
							chunk_idx: i,
							to_update: chunk_updates,
						})
					}),
			)
			.chain(to_walk.into_iter().map(IndexerJobStepInput::Walk))
			.collect::<Vec<_>>();

		IndexerJobData::on_scan_progress(
			ctx,
			vec![
				ScanProgress::ChunkCount(*to_save_chunks + *to_update_chunks),
				ScanProgress::Message(format!(
					"Starting saving {total_new_paths} files or directories, \
					{total_updated_paths} files or directories to update, \
					there still {to_walk_count} directories to index",
				)),
			],
		);

		*data = Some(IndexerJobData {
			indexed_path: to_walk_path,
			indexer_rules,
		});

		Ok((
			IndexerJobRunMetadata {
				db_write_time: db_delete_time,
				scan_read_time,
				total_paths: *total_new_paths,
				total_updated_paths: *total_updated_paths,
				indexed_count: 0,
				updated_count: 0,
				removed_count,
				total_save_steps: *to_save_chunks as u64,
				total_update_steps: *to_update_chunks as u64,
			},
			steps,
			errors
				.into_iter()
				.map(|e| format!("{e}"))
				.collect::<Vec<_>>()
				.into(),
		)
			.into())
	}

	/// Process each chunk of entries in the indexer job, writing to the `file_path` table
	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, .. }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		run_metadata: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;
		let mut new_metadata = Self::RunMetadata::default();
		match step {
			IndexerJobStepInput::Save(step) => {
				let start_time = Instant::now();

				IndexerJobData::on_scan_progress(
					ctx,
					vec![
						ScanProgress::SavedChunks(step.chunk_idx + 1),
						ScanProgress::Message(format!(
							"Writing chunk {} of {} to database",
							step.chunk_idx, run_metadata.total_save_steps
						)),
					],
				);

				let count = execute_indexer_save_step(&init.location, step, &ctx.library).await?;

				new_metadata.indexed_count = count as u64;
				new_metadata.db_write_time = start_time.elapsed();

				Ok(new_metadata.into())
			}
			IndexerJobStepInput::Update(to_update) => {
				let start_time = Instant::now();
				IndexerJobData::on_scan_progress(
					ctx,
					vec![
						ScanProgress::UpdatedChunks(to_update.chunk_idx + 1),
						ScanProgress::Message(format!(
							"Updating chunk {} of {} to database",
							to_update.chunk_idx, run_metadata.total_save_steps
						)),
					],
				);

				let count = execute_indexer_update_step(to_update, &ctx.library).await?;

				new_metadata.updated_count = count as u64;
				new_metadata.db_write_time = start_time.elapsed();

				Ok(new_metadata.into())
			}

			IndexerJobStepInput::Walk(to_walk_entry) => {
				let location_id = init.location.id;
				let location_path =
					maybe_missing(&init.location.path, "location.path").map(Path::new)?;

				let db = Arc::clone(&ctx.library.db);

				let scan_start = Instant::now();

				let WalkResult {
					walked,
					to_update,
					to_walk,
					to_remove,
					errors,
				} = keep_walking(
					to_walk_entry,
					&data.indexer_rules,
					update_notifier_fn(ctx),
					file_paths_db_fetcher_fn!(&db),
					to_remove_db_fetcher_fn!(location_id, &db),
					iso_file_path_factory(location_id, location_path),
				)
				.await?;

				new_metadata.scan_read_time = scan_start.elapsed();

				let db_delete_time = Instant::now();
				// TODO pass these uuids to sync system
				new_metadata.removed_count = remove_non_existing_file_paths(to_remove, &db).await?;
				new_metadata.db_write_time = db_delete_time.elapsed();

				let to_walk_count = to_walk.len();

				let more_steps = walked
					.chunks(BATCH_SIZE)
					.into_iter()
					.enumerate()
					.map(|(i, chunk)| {
						let chunk_steps = chunk.collect::<Vec<_>>();
						new_metadata.total_paths += chunk_steps.len() as u64;
						new_metadata.total_save_steps += 1;

						IndexerJobStepInput::Save(IndexerJobSaveStep {
							chunk_idx: i,
							walked: chunk_steps,
						})
					})
					.chain(to_update.chunks(BATCH_SIZE).into_iter().enumerate().map(
						|(i, chunk)| {
							let chunk_updates = chunk.collect::<Vec<_>>();
							new_metadata.total_updated_paths += chunk_updates.len() as u64;
							new_metadata.total_update_steps += 1;

							IndexerJobStepInput::Update(IndexerJobUpdateStep {
								chunk_idx: i,
								to_update: chunk_updates,
							})
						},
					))
					.chain(to_walk.into_iter().map(IndexerJobStepInput::Walk))
					.collect::<Vec<_>>();

				IndexerJobData::on_scan_progress(
					ctx,
					vec![
						ScanProgress::ChunkCount(more_steps.len() - to_walk_count),
						ScanProgress::Message(format!(
							"Scanned more {} files or directories; \
							{} more directories to scan and more {} entries to update",
							new_metadata.total_paths,
							to_walk_count,
							new_metadata.total_updated_paths
						)),
					],
				);

				Ok((
					more_steps,
					new_metadata,
					errors
						.into_iter()
						.map(|e| format!("{e}"))
						.collect::<Vec<_>>()
						.into(),
				)
					.into())
			}
		}
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		_data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		info!(
			"Scan of {} completed in {:?}. {} new files found, \
			indexed {} files in db, updated {} entries. db write completed in {:?}",
			maybe_missing(&init.location.path, "location.path")?,
			run_metadata.scan_read_time,
			run_metadata.total_paths,
			run_metadata.indexed_count,
			run_metadata.total_updated_paths,
			run_metadata.db_write_time,
		);

		if run_metadata.indexed_count > 0 || run_metadata.removed_count > 0 {
			invalidate_query!(ctx.library, "search.paths");
		}

		if run_metadata.total_updated_paths > 0 {
			// Invoking orphan remover here as we probably have some orphans objects due to updates
			ctx.library.orphan_remover.invoke().await;
		}

		Ok(Some(json!({"init: ": init, "run_metadata": run_metadata})))
	}
}

fn update_notifier_fn(ctx: &WorkerContext) -> impl FnMut(&Path, usize) + '_ {
	move |path, total_entries| {
		IndexerJobData::on_scan_progress(
			ctx,
			vec![ScanProgress::Message(format!(
				"Scanning: {:?}; Found: {total_entries} entries",
				path.file_name().unwrap_or(path.as_os_str())
			))],
		);
	}
}
