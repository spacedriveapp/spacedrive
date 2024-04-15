use crate::{
	file_paths_db_fetcher_fn, invalidate_query,
	library::Library,
	location::{
		indexer::{
			execute_indexer_update_step, reverse_update_directories_sizes, OldIndexerJobUpdateStep,
		},
		scan_location_sub_path, update_location_size,
	},
	old_job::JobError,
	to_remove_db_fetcher_fn, Node,
};

use sd_core_file_path_helper::{
	check_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	IsolatedFilePathData,
};
use sd_core_indexer_rules::IndexerRule;

use sd_utils::db::maybe_missing;

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::future::join_all;
use itertools::Itertools;
use tracing::{debug, error};

use super::{
	execute_indexer_save_step, iso_file_path_factory, location_with_indexer_rules,
	old_walk::walk_single_dir, remove_non_existing_file_paths, IndexerError, OldIndexerJobSaveStep,
};

/// BATCH_SIZE is the number of files to index at each step, writing the chunk of files metadata in the database.
const BATCH_SIZE: usize = 1000;

pub async fn old_shallow(
	location: &location_with_indexer_rules::Data,
	sub_path: &PathBuf,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), JobError> {
	let location_id = location.id;
	let location_path = maybe_missing(&location.path, "location.path").map(Path::new)?;

	let db = library.db.clone();
	let sync = &library.sync;

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

	let (walked, to_update, to_remove, errors, _s) = {
		walk_single_dir(
			&to_walk_path,
			&indexer_rules,
			file_paths_db_fetcher_fn!(&db),
			to_remove_db_fetcher_fn!(location_id, &db),
			iso_file_path_factory(location_id, location_path),
			add_root,
		)
		.await?
	};

	let to_remove_count = to_remove.len();

	node.thumbnailer
		.remove_indexed_cas_ids(
			to_remove
				.iter()
				.filter_map(|file_path| file_path.cas_id.clone())
				.collect::<Vec<_>>(),
			library.id,
		)
		.await;

	errors.into_iter().for_each(|e| error!("{e}"));

	remove_non_existing_file_paths(to_remove, &db, sync).await?;

	let mut new_directories_to_scan = HashSet::new();

	let mut to_create_count = 0;

	let save_steps = walked
		.chunks(BATCH_SIZE)
		.into_iter()
		.enumerate()
		.map(|(i, chunk)| {
			let walked = chunk.collect::<Vec<_>>();
			to_create_count += walked.len();

			walked
				.iter()
				.filter_map(|walked_entry| {
					walked_entry.iso_file_path.materialized_path_for_children()
				})
				.for_each(|new_dir| {
					new_directories_to_scan.insert(new_dir);
				});

			OldIndexerJobSaveStep {
				chunk_idx: i,
				walked,
			}
		})
		.collect::<Vec<_>>();

	for step in save_steps {
		execute_indexer_save_step(location, &step, library).await?;
	}

	for scan in join_all(
		new_directories_to_scan
			.into_iter()
			.map(|sub_path| scan_location_sub_path(node, library, location.clone(), sub_path)),
	)
	.await
	{
		if let Err(e) = scan {
			error!("{e}");
		}
	}

	let mut to_update_count = 0;

	let update_steps = to_update
		.chunks(BATCH_SIZE)
		.into_iter()
		.enumerate()
		.map(|(i, chunk)| {
			let to_update = chunk.collect::<Vec<_>>();
			to_update_count += to_update.len();

			OldIndexerJobUpdateStep {
				chunk_idx: i,
				to_update,
			}
		})
		.collect::<Vec<_>>();

	for step in update_steps {
		execute_indexer_update_step(&step, library).await?;
	}

	debug!(
		"Walker at shallow indexer found: \
		To create: {to_create_count}; To update: {to_update_count}; To remove: {to_remove_count};"
	);

	if to_create_count > 0 || to_update_count > 0 || to_remove_count > 0 {
		if to_walk_path != location_path {
			reverse_update_directories_sizes(to_walk_path, location_id, location_path, library)
				.await
				.map_err(IndexerError::from)?;
		}

		update_location_size(location.id, library)
			.await
			.map_err(IndexerError::from)?;

		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	}

	// library.orphan_remover.invoke().await;

	Ok(())
}
