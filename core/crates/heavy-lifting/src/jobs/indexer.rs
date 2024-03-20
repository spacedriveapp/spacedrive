use crate::tasks::indexer::{walker, IndexerError, NonCriticalIndexerError};

use sd_core_file_path_helper::{FilePathError, IsolatedFilePathData};
use sd_core_prisma_helpers::{file_path_pub_and_cas_ids, file_path_walker};

use sd_prisma::prisma::{file_path, location, PrismaClient, SortOrder};

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use itertools::Itertools;
use prisma_client_rust::operator::or;

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
