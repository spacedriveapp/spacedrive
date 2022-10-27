use crate::{
	job::JobError,
	library::LibraryContext,
	object::cas::generate_cas_id,
	prisma::{file_path, object},
};
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
};

use futures::future::join_all;
use int_enum::IntEnum;
use prisma_client_rust::QueryError;
use sd_file_ext::{extensions::Extension, kind::ObjectKind};
use thiserror::Error;
use tokio::{fs, io};
use tracing::{debug, error, info};

pub mod current_dir_identifier_job;
pub mod full_identifier_job;

// we break these jobs into chunks of 100 to improve performance
static CHUNK_SIZE: usize = 100;

#[derive(Error, Debug)]
pub enum IdentifierJobError {
	#[error("Location not found: <id = '{0}'>")]
	MissingLocation(i32),
	#[error("Root file path not found: <path = '{0}'>")]
	MissingRootFilePath(PathBuf),
	#[error("Location without local path: <id = '{0}'>")]
	LocationLocalPath(i32),
}

async fn assemble_object_metadata(
	location_path: impl AsRef<Path>,
	file_path: &file_path::Data,
) -> Result<(String, String, Vec<object::SetParam>), io::Error> {
	let path = location_path
		.as_ref()
		.join(file_path.materialized_path.as_str());

	info!("Reading path: {:?}", path);

	let metadata = fs::metadata(&path).await?;

	// derive Object kind
	let object_kind = match path.extension() {
		Some(ext) => match ext.to_str() {
			Some(ext) => {
				let mut file = fs::File::open(&path).await?;

				Extension::resolve_conflicting(ext, &mut file, true)
					.await
					.map(Into::into)
					.unwrap_or(ObjectKind::Unknown)
			}
			None => ObjectKind::Unknown,
		},
		None => ObjectKind::Unknown,
	};

	let size = metadata.len();

	let cas_id = {
		if !file_path.is_dir {
			let mut ret = generate_cas_id(path, size).await?;
			ret.truncate(16);
			ret
		} else {
			"".to_string()
		}
	};

	Ok(object::create_unchecked(
		cas_id,
		size.to_string(),
		vec![
			object::date_created::set(file_path.date_created),
			object::kind::set(object_kind.int_value()),
		],
	))
}

async fn batch_update_file_paths(
	library: &LibraryContext,
	location_id: i32,
	objects: &[object::Data],
	cas_lookup: &HashMap<String, i32>,
) -> Result<Vec<file_path::Data>, QueryError> {
	library
		.db
		._batch(
			objects
				.iter()
				.map(|object| {
					library.db.file_path().update(
						file_path::location_id_id(
							location_id,
							// SAFETY: This cas_id was put in the map before
							*cas_lookup.get(&object.cas_id).unwrap(),
						),
						vec![file_path::object_id::set(Some(object.id))],
					)
				})
				.collect::<Vec<_>>(),
		)
		.await
}

async fn prepare_object_data(
	location_path: impl AsRef<Path>,
	file_paths: &[file_path::Data],
) -> (
	Vec<(String, String, Vec<object::SetParam>)>,
	HashMap<String, i32>,
) {
	let mut objects_to_maybe_create = Vec::with_capacity(file_paths.len());
	let mut cas_lookup = HashMap::with_capacity(file_paths.len());

	// analyze each file_path
	let location_path = location_path.as_ref();
	for (file_path_id, objects_to_maybe_create_result) in
		join_all(file_paths.iter().map(|file_path| async move {
			(
				file_path.id,
				assemble_object_metadata(location_path, file_path).await,
			)
		}))
		.await
	{
		// get the cas_id and extract metadata
		match objects_to_maybe_create_result {
			Ok((cas_id, size, params)) => {
				// create entry into chunks for created file data
				objects_to_maybe_create.push((cas_id.clone(), size, params));
				cas_lookup.insert(cas_id, file_path_id);
			}
			Err(e) => {
				error!("Error assembling Object metadata: {:#?}", e);
				continue;
			}
		};
	}

	(objects_to_maybe_create, cas_lookup)
}

async fn identifier_job_step(
	library: &LibraryContext,
	location_id: i32,
	location_path: impl AsRef<Path>,
	file_paths: &[file_path::Data],
) -> Result<(), JobError> {
	let location_path = location_path.as_ref();

	let (objects_to_maybe_create, cas_lookup) =
		prepare_object_data(location_path, file_paths).await;

	// find all existing files by cas id
	let generated_cas_ids = objects_to_maybe_create
		.iter()
		.map(|(cas_id, _, _)| cas_id.clone())
		.collect();
	let existing_objects = library
		.db
		.object()
		.find_many(vec![object::cas_id::in_vec(generated_cas_ids)])
		.exec()
		.await?;

	info!("Found {} existing files", existing_objects.len());

	batch_update_file_paths(library, location_id, &existing_objects, &cas_lookup).await?;

	let existing_object_cas_ids = existing_objects
		.iter()
		.map(|object| object.cas_id.clone())
		.collect::<HashSet<_>>();

	// extract objects that don't already exist in the database
	let new_objects = objects_to_maybe_create
		.into_iter()
		.filter(|(cas_id, _, _)| !existing_object_cas_ids.contains(cas_id))
		.collect::<Vec<_>>();
	let new_objects_cas_ids = new_objects
		.iter()
		.map(|(cas_id, _, _)| cas_id.clone())
		.collect::<Vec<_>>();

	if !new_objects.is_empty() {
		// create new file records with assembled values
		let total_created_files = library
			.db
			.object()
			.create_many(new_objects)
			.skip_duplicates()
			.exec()
			.await
			.unwrap_or_else(|e| {
				error!("Error inserting files: {:#?}", e);
				0
			});

		debug!(
			"Created {} new files of {}",
			total_created_files,
			new_objects_cas_ids.len()
		);

		let created_files = library
			.db
			.object()
			.find_many(vec![object::cas_id::in_vec(new_objects_cas_ids)])
			.exec()
			.await
			.unwrap_or_else(|e| {
				error!("Error finding created files: {:#?}", e);
				vec![]
			});

		if !created_files.is_empty() {
			batch_update_file_paths(library, location_id, &created_files, &cas_lookup).await?;
		}
	}

	Ok(())
}
