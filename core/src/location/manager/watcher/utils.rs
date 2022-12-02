use crate::{
	invalidate_query,
	library::LibraryContext,
	location::{
		delete_directory,
		file_path_helper::create_file_path,
		indexer::indexer_job::indexer_job_location,
		manager::{LocationId, LocationManagerError},
		subtract_location_path,
	},
	object::{
		identifier_job::{assemble_object_metadata, ObjectCreationMetadata},
		preview::{
			can_generate_thumbnail_for_image, generate_image_thumbnail, THUMBNAIL_CACHE_DIR_NAME,
		},
		validation::hash::file_checksum,
	},
	prisma::{file_path, object},
};

use std::{
	path::{Path, PathBuf},
	str::FromStr,
};

use chrono::{FixedOffset, Utc};
use int_enum::IntEnum;
use notify::{event::RemoveKind, Event};
use prisma_client_rust::{raw, PrismaValue};
use sd_file_ext::extensions::ImageExtension;
use tokio::{fs, io::ErrorKind};
use tracing::{debug, error, info, warn};

use super::file_path_with_object;

pub(super) fn check_location_online(location: &indexer_job_location::Data) -> bool {
	// if location is offline return early
	// this prevents ....
	if !location.is_online {
		info!(
			"Location is offline, skipping event: <id='{}'>",
			location.id
		);
		false
	} else {
		true
	}
}

pub(super) fn check_event(event: &Event) -> bool {
	// if first path includes .DS_Store, ignore
	if event
		.paths
		.iter()
		.any(|p| p.to_string_lossy().contains(".DS_Store"))
	{
		return false;
	}

	true
}

pub(super) async fn create_dir(
	location: indexer_job_location::Data,
	event: Event,
	library_ctx: LibraryContext,
) -> Result<(), LocationManagerError> {
	if let Some(ref location_local_path) = location.local_path {
		debug!(
			"Location: <root_path ='{location_local_path}'> creating directory: {}",
			event.paths[0].display()
		);

		if let Some(subpath) = subtract_location_path(location_local_path, &event.paths[0]) {
			let subpath_str = subpath.to_string_lossy().to_string();
			let parent_directory = library_ctx
				.db
				.file_path()
				.find_first(vec![
					// We have an empty `materialized_path` for each location_id
					file_path::location_id::equals(location.id),
					file_path::materialized_path::equals(
						subpath
							.parent()
							.unwrap_or_else(|| Path::new(""))
							.to_string_lossy()
							.to_string(),
					),
				])
				.exec()
				.await?;

			debug!("parent_directory: {:?}", parent_directory);

			if let Some(parent_directory) = parent_directory {
				let created_path = create_file_path(
					&library_ctx,
					location.id,
					subpath_str,
					subpath.file_stem().unwrap().to_string_lossy().to_string(),
					None,
					Some(parent_directory.id),
					true,
				)
				.await?;

				info!("Created path: {}", created_path.materialized_path);

				invalidate_query!(library_ctx, "locations.getExplorerData");
			} else {
				warn!("Watcher found a path without parent");
			}
		}
	}

	Ok(())
}

pub(super) async fn create_file(
	location: indexer_job_location::Data,
	event: Event,
	library_ctx: LibraryContext,
) -> Result<(), LocationManagerError> {
	if let Some(ref location_local_path) = location.local_path {
		inner_create_file(location.id, location_local_path, event, &library_ctx).await
	} else {
		Err(LocationManagerError::LocationMissingLocalPath(location.id))
	}
}

async fn inner_create_file(
	location_id: LocationId,
	location_local_path: &str,
	event: Event,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	debug!(
		"Location: <root_path ='{location_local_path}'> creating file: {}",
		event.paths[0].display()
	);
	if let Some(materialized_path) = subtract_location_path(location_local_path, &event.paths[0]) {
		if let Some(parent_directory) =
			get_parent_dir(location_id, &materialized_path, library_ctx).await?
		{
			let created_file = create_file_path(
				library_ctx,
				location_id,
				materialized_path.to_string_lossy().to_string(),
				materialized_path
					.file_stem()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string(),
				materialized_path.extension().and_then(|ext| {
					if ext.is_empty() {
						None
					} else {
						Some(ext.to_string_lossy().to_string())
					}
				}),
				Some(parent_directory.id),
				false,
			)
			.await?;

			info!("Created path: {}", created_file.materialized_path);

			// generate provisional object
			let ObjectCreationMetadata {
				cas_id,
				size_str,
				kind,
				date_created,
			} = assemble_object_metadata(location_local_path, &created_file).await?;

			// upsert object because in can be from a file that previously existed and was moved
			let object = library_ctx
				.db
				.object()
				.upsert(
					object::cas_id::equals(cas_id.clone()),
					(
						cas_id.clone(),
						size_str.clone(),
						vec![
							object::date_created::set(date_created),
							object::kind::set(kind.int_value()),
						],
					),
					vec![
						object::size_in_bytes::set(size_str),
						object::date_indexed::set(
							Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
						),
					],
				)
				.exec()
				.await?;

			library_ctx
				.db
				.file_path()
				.update(
					file_path::location_id_id(location_id, created_file.id),
					vec![file_path::object_id::set(Some(object.id))],
				)
				.exec()
				.await?;

			debug!("object: {:#?}", object);
			if !object.has_thumbnail {
				if let Some(ref extension) = created_file.extension {
					generate_thumbnail(extension, &cas_id, &event.paths[0], library_ctx).await;
				}
			}

			invalidate_query!(library_ctx, "locations.getExplorerData");
		} else {
			warn!("Watcher found a path without parent");
		}
	}

	Ok(())
}

pub(super) async fn file_creation_or_update(
	location: indexer_job_location::Data,
	event: Event,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	if let Some(ref location_local_path) = location.local_path {
		if let Some(file_path) =
			get_existing_file_path(&location, &event.paths[0], library_ctx).await?
		{
			inner_update_file(location_local_path, file_path, event, library_ctx).await
		} else {
			// We received None because it is a new file
			inner_create_file(location.id, location_local_path, event, library_ctx).await
		}
	} else {
		Err(LocationManagerError::LocationMissingLocalPath(location.id))
	}
}

pub(super) async fn update_file(
	location: indexer_job_location::Data,
	event: Event,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	if let Some(ref location_local_path) = location.local_path {
		if let Some(file_path) =
			get_existing_file_path(&location, &event.paths[0], library_ctx).await?
		{
			inner_update_file(location_local_path, file_path, event, library_ctx).await
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
	location_local_path: &str,
	file_path: file_path_with_object::Data,
	event: Event,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	debug!(
		"Location: <root_path ='{location_local_path}'> updating file: {}",
		event.paths[0].display()
	);
	// We have to separate this object, as the `assemble_object_metadata` doesn't
	// accept `file_path_with_object::Data`
	let file_path_only = file_path::Data {
		id: file_path.id,
		is_dir: file_path.is_dir,
		location_id: file_path.location_id,
		location: None,
		materialized_path: file_path.materialized_path,
		name: file_path.name,
		extension: file_path.extension,
		object_id: file_path.object_id,
		object: None,
		parent_id: file_path.parent_id,
		key_id: file_path.key_id,
		date_created: file_path.date_created,
		date_modified: file_path.date_modified,
		date_indexed: file_path.date_indexed,
		key: None,
	};
	let ObjectCreationMetadata {
		cas_id,
		size_str,
		kind,
		date_created,
	} = assemble_object_metadata(location_local_path, &file_path_only).await?;

	if let Some(ref object) = file_path.object {
		if object.cas_id != cas_id {
			// file content changed
			library_ctx
				.db
				.object()
				.update(
					object::id::equals(object.id),
					vec![
						object::cas_id::set(cas_id.clone()),
						object::size_in_bytes::set(size_str),
						object::kind::set(kind.int_value()),
						object::date_modified::set(date_created),
						object::integrity_checksum::set(if object.integrity_checksum.is_some() {
							// If a checksum was already computed, we need to recompute it
							Some(file_checksum(&event.paths[0]).await?)
						} else {
							None
						}),
					],
				)
				.exec()
				.await?;

			if object.has_thumbnail {
				// if this file had a thumbnail previously, we update it to match the new content
				if let Some(ref extension) = file_path_only.extension {
					generate_thumbnail(extension, &cas_id, &event.paths[0], library_ctx).await;
				}
			}
		}
	}

	Ok(())
}

pub(super) async fn rename_both_event(
	location: indexer_job_location::Data,
	event: Event,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	rename(&event.paths[1], &event.paths[0], location, library_ctx).await
}

pub(super) async fn rename(
	new_path: impl AsRef<Path>,
	old_path: impl AsRef<Path>,
	location: indexer_job_location::Data,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	let old_path_materialized = extract_materialized_path(&location, old_path.as_ref())?
		.to_string_lossy()
		.to_string();
	let new_path_materialized = extract_materialized_path(&location, new_path.as_ref())?;

	if let Some(file_path) = get_existing_file_path(&location, old_path, library_ctx).await? {
		// If the renamed path is a directory, we have to update every successor
		if file_path.is_dir {
			let updated = library_ctx
				.db
				._execute_raw(
					raw!(
						"UPDATE file_path SET materialized_path = REPLACE(materialized_path, {}, {}) WHERE location_id = {}",
						PrismaValue::String(old_path_materialized),
						PrismaValue::String(
							new_path_materialized
								.to_string_lossy()
								.to_string()
						),
						PrismaValue::Int(location.id as i64)
					)
				)
				.exec()
				.await?;
			debug!("Updated {updated} file_paths");
		}

		library_ctx
			.db
			.file_path()
			.update(
				file_path::location_id_id(file_path.location_id, file_path.id),
				vec![
					file_path::materialized_path::set(
						new_path_materialized.to_string_lossy().to_string(),
					),
					file_path::name::set(
						new_path_materialized
							.file_stem()
							.unwrap()
							.to_string_lossy()
							.to_string(),
					),
					file_path::extension::set(
						new_path_materialized
							.extension()
							.map(|s| s.to_string_lossy().to_string()),
					),
				],
			)
			.exec()
			.await?;
	}

	Ok(())
}

pub(super) async fn remove_event(
	location: indexer_job_location::Data,
	event: Event,
	remove_kind: RemoveKind,
	library_ctx: &LibraryContext,
) -> Result<(), LocationManagerError> {
	debug!("removed {remove_kind:#?}");

	// check if path exists in our db, if it doesn't, then we don't care
	if let Some(file_path) = get_existing_file_path(&location, &event.paths[0], library_ctx).await?
	{
		// check file still exists on disk
		match fs::metadata(&event.paths[0]).await {
			Ok(_) => {
				todo!("file has changed in some way, re-identify it")
			}
			Err(e) if e.kind() == ErrorKind::NotFound => {
				// if is doesn't, we can remove it safely from our db
				if file_path.is_dir {
					delete_directory(library_ctx, location.id, Some(file_path.materialized_path))
						.await?;
				} else {
					library_ctx
						.db
						.file_path()
						.delete(file_path::location_id_id(location.id, file_path.id))
						.exec()
						.await?;

					if let Some(object_id) = file_path.object_id {
						library_ctx
							.db
							.object()
							.delete(object::id::equals(object_id))
							.exec()
							.await?;
					}
				}
			}
			Err(e) => return Err(e.into()),
		}
	}

	Ok(())
}

fn extract_materialized_path(
	location: &indexer_job_location::Data,
	path: impl AsRef<Path>,
) -> Result<PathBuf, LocationManagerError> {
	subtract_location_path(
		location
			.local_path
			.as_ref()
			.ok_or(LocationManagerError::LocationMissingLocalPath(location.id))?,
		&path,
	)
	.ok_or_else(|| {
		LocationManagerError::UnableToExtractMaterializedPath(
			location.id,
			path.as_ref().to_path_buf(),
		)
	})
}

async fn get_existing_file_path(
	location: &indexer_job_location::Data,
	path: impl AsRef<Path>,
	library_ctx: &LibraryContext,
) -> Result<Option<file_path_with_object::Data>, LocationManagerError> {
	library_ctx
		.db
		.file_path()
		.find_first(vec![file_path::materialized_path::equals(
			extract_materialized_path(location, path)?
				.to_string_lossy()
				.to_string(),
		)])
		// include object for orphan check
		.include(file_path_with_object::include())
		.exec()
		.await
		.map_err(Into::into)
}

async fn get_parent_dir(
	location_id: LocationId,
	path: impl AsRef<Path>,
	library_ctx: &LibraryContext,
) -> Result<Option<file_path::Data>, LocationManagerError> {
	library_ctx
		.db
		.file_path()
		.find_first(vec![
			// We have an empty `materialized_path` for each location_id
			file_path::location_id::equals(location_id),
			file_path::materialized_path::equals(
				path.as_ref()
					.parent()
					.unwrap_or_else(|| Path::new(""))
					.to_string_lossy()
					.to_string(),
			),
		])
		.exec()
		.await
		.map_err(Into::into)
}

async fn generate_thumbnail(
	extension: &str,
	cas_id: &str,
	file_path: impl AsRef<Path>,
	library_ctx: &LibraryContext,
) {
	let file_path = file_path.as_ref();
	let output_path = library_ctx
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
