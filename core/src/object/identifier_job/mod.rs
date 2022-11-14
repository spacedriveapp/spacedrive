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
use tracing::{error, info};

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

pub async fn assemble_object_metadata(
	location_path: impl AsRef<Path>,
	file_path: &file_path::Data,
) -> Result<(String, String, Vec<object::SetParam>), io::Error> {
	assert!(
		!file_path.is_dir,
		"We can't generate cas_id for directories"
	);

	let path = location_path
		.as_ref()
		.join(file_path.materialized_path.as_str());

	let metadata = fs::metadata(&path).await?;

	// derive Object kind
	let object_kind = match path.extension() {
		Some(ext) => match ext.to_str() {
			Some(ext) => {
				let mut file = fs::File::open(&path).await?;

				Extension::resolve_conflicting(&ext.to_lowercase(), &mut file, false)
					.await
					.map(Into::into)
					.unwrap_or(ObjectKind::Unknown)
			}
			None => ObjectKind::Unknown,
		},
		None => ObjectKind::Unknown,
	};

	let size = metadata.len();

	let cas_id = generate_cas_id(&path, size).await?;

	info!("Analyzed file: {:?} {:?} {:?}", path, cas_id, object_kind);

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
	cas_id_lookup: &HashMap<String, Vec<i32>>,
) -> Result<Vec<file_path::Data>, QueryError> {
	let mut file_path_updates = Vec::new();

	objects.iter().for_each(|object| {
		let file_path_ids = cas_id_lookup.get(&object.cas_id).unwrap();

		file_path_updates.extend(file_path_ids.iter().map(|file_path_id| {
			info!(
				"Linking: <file_path_id = '{}', object_id = '{}'>",
				file_path_id, object.id
			);
			library.db.file_path().update(
				file_path::location_id_id(location_id, *file_path_id),
				vec![file_path::object_id::set(Some(object.id))],
			)
		}));
	});

	info!(
		"Updating {} file paths for {} objects",
		file_path_updates.len(),
		objects.len()
	);

	library.db._batch(file_path_updates).await
}

async fn generate_provisional_objects(
	location_path: impl AsRef<Path>,
	file_paths: &[file_path::Data],
) -> HashMap<i32, (String, String, Vec<object::SetParam>)> {
	let mut provisional_objects = HashMap::with_capacity(file_paths.len());

	// analyze each file_path
	let location_path = location_path.as_ref();
	for (file_path_id, objects_result) in join_all(file_paths.iter().map(|file_path| async move {
		(
			file_path.id,
			assemble_object_metadata(location_path, file_path).await,
		)
	}))
	.await
	{
		// get the cas_id and extract metadata
		match objects_result {
			Ok((cas_id, size, params)) => {
				// create entry into chunks for created file data
				provisional_objects.insert(file_path_id, (cas_id.clone(), size, params));
			}
			Err(e) => {
				error!("Error assembling Object metadata: {:#?}", e);
				continue;
			}
		};
	}
	provisional_objects
}

async fn identifier_job_step(
	library: &LibraryContext,
	location_id: i32,
	location_path: impl AsRef<Path>,
	file_paths: &[file_path::Data],
) -> Result<(usize, usize), JobError> {
	let location_path = location_path.as_ref();

	// generate objects for all file paths
	let provisional_objects = generate_provisional_objects(location_path, file_paths).await;

	let unique_cas_ids = provisional_objects
		.values()
		.map(|(cas_id, _, _)| cas_id.clone())
		.collect::<HashSet<_>>()
		.into_iter()
		.collect::<Vec<_>>();

	// allow easy lookup of cas_id to many file_path_ids
	let mut cas_id_lookup: HashMap<String, Vec<i32>> = HashMap::with_capacity(unique_cas_ids.len());

	// populate cas_id_lookup with file_path_ids
	for (file_path_id, (cas_id, _, _)) in provisional_objects.iter() {
		cas_id_lookup
			.entry(cas_id.clone())
			.or_insert_with(Vec::new)
			.push(*file_path_id);
	}

	// info!("{:#?}", cas_id_lookup);

	// get all objects that already exist in the database
	let existing_objects = library
		.db
		.object()
		.find_many(vec![object::cas_id::in_vec(unique_cas_ids)])
		.exec()
		.await?;

	info!(
		"Found {} existing Objects in Library, linking file paths...",
		existing_objects.len()
	);

	let existing_objects_linked = if !existing_objects.is_empty() {
		// link file_path.object_id to existing objects
		batch_update_file_paths(library, location_id, &existing_objects, &cas_id_lookup)
			.await?
			.len()
	} else {
		0
	};

	let existing_object_cas_ids = existing_objects
		.iter()
		.map(|object| object.cas_id.clone())
		.collect::<HashSet<_>>();

	// extract objects that don't already exist in the database
	let new_objects = provisional_objects
		.into_iter()
		.filter(|(_, (cas_id, _, _))| !existing_object_cas_ids.contains(cas_id))
		.collect::<Vec<_>>();

	let new_objects_cas_ids = new_objects
		.iter()
		.map(|(_, (cas_id, _, _))| cas_id.clone())
		.collect::<Vec<_>>();

	info!(
		"Creating {} new Objects in Library... {:#?}",
		new_objects.len(),
		new_objects_cas_ids
	);

	let mut total_created: usize = 0;
	if !new_objects.is_empty() {
		// create new object records with assembled values
		let total_created_files = library
			.db
			.object()
			.create_many(
				new_objects
					.into_iter()
					.map(|(_, (cas_id, size, params))| (cas_id, size, params))
					.collect(),
			)
			.skip_duplicates()
			.exec()
			.await
			.unwrap_or_else(|e| {
				error!("Error inserting files: {:#?}", e);
				0
			});

		total_created = total_created_files as usize;

		info!("Created {} new Objects in Library", total_created);

		// fetch newly created objects so we can link them to file_paths by their id
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

		info!(
			"Retrieved {} newly created Objects in Library",
			created_files.len()
		);

		if !created_files.is_empty() {
			batch_update_file_paths(library, location_id, &created_files, &cas_id_lookup).await?;
		}
	}

	Ok((total_created, existing_objects_linked))
}
