use crate::{
	file_paths_db_fetcher_fn, invalidate_query,
	job::JobError,
	library::Library,
	location::{
		file_path_helper::{
			check_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
			IsolatedFilePathData,
		},
		indexer::{execute_indexer_update_step, IndexerJobUpdateStep},
		LocationError,
	},
	to_remove_db_fetcher_fn, Node,
};
use tracing::error;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use itertools::Itertools;

use super::{
	execute_indexer_save_step, iso_file_path_factory, location_with_indexer_rules,
	remove_non_existing_file_paths, rules::IndexerRule, walk::walk_single_dir, IndexerError,
	IndexerJobSaveStep,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

pub async fn shallow(
	location: &location_with_indexer_rules::Data,
	sub_path: &PathBuf,
	node: &Arc<Node>,
	library: &Library,
) -> Result<(), JobError> {
	let location_id = location.id;
	let Some(location_path) = location.path.as_ref().map(PathBuf::from) else {
        return Err(JobError::Location(LocationError::MissingPath(location_id)));
    };

	let db = library.db.clone();

	let indexer_rules = location
		.indexer_rules
		.iter()
		.map(|rule| IndexerRule::try_from(&rule.indexer_rule))
		.collect::<Result<Vec<_>, _>>()
		.map_err(IndexerError::from)?;

	let (add_root, to_walk_path) = if sub_path != Path::new("") && sub_path != Path::new("/") {
		let full_path = ensure_sub_path_is_in_location(&location_path, &sub_path)
			.await
			.map_err(IndexerError::from)?;
		ensure_sub_path_is_directory(&location_path, &sub_path)
			.await
			.map_err(IndexerError::from)?;

		(
			!check_file_path_exists::<IndexerError>(
				&IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
					.map_err(IndexerError::from)?,
				&db,
			)
			.await?,
			full_path,
		)
	} else {
		(false, location_path.to_path_buf())
	};

	let (walked, to_update, to_remove, errors) = {
		walk_single_dir(
			&to_walk_path,
			&indexer_rules,
			|_, _| {},
			file_paths_db_fetcher_fn!(&db),
			to_remove_db_fetcher_fn!(location_id, &db),
			iso_file_path_factory(location_id, &location_path),
			add_root,
		)
		.await?
	};

	node.thumbnail_remover
		.remove_cas_ids(
			to_remove
				.iter()
				.filter_map(|file_path| file_path.cas_id.clone())
				.collect::<Vec<_>>(),
		)
		.await;

	errors.into_iter().for_each(|e| error!("{e}"));

	// TODO pass these uuids to sync system
	remove_non_existing_file_paths(to_remove, &db).await?;

	let save_steps = walked
		.chunks(BATCH_SIZE)
		.into_iter()
		.enumerate()
		.map(|(i, chunk)| IndexerJobSaveStep {
			chunk_idx: i,
			walked: chunk.collect::<Vec<_>>(),
		})
		.collect::<Vec<_>>();

	for step in save_steps {
		execute_indexer_save_step(location, &step, library).await?;
	}

	let update_steps = to_update
		.chunks(BATCH_SIZE)
		.into_iter()
		.enumerate()
		.map(|(i, chunk)| IndexerJobUpdateStep {
			chunk_idx: i,
			to_update: chunk.collect::<Vec<_>>(),
		})
		.collect::<Vec<_>>();

	for step in update_steps {
		execute_indexer_update_step(&step, library).await?;
	}

	invalidate_query!(library, "search.paths");

	library.orphan_remover.invoke().await;

	Ok(())
}
