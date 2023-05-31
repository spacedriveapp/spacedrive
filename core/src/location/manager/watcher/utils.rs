use crate::{
	invalidate_query,
	library::Library,
	location::{
		delete_directory,
		file_path_helper::{
			check_existing_file_path, create_file_path, file_path_with_object,
			filter_existing_file_path_params, get_parent_dir,
			isolated_file_path_data::extract_normalized_materialized_path_str,
			loose_find_existing_file_path_params, FilePathError, FilePathMetadata,
			IsolatedFilePathData, MetadataExt,
		},
		find_location, location_with_indexer_rules,
		manager::LocationManagerError,
		scan_location_sub_path, LocationId,
	},
	object::{
		file_identifier::FileMetadata,
		object_just_id_has_thumbnail,
		preview::{can_generate_thumbnail_for_image, generate_image_thumbnail, get_thumbnail_path},
		validation::hash::file_checksum,
	},
	prisma::{file_path, location, object},
	sync,
	util::error::FileIOError,
};

#[cfg(target_family = "unix")]
use crate::location::file_path_helper::get_inode_and_device;

#[cfg(target_family = "windows")]
use crate::location::file_path_helper::get_inode_and_device_from_path;

use std::{
	collections::HashSet,
	fs::Metadata,
	path::{Path, PathBuf},
	str::FromStr,
};

use sd_file_ext::extensions::ImageExtension;

use chrono::{DateTime, Local};
use notify::{Event, EventKind};
use prisma_client_rust::{raw, PrismaValue};
use serde_json::json;
use tokio::{fs, io::ErrorKind};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::INodeAndDevice;

pub(super) fn check_event(event: &Event, ignore_paths: &HashSet<PathBuf>) -> bool {
	// if path includes .DS_Store, .spacedrive file creation or is in the `ignore_paths` set, we ignore
	!event.paths.iter().any(|p| {
		let path_str = p.to_str().expect("Found non-UTF-8 path");

		path_str.contains(".DS_Store")
			|| (path_str.contains(".spacedrive") && matches!(event.kind, EventKind::Create(_)))
			|| ignore_paths.contains(p)
	})
}

pub(super) async fn create_dir(
	location_id: LocationId,
	path: impl AsRef<Path>,
	metadata: &Metadata,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let location = find_location(library, location_id)
		.include(location_with_indexer_rules::include())
		.exec()
		.await?
		.ok_or(LocationManagerError::MissingLocation(location_id))?;

	let path = path.as_ref();

	trace!(
		"Location: <root_path ='{}'> creating directory: {}",
		location.path,
		path.display()
	);

	let materialized_path = IsolatedFilePathData::new(location.id, &location.path, path, true)?;

	let (inode, device) = {
		#[cfg(target_family = "unix")]
		{
			get_inode_and_device(metadata)?
		}

		#[cfg(target_family = "windows")]
		{
			// FIXME: This is a workaround for Windows, because we can't get the inode and device from the metadata
			let _ = metadata; // To avoid unused variable warning
			get_inode_and_device_from_path(&path).await?
		}
	};

	let parent_directory = get_parent_dir(&materialized_path, &library.db).await?;

	trace!("parent_directory: {:?}", parent_directory);

	if parent_directory.is_none() {
		warn!("Watcher found a directory without parent");
		return Ok(());
	};

	let created_path = create_file_path(
		library,
		materialized_path,
		None,
		FilePathMetadata {
			inode,
			device,
			size_in_bytes: metadata.len(),
			created_at: metadata.created_or_now().into(),
			modified_at: metadata.modified_or_now().into(),
		},
	)
	.await?;

	info!("Created path: {}", created_path.materialized_path);

	// scan the new directory
	scan_location_sub_path(library, location, &created_path.materialized_path).await?;

	invalidate_query!(library, "search.paths");

	Ok(())
}

pub(super) async fn create_file(
	location_id: LocationId,
	path: impl AsRef<Path>,
	metadata: &Metadata,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let path = path.as_ref();
	let location_path = extract_location_path(location_id, library).await?;

	trace!(
		"Location: <root_path ='{}'> creating file: {}",
		location_path.display(),
		path.display()
	);

	let db = &library.db;

	let iso_file_path = IsolatedFilePathData::new(location_id, &location_path, path, false)?;

	let (inode, device) = {
		#[cfg(target_family = "unix")]
		{
			get_inode_and_device(metadata)?
		}

		#[cfg(target_family = "windows")]
		{
			// FIXME: This is a workaround for Windows, because we can't get the inode and device from the metadata
			let _ = metadata; // To avoid unused variable warning
			get_inode_and_device_from_path(path).await?
		}
	};

	if get_parent_dir(&iso_file_path, db).await?.is_none() {
		warn!("Watcher found a file without parent");
		return Ok(());
	};

	// generate provisional object
	let FileMetadata {
		cas_id,
		kind,
		fs_metadata,
	} = FileMetadata::new(&location_path, &iso_file_path)
		.await
		.map_err(|e| FileIOError::from((location_path.join(&iso_file_path), e)))?;

	let created_file = create_file_path(
		library,
		iso_file_path,
		Some(cas_id.clone()),
		FilePathMetadata {
			inode,
			device,
			size_in_bytes: metadata.len(),
			created_at: metadata.created_or_now().into(),
			modified_at: metadata.modified_or_now().into(),
		},
	)
	.await?;

	info!("Created path: {}", created_file.materialized_path);

	let existing_object = db
		.object()
		.find_first(vec![object::file_paths::some(vec![
			file_path::cas_id::equals(Some(cas_id.clone())),
			file_path::pub_id::not(created_file.pub_id.clone()),
		])])
		.select(object_just_id_has_thumbnail::select())
		.exec()
		.await?;

	let object = if let Some(object) = existing_object {
		object
	} else {
		db.object()
			.create(
				Uuid::new_v4().as_bytes().to_vec(),
				vec![
					object::date_created::set(
						DateTime::<Local>::from(fs_metadata.created_or_now()).into(),
					),
					object::kind::set(kind as i32),
				],
			)
			.select(object_just_id_has_thumbnail::select())
			.exec()
			.await?
	};

	db.file_path()
		.update(
			file_path::pub_id::equals(created_file.pub_id),
			vec![file_path::object::connect(object::id::equals(object.id))],
		)
		.exec()
		.await?;

	if !object.has_thumbnail && !created_file.extension.is_empty() {
		// Running in a detached task as thumbnail generation can take a while and we don't want to block the watcher
		let path = path.to_path_buf();
		let library = library.clone();
		tokio::spawn(async move {
			generate_thumbnail(&created_file.extension, &cas_id, path, &library).await;
		});
	}

	invalidate_query!(library, "search.paths");

	Ok(())
}

pub(super) async fn create_dir_or_file(
	location_id: LocationId,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<Metadata, LocationManagerError> {
	let path = path.as_ref();
	let metadata = fs::metadata(path)
		.await
		.map_err(|e| FileIOError::from((path, e)))?;

	if metadata.is_dir() {
		create_dir(location_id, path, &metadata, library).await
	} else {
		create_file(location_id, path, &metadata, library).await
	}
	.map(|_| metadata)
}

pub(super) async fn file_creation_or_update(
	location_id: LocationId,
	full_path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = extract_location_path(location_id, library).await?;

	if let Some(ref file_path) = library
		.db
		.file_path()
		.find_first(filter_existing_file_path_params(
			&IsolatedFilePathData::new(location_id, &location_path, full_path, false)?,
		))
		// include object for orphan check
		.include(file_path_with_object::include())
		.exec()
		.await?
	{
		inner_update_file(location_id, file_path, full_path, library).await
	} else {
		create_file(
			location_id,
			full_path,
			&fs::metadata(full_path)
				.await
				.map_err(|e| FileIOError::from((full_path, e)))?,
			library,
		)
		.await
	}
}

pub(super) async fn update_file(
	location_id: LocationId,
	full_path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = extract_location_path(location_id, library).await?;

	if let Some(ref file_path) = library
		.db
		.file_path()
		.find_first(filter_existing_file_path_params(
			&IsolatedFilePathData::new(location_id, &location_path, full_path, false)?,
		))
		// include object for orphan check
		.include(file_path_with_object::include())
		.exec()
		.await?
	{
		let ret = inner_update_file(location_id, file_path, full_path, library).await;
		invalidate_query!(library, "search.paths");
		ret
	} else {
		// FIXME(fogodev): Have to handle files excluded by indexer rules
		Err(LocationManagerError::UpdateNonExistingFile(
			full_path.to_path_buf(),
		))
	}
}

async fn inner_update_file(
	location_id: LocationId,
	file_path: &file_path_with_object::Data,
	full_path: impl AsRef<Path>,
	library @ Library { db, sync, .. }: &Library,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.ok_or_else(|| LocationManagerError::MissingLocation(location_id))?;

	let location_path = PathBuf::from(location.path);

	trace!(
		"Location: <root_path ='{}'> updating file: {}",
		location_path.display(),
		full_path.display()
	);

	let iso_file_path = IsolatedFilePathData::from(file_path);

	let FileMetadata {
		cas_id,
		fs_metadata,
		kind,
	} = FileMetadata::new(&location_path, &iso_file_path)
		.await
		.map_err(|e| FileIOError::from((location_path.join(&iso_file_path), e)))?;

	if let Some(old_cas_id) = &file_path.cas_id {
		if old_cas_id != &cas_id {
			let (sync_params, db_params): (Vec<_>, Vec<_>) = {
				use file_path::*;

				[
					(
						(cas_id::NAME, json!(old_cas_id)),
						cas_id::set(Some(old_cas_id.clone())),
					),
					(
						(size_in_bytes::NAME, json!(fs_metadata.len().to_string())),
						size_in_bytes::set(fs_metadata.len().to_string()),
					),
					{
						let date = DateTime::<Local>::from(fs_metadata.modified_or_now()).into();

						((date_modified::NAME, json!(date)), date_modified::set(date))
					},
					{
						// TODO: Should this be a skip rather than a null-set?
						let checksum = if file_path.integrity_checksum.is_some() {
							// If a checksum was already computed, we need to recompute it
							Some(
								file_checksum(full_path)
									.await
									.map_err(|e| FileIOError::from((full_path, e)))?,
							)
						} else {
							None
						};

						(
							(integrity_checksum::NAME, json!(checksum)),
							integrity_checksum::set(checksum),
						)
					},
				]
				.into_iter()
				.unzip()
			};

			// file content changed
			sync.write_ops(
				db,
				(
					sync_params
						.into_iter()
						.map(|(field, value)| {
							sync.shared_update(
								sync::file_path::SyncId {
									pub_id: file_path.pub_id.clone(),
								},
								field,
								value,
							)
						})
						.collect(),
					db.file_path().update(
						file_path::pub_id::equals(file_path.pub_id.clone()),
						db_params,
					),
				),
			)
			.await?;

			if let Some(ref object) = file_path.object {
				// if this file had a thumbnail previously, we update it to match the new content
				if library.thumbnail_exists(old_cas_id).await? && !file_path.extension.is_empty() {
					generate_thumbnail(&file_path.extension, &cas_id, full_path, library).await;

					// remove the old thumbnail as we're generating a new one
					let thumb_path = get_thumbnail_path(library, old_cas_id);
					fs::remove_file(&thumb_path)
						.await
						.map_err(|e| FileIOError::from((thumb_path, e)))?;
				}

				let int_kind = kind as i32;

				if object.kind != int_kind {
					sync.write_op(
						db,
						sync.shared_update(
							sync::object::SyncId {
								pub_id: object.pub_id.clone(),
							},
							object::kind::NAME,
							json!(int_kind),
						),
						db.object().update(
							object::id::equals(object.id),
							vec![object::kind::set(int_kind)],
						),
					)
					.await?;
				}
			}

			invalidate_query!(library, "search.paths");
		}
	}

	Ok(())
}

pub(super) async fn rename(
	location_id: LocationId,
	new_path: impl AsRef<Path>,
	old_path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let location_path = extract_location_path(location_id, library).await?;
	let old_path = old_path.as_ref();
	let new_path = new_path.as_ref();
	let Library { db, .. } = library;

	let old_path_materialized_str =
		extract_normalized_materialized_path_str(location_id, &location_path, old_path)?;

	let new_path_materialized_str =
		extract_normalized_materialized_path_str(location_id, &location_path, new_path)?;

	// Renaming a file could potentially be a move to another directory, so we check if our parent changed
	if old_path_materialized_str != new_path_materialized_str
		&& !check_existing_file_path(
			&IsolatedFilePathData::new(location_id, &location_path, new_path, true)?.parent(),
			db,
		)
		.await?
	{
		return Err(LocationManagerError::MoveError {
			path: new_path.into(),
			reason: "parent directory does not exist".into(),
		});
	}

	if let Some(file_path) = db
		.file_path()
		.find_first(loose_find_existing_file_path_params(
			&IsolatedFilePathData::new(location_id, &location_path, old_path, true)?,
		))
		.exec()
		.await?
	{
		let new =
			IsolatedFilePathData::new(location_id, &location_path, new_path, file_path.is_dir)?;

		// If the renamed path is a directory, we have to update every successor
		if file_path.is_dir {
			let old =
				IsolatedFilePathData::new(location_id, &location_path, old_path, file_path.is_dir)?;
			// TODO: Fetch all file_paths that will be updated and dispatch sync events

			let updated = library
				.db
				._execute_raw(raw!(
					"UPDATE file_path \
						SET materialized_path = REPLACE(materialized_path, {}, {}) \
						WHERE location_id = {}",
					PrismaValue::String(format!("{}/{}/", old.materialized_path, old.name)),
					PrismaValue::String(format!("{}/{}/", new.materialized_path, new.name)),
					PrismaValue::Int(location_id as i64)
				))
				.exec()
				.await?;
			trace!("Updated {updated} file_paths");
		}

		library
			.db
			.file_path()
			.update(
				file_path::pub_id::equals(file_path.pub_id),
				vec![
					file_path::materialized_path::set(new_path_materialized_str),
					file_path::name::set(new.name.to_string()),
					file_path::extension::set(new.extension.to_string()),
				],
			)
			.exec()
			.await?;

		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

pub(super) async fn remove(
	location_id: LocationId,
	full_path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = extract_location_path(location_id, library).await?;

	// if it doesn't exist either way, then we don't care
	let Some(file_path) = library.db
		.file_path()
		.find_first(loose_find_existing_file_path_params(
			&IsolatedFilePathData::new(location_id, &location_path, full_path, true)?,
		))
		.exec()
		.await? else {
			return Ok(());
	};

	remove_by_file_path(location_id, full_path, &file_path, library).await
}

pub(super) async fn remove_by_file_path(
	location_id: LocationId,
	path: impl AsRef<Path>,
	file_path: &file_path::Data,
	library: &Library,
) -> Result<(), LocationManagerError> {
	// check file still exists on disk
	match fs::metadata(path.as_ref()).await {
		Ok(_) => {
			todo!("file has changed in some way, re-identify it")
		}
		Err(e) if e.kind() == ErrorKind::NotFound => {
			let db = &library.db;

			// if is doesn't, we can remove it safely from our db
			if file_path.is_dir {
				delete_directory(
					library,
					location_id,
					Some(file_path.materialized_path.clone()),
				)
				.await?;
			} else {
				db.file_path()
					.delete(file_path::pub_id::equals(file_path.pub_id.clone()))
					.exec()
					.await?;

				if let Some(object_id) = file_path.object_id {
					db.object()
						.delete_many(vec![
							object::id::equals(object_id),
							// https://www.prisma.io/docs/reference/api-reference/prisma-client-reference#none
							object::file_paths::none(vec![]),
						])
						.exec()
						.await?;
				}
			}

			library.orphan_remover.invoke().await;
		}
		Err(e) => return Err(FileIOError::from((path, e)).into()),
	}

	invalidate_query!(library, "search.paths");

	Ok(())
}

async fn generate_thumbnail(
	extension: &str,
	cas_id: &str,
	path: impl AsRef<Path>,
	library: &Library,
) {
	let path = path.as_ref();
	let output_path = get_thumbnail_path(library, cas_id);

	if let Err(e) = fs::metadata(&output_path).await {
		if e.kind() != ErrorKind::NotFound {
			error!(
				"Failed to check if thumbnail exists, but we will try to generate it anyway: {e}"
			);
		}
	// Otherwise we good, thumbnail doesn't exist so we can generate it
	} else {
		debug!(
			"Skipping thumbnail generation for {} because it already exists",
			path.display()
		);
		return;
	}

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(&extension) {
			if let Err(e) = generate_image_thumbnail(path, &output_path).await {
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
				if let Err(e) = generate_video_thumbnail(path, &output_path).await {
					error!("Failed to video thumbnail on location manager: {e:#?}");
				}
			}
		}
	}
}

pub(super) async fn extract_inode_and_device_from_path(
	location_id: LocationId,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<INodeAndDevice, LocationManagerError> {
	let path = path.as_ref();
	let location = find_location(library, location_id)
		.select(location::select!({ path }))
		.exec()
		.await?
		.ok_or(LocationManagerError::MissingLocation(location_id))?;

	library
		.db
		.file_path()
		.find_first(loose_find_existing_file_path_params(
			&IsolatedFilePathData::new(location_id, &location.path, path, true)?,
		))
		.select(file_path::select!( {inode device} ))
		.exec()
		.await?
		.map_or(
			Err(FilePathError::NotFound(path.into()).into()),
			|file_path| {
				Ok((
					u64::from_le_bytes(
						file_path.inode[0..8]
							.try_into()
							.map_err(|_| LocationManagerError::InvalidInode)?,
					),
					u64::from_le_bytes(
						file_path.device[0..8]
							.try_into()
							.map_err(|_| LocationManagerError::InvalidDevice)?,
					),
				))
			},
		)
}

pub(super) async fn extract_location_path(
	location_id: LocationId,
	library: &Library,
) -> Result<PathBuf, LocationManagerError> {
	find_location(library, location_id)
		.select(location::select!({ path }))
		.exec()
		.await?
		.map_or(
			Err(LocationManagerError::MissingLocation(location_id)),
			// NOTE: The following usage of `PathBuf` doesn't incur a new allocation so it's fine
			|location| Ok(PathBuf::from(location.path)),
		)
}
