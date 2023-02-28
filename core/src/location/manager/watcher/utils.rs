use crate::{
	invalidate_query,
	library::Library,
	location::{
		delete_directory,
		file_path_helper::{
			extract_materialized_path, file_path_with_object, get_existing_file_or_directory,
			get_existing_file_path_with_object, get_parent_dir, MaterializedPath,
		},
		location_with_indexer_rules,
		manager::LocationManagerError,
	},
	object::{
		file_identifier::FileMetadata,
		object_just_id_has_thumbnail,
		preview::{
			can_generate_thumbnail_for_image, generate_image_thumbnail, THUMBNAIL_CACHE_DIR_NAME,
		},
		validation::hash::file_checksum,
	},
	prisma::{file_path, object},
};

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	str::FromStr,
};

use chrono::{DateTime, FixedOffset, Local, Utc};
use int_enum::IntEnum;
use notify::{event::RemoveKind, Event};
use prisma_client_rust::{raw, PrismaValue};
use sd_file_ext::extensions::ImageExtension;
use tokio::{fs, io::ErrorKind};
use tracing::{error, info, trace, warn};
use uuid::Uuid;

pub(super) fn check_event(event: &Event, ignore_paths: &HashSet<PathBuf>) -> bool {
	// if path includes .DS_Store, .spacedrive or is in the `ignore_paths` set, we ignore
	!event.paths.iter().any(|p| {
		let path_str = p.to_str().expect("Found non-UTF-8 path");

		path_str.contains(".DS_Store")
			|| path_str.contains(".spacedrive")
			|| ignore_paths.contains(p)
	})
}

pub(super) async fn create_dir(
	location: &location_with_indexer_rules::Data,
	event: &Event,
	library: &Library,
) -> Result<(), LocationManagerError> {
	create_dir_by_path(location, &event.paths[0], library).await
}

pub(super) async fn create_dir_by_path(
	location: &location_with_indexer_rules::Data,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	if location.node_id != library.node_local_id {
		return Ok(());
	}

	let path = path.as_ref();

	trace!(
		"Location: <root_path ='{}'> creating directory: {}",
		location.path,
		path.display()
	);

	let materialized_path =
		MaterializedPath::new(location.id, &location.path, path, true)?;

	let parent_directory = get_parent_dir(&materialized_path, &library.db).await?;

	trace!("parent_directory: {:?}", parent_directory);

	let Some(parent_directory) = parent_directory else {
		warn!("Watcher found a path without parent");
        return Ok(())
	};

	let created_path = library
		.last_file_path_id_manager
		.create_file_path(&library.db, materialized_path, Some(parent_directory.id))
		.await?;

	info!("Created path: {}", created_path.materialized_path);

	invalidate_query!(library, "locations.getExplorerData");

	Ok(())
}

pub(super) async fn create_file(
	location: &location_with_indexer_rules::Data,
	event: &Event,
	library: &Library,
) -> Result<(), LocationManagerError> {
	create_file_by_path(location, &event.paths[0], library).await
}

pub(super) async fn create_file_by_path(
	location: &location_with_indexer_rules::Data,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let path = path.as_ref();
	if location.node_id != library.node_local_id {
		return Ok(());
	}

	trace!(
		"Location: <root_path ='{}'> creating file: {}",
		&location.path,
		path.display()
	);

	let db = &library.db;

	let materialized_path = MaterializedPath::new(location.id, &location.path, path, false)?;

	let Some(parent_directory) =
		get_parent_dir(&materialized_path, &library.db).await?
    else {
		warn!("Watcher found a path without parent");
        return Ok(())
    };

	let created_file = library
		.last_file_path_id_manager
		.create_file_path(&library.db, materialized_path, Some(parent_directory.id))
		.await?;

	info!("Created path: {}", created_file.materialized_path);

	// generate provisional object
	let FileMetadata {
		cas_id,
		kind,
		fs_metadata,
	} = FileMetadata::new(&location.path, &created_file.materialized_path).await?;

	let existing_object = db
		.object()
		.find_first(vec![object::file_paths::some(vec![
			file_path::cas_id::equals(Some(cas_id.clone())),
		])])
		.exec()
		.await?;

	let size_str = fs_metadata.len().to_string();

	let object = if let Some(object) = existing_object {
		db.object()
			.update(
				object::id::equals(object.id),
				vec![
					object::size_in_bytes::set(size_str),
					object::date_indexed::set(
						Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
					),
				],
			)
			.select(object_just_id_has_thumbnail::select())
			.exec()
			.await?
	} else {
		db.object()
			.create(
				Uuid::new_v4().as_bytes().to_vec(),
				vec![
					object::date_created::set(
						DateTime::<Local>::from(fs_metadata.created().unwrap()).into(),
					),
					object::kind::set(kind.int_value()),
					object::size_in_bytes::set(size_str.clone()),
				],
			)
			.select(object_just_id_has_thumbnail::select())
			.exec()
			.await?
	};

	db.file_path()
		.update(
			file_path::location_id_id(location.id, created_file.id),
			vec![file_path::object_id::set(Some(object.id))],
		)
		.exec()
		.await?;

	trace!("object: {:#?}", object);
	if !object.has_thumbnail && !created_file.extension.is_empty() {
		generate_thumbnail(&created_file.extension, &cas_id, path, library).await;
	}

	invalidate_query!(library, "locations.getExplorerData");

	Ok(())
}

pub(super) async fn file_creation_or_update(
	location: &location_with_indexer_rules::Data,
	event: &Event,
	library: &Library,
) -> Result<(), LocationManagerError> {
	if let Some(ref file_path) = get_existing_file_path_with_object(
		MaterializedPath::new(location.id, &location.path, &event.paths[0], false)?,
		&library.db,
	)
	.await?
	{
		inner_update_file(location, file_path, event, library).await
	} else {
		// We received None because it is a new file
		create_file(location, event, library).await
	}
}

pub(super) async fn update_file(
	location: &location_with_indexer_rules::Data,
	event: &Event,
	library: &Library,
) -> Result<(), LocationManagerError> {
	if location.node_id == library.node_local_id {
		if let Some(ref file_path) = get_existing_file_path_with_object(
			MaterializedPath::new(location.id, &location.path, &event.paths[0], false)?,
			&library.db,
		)
		.await?
		{
			let ret = inner_update_file(location, file_path, event, library).await;
			invalidate_query!(library, "locations.getExplorerData");
			ret
		} else {
			Err(LocationManagerError::UpdateNonExistingFile(
				event.paths[0].clone(),
			))
		}
	} else {
		Err(LocationManagerError::LocationMissingLocalPath(location.id))
	}
}

async fn inner_update_file(
	location: &location_with_indexer_rules::Data,
	file_path: &file_path_with_object::Data,
	event: &Event,
	library: &Library,
) -> Result<(), LocationManagerError> {
	trace!(
		"Location: <root_path ='{}'> updating file: {}",
		&location.path,
		event.paths[0].display()
	);

	let FileMetadata {
		cas_id,
		fs_metadata,
		..
	} = FileMetadata::new(&location.path, &file_path.materialized_path).await?;

	if let Some(old_cas_id) = &file_path.cas_id {
		if old_cas_id != &cas_id {
			// file content changed
			library
				.db
				.file_path()
				.update(
					file_path::location_id_id(location.id, file_path.id),
					vec![
						file_path::cas_id::set(Some(old_cas_id.clone())),
						// file_path::size_in_bytes::set(fs_metadata.len().to_string()),
						// file_path::kind::set(kind.int_value()),
						file_path::date_modified::set(
							DateTime::<Local>::from(fs_metadata.created().unwrap()).into(),
						),
						file_path::integrity_checksum::set(
							if file_path.integrity_checksum.is_some() {
								// If a checksum was already computed, we need to recompute it
								Some(file_checksum(&event.paths[0]).await?)
							} else {
								None
							},
						),
					],
				)
				.exec()
				.await?;

			if file_path
				.object
				.as_ref()
				.map(|o| o.has_thumbnail)
				.unwrap_or_default()
			{
				// if this file had a thumbnail previously, we update it to match the new content
				if !file_path.extension.is_empty() {
					generate_thumbnail(&file_path.extension, &cas_id, &event.paths[0], library)
						.await;
				}
			}
		}
	}

	invalidate_query!(library, "locations.getExplorerData");

	Ok(())
}

pub(super) async fn rename_both_event(
	location: &location_with_indexer_rules::Data,
	event: &Event,
	library: &Library,
) -> Result<(), LocationManagerError> {
	rename(&event.paths[1], &event.paths[0], location, library).await
}

pub(super) async fn rename(
	new_path: impl AsRef<Path>,
	old_path: impl AsRef<Path>,
	location: &location_with_indexer_rules::Data,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let mut old_path_materialized =
		extract_materialized_path(location.id, &location.path, old_path.as_ref())?
			.to_str()
			.expect("Found non-UTF-8 path")
			.to_string();

	let new_path_materialized =
		extract_materialized_path(location.id, &location.path, new_path.as_ref())?;
	let mut new_path_materialized_str = new_path_materialized
		.to_str()
		.expect("Found non-UTF-8 path")
		.to_string();

	// TODO handle the case where a rename means a move to another directory in this location or outside the location which is actually a delete

	if let Some(file_path) = get_existing_file_or_directory(location, old_path, &library.db).await?
	{
		// If the renamed path is a directory, we have to update every successor
		if file_path.is_dir {
			if !old_path_materialized.ends_with('/') {
				old_path_materialized += "/";
			}
			if !new_path_materialized_str.ends_with('/') {
				new_path_materialized_str += "/";
			}

			let updated = library
				.db
				._execute_raw(
					raw!(
						"UPDATE file_path SET materialized_path = REPLACE(materialized_path, {}, {}) WHERE location_id = {}",
						PrismaValue::String(old_path_materialized),
						PrismaValue::String(new_path_materialized_str.clone()),
						PrismaValue::Int(location.id as i64)
					)
				)
				.exec()
				.await?;
			trace!("Updated {updated} file_paths");
		}

		library
			.db
			.file_path()
			.update(
				file_path::location_id_id(file_path.location_id, file_path.id),
				vec![
					file_path::materialized_path::set(new_path_materialized_str),
					file_path::name::set(
						new_path_materialized
							.file_stem()
							.unwrap()
							.to_str()
							.expect("Found non-UTF-8 path")
							.to_string(),
					),
					file_path::extension::set(
						new_path_materialized
							.extension()
							.map(|s| {
								s.to_str()
									.expect("Found non-UTF-8 extension in path")
									.to_string()
							})
							.unwrap_or_default(),
					),
				],
			)
			.exec()
			.await?;
		invalidate_query!(library, "locations.getExplorerData");
	}

	Ok(())
}

pub(super) async fn remove_event(
	location: &location_with_indexer_rules::Data,
	event: &Event,
	remove_kind: RemoveKind,
	library: &Library,
) -> Result<(), LocationManagerError> {
	trace!("removed {remove_kind:#?}");

	remove_by_path(location, &event.paths[0], library).await
}

pub(super) async fn remove_by_path(
	location: &location_with_indexer_rules::Data,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let path = path.as_ref();

	// if it doesn't exist either way, then we don't care
	let Some(file_path) = get_existing_file_or_directory(
		location,
		path,
		&library.db
	).await? else {
		return Ok(());
	};

	remove_by_file_path(location, path, &file_path, library).await
}

pub(super) async fn remove_by_file_path(
	location: &location_with_indexer_rules::Data,
	path: impl AsRef<Path>,
	file_path: &file_path_with_object::Data,
	library: &Library,
) -> Result<(), LocationManagerError> {
	// check file still exists on disk
	match fs::metadata(path).await {
		Ok(_) => {
			todo!("file has changed in some way, re-identify it")
		}
		Err(e) if e.kind() == ErrorKind::NotFound => {
			// if is doesn't, we can remove it safely from our db
			if file_path.is_dir {
				delete_directory(
					library,
					location.id,
					Some(file_path.materialized_path.clone()),
				)
				.await?;
			} else {
				library
					.db
					.file_path()
					.delete(file_path::location_id_id(location.id, file_path.id))
					.exec()
					.await?;

				if let Some(object_id) = file_path.object_id {
					library
						.db
						.object()
						.delete_many(vec![
							object::id::equals(object_id),
							// https://www.prisma.io/docs/reference/api-reference/prisma-client-reference#none
							object::file_paths::none(vec![]),
						])
						.exec()
						.await?;
				}
			}
		}
		Err(e) => return Err(e.into()),
	}

	invalidate_query!(library, "locations.getExplorerData");

	Ok(())
}

async fn generate_thumbnail(
	extension: &str,
	cas_id: &str,
	file_path: impl AsRef<Path>,
	library: &Library,
) {
	let file_path = file_path.as_ref();
	let output_path = library
		.config()
		.data_directory()
		.join(THUMBNAIL_CACHE_DIR_NAME)
		.join(cas_id)
		.with_extension("webp");

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(&extension) {
			if let Err(e) = generate_image_thumbnail(file_path, &output_path).await {
				error!("Failed to image thumbnail on location manager: {e:#?}");
			}
		}
	}

	#[cfg(feature = "ffmpeg")]
	{
		use crate::object::preview::{can_generate_thumbnail_for_video, generate_video_thumbnail};
		use sd_file_ext::extensions::VideoExtension;

		if let Ok(extension) = VideoExtension::from_str(extension) {
			if can_generate_thumbnail_for_video(&extension) {
				if let Err(e) = generate_video_thumbnail(file_path, &output_path).await {
					error!("Failed to video thumbnail on location manager: {e:#?}");
				}
			}
		}
	}
}
