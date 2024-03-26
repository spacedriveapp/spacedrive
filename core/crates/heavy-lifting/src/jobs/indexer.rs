use crate::{
	tasks::indexer::{walker, IndexerError, NonCriticalIndexerError},
	Error,
};

use sd_core_file_path_helper::{
	ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	FilePathError, IsolatedFilePathData,
};
use sd_core_indexer_rules::{IndexerRule, IndexerRuler};
use sd_core_prisma_helpers::{
	file_path_pub_and_cas_ids, file_path_walker, location_with_indexer_rules,
};

use sd_prisma::prisma::{file_path, location, PrismaClient, SortOrder};
use sd_task_system::{Task, TaskHandle, TaskStatus};
use sd_utils::db::maybe_missing;

use std::{
	collections::HashSet,
	hash::{Hash, Hasher},
	mem,
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use prisma_client_rust::operator::or;

use super::{
	cancel_pending_tasks,
	job_system::{
		job::{Job, JobContext, JobName, JobReturn, ReturnStatus, TaskDispatcher},
		SerializableJob,
	},
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

#[derive(Debug)]
pub struct IndexerJob {
	location: location_with_indexer_rules::Data,
	location_path: PathBuf,
	sub_path: Option<PathBuf>,
	indexer_ruler: IndexerRuler,

	pending_tasks_on_resume: Vec<TaskHandle<Error>>,
	tasks_for_shutdown: Vec<Vec<u8>>,
}

impl<Ctx: JobContext> SerializableJob<Ctx> for IndexerJob {
	fn serialize(&self) -> Option<Result<Vec<u8>, rmp_serde::encode::Error>> {
		todo!("Implement serialization")
	}

	fn deserialize(
		serialized_job: Vec<u8>,
		ctx: &Ctx,
	) -> Result<(Self, Vec<Box<dyn Task<Error>>>), rmp_serde::decode::Error> {
		todo!("Implement deserialization")
	}
}

impl Hash for IndexerJob {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

impl IndexerJob {
	pub fn new(
		location: location_with_indexer_rules::Data,
		sub_path: Option<PathBuf>,
	) -> Result<Self, IndexerError> {
		Ok(Self {
			indexer_ruler: location
				.indexer_rules
				.iter()
				.map(|rule| IndexerRule::try_from(&rule.indexer_rule))
				.collect::<Result<Vec<_>, _>>()
				.map(IndexerRuler::new)?,
			location_path: maybe_missing(&location.path, "location.path").map(PathBuf::from)?,
			location,
			sub_path,

			pending_tasks_on_resume: Vec::new(),
			tasks_for_shutdown: Vec::new(),
		})
	}
}

impl<Ctx: JobContext> Job<Ctx> for IndexerJob {
	const NAME: JobName = JobName::Indexer;

	async fn run(mut self, dispatcher: TaskDispatcher, ctx: Ctx) -> Result<ReturnStatus, Error> {
		let mut pending_running_tasks = FuturesUnordered::new();

		// if we don't have any pending task, then this is a fresh job
		if self.pending_tasks_on_resume.is_empty() {
			let to_walk = determine_initial_walk_path(
				self.location.id,
				&self.sub_path,
				&self.location_path,
				ctx.db(),
			)
			.await?;
		} else {
			pending_running_tasks.extend(mem::take(&mut self.pending_tasks_on_resume));
		}

		while let Some(task) = pending_running_tasks.next().await {
			match task {
				Ok(TaskStatus::Done((_, out))) => {}

				Ok(TaskStatus::Shutdown(task)) => {}

				Ok(TaskStatus::Error(e)) => {
					cancel_pending_tasks(&pending_running_tasks).await;

					return Err(e);
				}

				Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
					cancel_pending_tasks(&pending_running_tasks).await;

					return Ok(ReturnStatus::Canceled);
				}

				Err(e) => {
					cancel_pending_tasks(&pending_running_tasks).await;

					return Err(e.into());
				}
			}
		}

		Ok(ReturnStatus::Completed(JobReturn::default()))
	}

	fn resume(&mut self, dispatched_tasks: Vec<TaskHandle<Error>>) {
		self.pending_tasks_on_resume = dispatched_tasks;
	}
}

async fn determine_initial_walk_path(
	location_id: location::id::Type,
	sub_path: &Option<PathBuf>,
	location_path: &Path,
	db: &PrismaClient,
) -> Result<PathBuf, IndexerError> {
	match sub_path {
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
				db,
				IndexerError::SubPathNotFound,
			)
			.await?;

			Ok(full_path)
		}
		_ => Ok(location_path.to_path_buf()),
	}
}

#[derive(Debug)]
struct IsoFilePathFactory {
	location_id: location::id::Type,
	location_path: PathBuf,
}

impl walker::IsoFilePathFactory for IsoFilePathFactory {
	fn build(
		&self,
		path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<IsolatedFilePathData<'static>, FilePathError> {
		IsolatedFilePathData::new(self.location_id, &self.location_path, path, is_dir)
	}
}

#[derive(Debug)]
struct WalkerDBProxy {
	location_id: location::id::Type,
	db: Arc<PrismaClient>,
}

impl walker::WalkerDBProxy for WalkerDBProxy {
	async fn fetch_file_paths(
		&self,
		found_paths: Vec<file_path::WhereParam>,
	) -> Result<Vec<file_path_walker::Data>, IndexerError> {
		// Each found path is a AND with 4 terms, and SQLite has a expression tree limit of 1000 terms
		// so we will use chunks of 200 just to be safe
		self.db
			._batch(
				found_paths
					.into_iter()
					.chunks(200)
					.into_iter()
					.map(|founds| {
						self.db
							.file_path()
							.find_many(vec![or(founds.collect::<Vec<_>>())])
							.select(file_path_walker::select())
					})
					.collect::<Vec<_>>(),
			)
			.await
			.map(|fetched| fetched.into_iter().flatten().collect::<Vec<_>>())
			.map_err(Into::into)
	}

	async fn fetch_file_paths_to_remove(
		&self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		unique_location_id_materialized_path_name_extension_params: Vec<file_path::WhereParam>,
	) -> Result<Vec<file_path_pub_and_cas_ids::Data>, NonCriticalIndexerError> {
		// NOTE: This batch size can be increased if we wish to trade memory for more performance
		const BATCH_SIZE: i64 = 1000;

		let founds_ids = self
			.db
			._batch(
				unique_location_id_materialized_path_name_extension_params
					.into_iter()
					.chunks(200)
					.into_iter()
					.map(|unique_params| {
						self.db
							.file_path()
							.find_many(vec![or(unique_params.collect())])
							.select(file_path::select!({ id }))
					})
					.collect::<Vec<_>>(),
			)
			.await
			.map(|founds_chunk| {
				founds_chunk
					.into_iter()
					.flat_map(|file_paths| file_paths.into_iter().map(|file_path| file_path.id))
					.collect::<HashSet<_>>()
			})
			.map_err(|e| NonCriticalIndexerError::FetchAlreadyExistingFilePathIds(e.to_string()))?;

		let mut to_remove = vec![];
		let mut cursor = 1;

		loop {
			let found = self
				.db
				.file_path()
				.find_many(vec![
					file_path::location_id::equals(Some(self.location_id)),
					file_path::materialized_path::equals(Some(
						parent_iso_file_path
							.materialized_path_for_children()
							.expect("the received isolated file path must be from a directory"),
					)),
				])
				.order_by(file_path::id::order(SortOrder::Asc))
				.take(BATCH_SIZE)
				.cursor(file_path::id::equals(cursor))
				.select(file_path::select!({ id pub_id cas_id }))
				.exec()
				.await
				.map_err(|e| NonCriticalIndexerError::FetchFilePathsToRemove(e.to_string()))?;

			#[allow(clippy::cast_possible_truncation)] // Safe because we are using a constant
			let should_stop = found.len() < BATCH_SIZE as usize;

			if let Some(last) = found.last() {
				cursor = last.id;
			} else {
				break;
			}

			to_remove.extend(
				found
					.into_iter()
					.filter(|file_path| !founds_ids.contains(&file_path.id))
					.map(|file_path| file_path_pub_and_cas_ids::Data {
						id: file_path.id,
						pub_id: file_path.pub_id,
						cas_id: file_path.cas_id,
					}),
			);

			if should_stop {
				break;
			}
		}

		Ok(to_remove)
	}
}
