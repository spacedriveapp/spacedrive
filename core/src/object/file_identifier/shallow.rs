use crate::{
	invalidate_query,
	job::JobError,
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_file_identifier, IsolatedFilePathData,
	},
	prisma::{file_path, location, PrismaClient, SortOrder},
	util::db::maybe_missing,
};

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use super::{process_identifier_file_paths, FileIdentifierJobError, CHUNK_SIZE};

#[derive(Serialize, Deserialize)]
pub struct ShallowFileIdentifierJobState {
	cursor: file_path::id::Type,
	sub_iso_file_path: IsolatedFilePathData<'static>,
}

pub async fn shallow(
	location: &location::Data,
	sub_path: &PathBuf,
	library: &Library,
) -> Result<(), JobError> {
	let Library { db, .. } = library;

	debug!("Identifying orphan File Paths...");

	let location_id = location.id;
	let location_path = maybe_missing(&location.path, "location.path").map(Path::new)?;

	let sub_iso_file_path = if sub_path != Path::new("") {
		let full_path = ensure_sub_path_is_in_location(location_path, &sub_path)
			.await
			.map_err(FileIdentifierJobError::from)?;
		ensure_sub_path_is_directory(location_path, &sub_path)
			.await
			.map_err(FileIdentifierJobError::from)?;

		let sub_iso_file_path =
			IsolatedFilePathData::new(location_id, location_path, &full_path, true)
				.map_err(FileIdentifierJobError::from)?;

		ensure_file_path_exists(
			&sub_path,
			&sub_iso_file_path,
			db,
			FileIdentifierJobError::SubPathNotFound,
		)
		.await?;

		sub_iso_file_path
	} else {
		IsolatedFilePathData::new(location_id, location_path, location_path, true)
			.map_err(FileIdentifierJobError::from)?
	};

	let orphan_count = count_orphan_file_paths(db, location_id, &sub_iso_file_path).await?;

	if orphan_count == 0 {
		return Ok(());
	}

	let task_count = (orphan_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
	debug!(
		"Found {} orphan Paths. Will execute {} tasks...",
		orphan_count, task_count
	);

	let first_path = db
		.file_path()
		.find_first(orphan_path_filters(location_id, None, &sub_iso_file_path))
		// .order_by(file_path::id::order(Direction::Asc))
		.select(file_path::select!({ id }))
		.exec()
		.await?
		.expect("We already validated before that there are orphans `file_path`s");

	// Initializing `state.data` here because we need a complete state in case of early finish
	let mut data = ShallowFileIdentifierJobState {
		cursor: first_path.id,
		sub_iso_file_path,
	};

	for step_number in 0..task_count {
		let ShallowFileIdentifierJobState {
			cursor,
			sub_iso_file_path,
		} = &mut data;

		// get chunk of orphans to process
		let file_paths =
			get_orphan_file_paths(&library.db, location.id, *cursor, sub_iso_file_path).await?;

		let (_, _, new_cursor) = process_identifier_file_paths(
			location,
			&file_paths,
			step_number,
			*cursor,
			library,
			orphan_count,
		)
		.await?;
		*cursor = new_cursor;
	}

	invalidate_query!(library, "search.paths");

	Ok(())
}

fn orphan_path_filters(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	sub_iso_file_path: &IsolatedFilePathData<'_>,
) -> Vec<file_path::WhereParam> {
	sd_utils::chain_optional_iter(
		[
			file_path::object_id::equals(None),
			file_path::is_dir::equals(Some(false)),
			file_path::location_id::equals(Some(location_id)),
			file_path::materialized_path::equals(Some(
				sub_iso_file_path
					.materialized_path_for_children()
					.expect("sub path for shallow identifier must be a directory"),
			)),
		],
		[file_path_id.map(file_path::id::gte)],
	)
}

async fn count_orphan_file_paths(
	db: &PrismaClient,
	location_id: location::id::Type,
	sub_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<usize, prisma_client_rust::QueryError> {
	db.file_path()
		.count(orphan_path_filters(location_id, None, sub_iso_file_path))
		.exec()
		.await
		.map(|c| c as usize)
}

async fn get_orphan_file_paths(
	db: &PrismaClient,
	location_id: location::id::Type,
	file_path_id_cursor: file_path::id::Type,
	sub_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<Vec<file_path_for_file_identifier::Data>, prisma_client_rust::QueryError> {
	trace!(
		"Querying {} orphan Paths at cursor: {:?}",
		CHUNK_SIZE,
		file_path_id_cursor
	);
	db.file_path()
		.find_many(orphan_path_filters(
			location_id,
			Some(file_path_id_cursor),
			sub_iso_file_path,
		))
		.order_by(file_path::id::order(SortOrder::Asc))
		// .cursor(cursor.into())
		.take(CHUNK_SIZE as i64)
		// .skip(1)
		.select(file_path_for_file_identifier::select())
		.exec()
		.await
}
