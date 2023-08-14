use crate::{
	invalidate_query,
	library::Library,
	location::{
		delete_directory,
		file_path_helper::{
			check_file_path_exists, create_file_path, file_path_with_object,
			filter_existing_file_path_params,
			isolated_file_path_data::extract_normalized_materialized_path_str,
			loose_find_existing_file_path_params, FilePathError, FilePathMetadata,
			IsolatedFilePathData, MetadataExt,
		},
		find_location, generate_thumbnail, location_with_indexer_rules,
		manager::LocationManagerError,
		scan_location_sub_path,
	},
	object::{
		file_identifier::FileMetadata, preview::get_thumbnail_path, validation::hash::file_checksum,
	},
	prisma::{file_path, location, object},
	util::{
		db::{device_from_db, device_to_db, inode_from_db, inode_to_db, maybe_missing},
		error::FileIOError,
	},
	Node,
};

#[cfg(target_family = "unix")]
use crate::location::file_path_helper::get_inode_and_device;

#[cfg(target_family = "windows")]
use crate::location::file_path_helper::get_inode_and_device_from_path;

use std::{
	collections::HashSet,
	ffi::OsStr,
	fs::Metadata,
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, Local, Utc};
use notify::Event;
use prisma_client_rust::{raw, PrismaValue};
use sd_prisma::prisma_sync;
use sd_sync::OperationFactory;
use serde_json::json;
use tokio::{fs, io::ErrorKind};
use tracing::{debug, trace, warn};
use uuid::Uuid;

use super::INodeAndDevice;

pub(super) fn check_event(event: &Event, ignore_paths: &HashSet<PathBuf>) -> bool {
	// if path includes .DS_Store, .spacedrive file creation or is in the `ignore_paths` set, we ignore
	!event.paths.iter().any(|p| {
		p.file_name()
			.and_then(OsStr::to_str)
			.map_or(false, |name| name == ".DS_Store" || name == ".spacedrive")
			|| ignore_paths.contains(p)
	})
}

pub(super) async fn create_dir(
	location_id: location::id::Type,
	path: impl AsRef<Path>,
	metadata: &Metadata,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), LocationManagerError> {
	let location = find_location(library, location_id)
		.include(location_with_indexer_rules::include())
		.exec()
		.await?
		.ok_or(LocationManagerError::MissingLocation(location_id))?;

	let path = path.as_ref();

	let location_path = maybe_missing(&location.path, "location.path")?;

	trace!(
		"Location: <root_path ='{}'> creating directory: {}",
		location_path,
		path.display()
	);

	let iso_file_path = IsolatedFilePathData::new(location.id, location_path, path, true)?;

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

	let parent_iso_file_path = iso_file_path.parent();
	if !parent_iso_file_path.is_root()
		&& !check_file_path_exists::<FilePathError>(&parent_iso_file_path, &library.db).await?
	{
		warn!(
			"Watcher found a directory without parent: {}",
			&iso_file_path
		);
		return Ok(());
	};

	let children_materialized_path = iso_file_path
		.materialized_path_for_children()
		.expect("We're in the create dir function lol");

	debug!("Creating path: {}", iso_file_path);

	create_file_path(
		library,
		iso_file_path,
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

	// scan the new directory
	scan_location_sub_path(node, library, location, &children_materialized_path).await?;

	invalidate_query!(library, "search.paths");

	Ok(())
}

pub(super) async fn create_file(
	location_id: location::id::Type,
	path: impl AsRef<Path>,
	metadata: &Metadata,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), LocationManagerError> {
	inner_create_file(
		location_id,
		extract_location_path(location_id, library).await?,
		path,
		metadata,
		node,
		library,
	)
	.await
}

async fn inner_create_file(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
	metadata: &Metadata,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), LocationManagerError> {
	let path = path.as_ref();
	let location_path = location_path.as_ref();

	trace!(
		"Location: <root_path ='{}'> creating file: {}",
		location_path.display(),
		path.display()
	);

	let db = &library.db;

	let iso_file_path = IsolatedFilePathData::new(location_id, location_path, path, false)?;
	let extension = iso_file_path.extension.to_string();

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

	// First we check if already exist a file with these same inode and device numbers
	// if it does, we just update it
	if let Some(file_path) = db
		.file_path()
		.find_unique(file_path::location_id_inode_device(
			location_id,
			inode.to_le_bytes().to_vec(),
			device.to_le_bytes().to_vec(),
		))
		.include(file_path_with_object::include())
		.exec()
		.await?
	{
		trace!(
			"File already exists with that inode and device: {}",
			iso_file_path
		);
		return inner_update_file(location_path, &file_path, path, node, library, None).await;

	// If we can't find an existing file with the same inode and device, we check if there is a file with the same path
	} else if let Some(file_path) = db
		.file_path()
		.find_unique(file_path::location_id_materialized_path_name_extension(
			location_id,
			iso_file_path.materialized_path.to_string(),
			iso_file_path.name.to_string(),
			iso_file_path.extension.to_string(),
		))
		.include(file_path_with_object::include())
		.exec()
		.await?
	{
		trace!(
			"File already exists with that iso_file_path: {}",
			iso_file_path
		);
		return inner_update_file(
			location_path,
			&file_path,
			path,
			node,
			library,
			Some((inode, device)),
		)
		.await;
	}

	let parent_iso_file_path = iso_file_path.parent();
	if !parent_iso_file_path.is_root()
		&& !check_file_path_exists::<FilePathError>(&parent_iso_file_path, &library.db).await?
	{
		warn!("Watcher found a file without parent: {}", &iso_file_path);
		return Ok(());
	};

	// generate provisional object
	let FileMetadata {
		cas_id,
		kind,
		fs_metadata,
	} = FileMetadata::new(&location_path, &iso_file_path).await?;

	debug!("Creating path: {}", iso_file_path);

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

	object::select!(object_just_id { id });

	let existing_object = db
		.object()
		.find_first(vec![object::file_paths::some(vec![
			file_path::cas_id::equals(Some(cas_id.clone())),
			file_path::pub_id::not(created_file.pub_id.clone()),
		])])
		.select(object_just_id::select())
		.exec()
		.await?;

	let object = if let Some(object) = existing_object {
		object
	} else {
		db.object()
			.create(
				Uuid::new_v4().as_bytes().to_vec(),
				vec![
					object::date_created::set(Some(
						DateTime::<Local>::from(fs_metadata.created_or_now()).into(),
					)),
					object::kind::set(Some(kind as i32)),
				],
			)
			.select(object_just_id::select())
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

	if !extension.is_empty() {
		// Running in a detached task as thumbnail generation can take a while and we don't want to block the watcher
		let path = path.to_path_buf();
		let node = node.clone();

		tokio::spawn(async move {
			generate_thumbnail(&extension, &cas_id, path, &node).await;
		});
	}

	invalidate_query!(library, "search.paths");

	Ok(())
}

pub(super) async fn create_dir_or_file(
	location_id: location::id::Type,
	path: impl AsRef<Path>,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<Metadata, LocationManagerError> {
	let path = path.as_ref();
	let metadata = fs::metadata(path)
		.await
		.map_err(|e| FileIOError::from((path, e)))?;

	if metadata.is_dir() {
		create_dir(location_id, path, &metadata, node, library).await
	} else {
		create_file(location_id, path, &metadata, node, library).await
	}
	.map(|_| metadata)
}

pub(super) async fn update_file(
	location_id: location::id::Type,
	full_path: impl AsRef<Path>,
	node: &Arc<Node>,
	library: &Arc<Library>,
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
		inner_update_file(location_path, file_path, full_path, node, library, None).await
	} else {
		inner_create_file(
			location_id,
			location_path,
			full_path,
			&fs::metadata(full_path)
				.await
				.map_err(|e| FileIOError::from((full_path, e)))?,
			node,
			library,
		)
		.await
	}
	.map(|_| invalidate_query!(library, "search.paths"))
}

async fn inner_update_file(
	location_path: impl AsRef<Path>,
	file_path: &file_path_with_object::Data,
	full_path: impl AsRef<Path>,
	node: &Arc<Node>,
	library @ Library { db, sync, .. }: &Library,
	maybe_new_inode_and_device: Option<INodeAndDevice>,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = location_path.as_ref();

	let (current_inode, current_device) = (
		inode_from_db(&maybe_missing(file_path.inode.as_ref(), "file_path.inode")?[0..8]),
		device_from_db(&maybe_missing(file_path.device.as_ref(), "file_path.device")?[0..8]),
	);

	trace!(
		"Location: <root_path ='{}'> updating file: {}",
		location_path.display(),
		full_path.display()
	);

	let iso_file_path = IsolatedFilePathData::try_from(file_path)?;

	let FileMetadata {
		cas_id,
		fs_metadata,
		kind,
	} = FileMetadata::new(&location_path, &iso_file_path).await?;

	let (inode, device) = if let Some((inode, device)) = maybe_new_inode_and_device {
		(inode, device)
	} else {
		#[cfg(target_family = "unix")]
		{
			get_inode_and_device(&fs_metadata)?
		}

		#[cfg(target_family = "windows")]
		{
			// FIXME: This is a workaround for Windows, because we can't get the inode and device from the metadata
			get_inode_and_device_from_path(full_path).await?
		}
	};

	let (maybe_new_inode, maybe_new_device) =
		match (current_inode == inode, current_device == device) {
			(true, true) => (None, None),
			(true, false) => (None, Some(device)),
			(false, true) => (Some(inode), None),
			(false, false) => (Some(inode), Some(device)),
		};

	if let Some(old_cas_id) = &file_path.cas_id {
		if old_cas_id != &cas_id {
			let (sync_params, db_params): (Vec<_>, Vec<_>) = {
				use file_path::*;

				[
					(
						(cas_id::NAME, json!(old_cas_id)),
						Some(cas_id::set(Some(old_cas_id.clone()))),
					),
					(
						(
							size_in_bytes_bytes::NAME,
							json!(fs_metadata.len().to_be_bytes().to_vec()),
						),
						Some(size_in_bytes_bytes::set(Some(
							fs_metadata.len().to_be_bytes().to_vec(),
						))),
					),
					{
						let date = DateTime::<Utc>::from(fs_metadata.modified_or_now()).into();

						(
							(date_modified::NAME, json!(date)),
							Some(date_modified::set(Some(date))),
						)
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
							Some(integrity_checksum::set(checksum)),
						)
					},
					{
						if let Some(new_inode) = maybe_new_inode {
							(
								(inode::NAME, json!(new_inode)),
								Some(inode::set(Some(inode_to_db(new_inode)))),
							)
						} else {
							((inode::NAME, serde_json::Value::Null), None)
						}
					},
					{
						if let Some(new_device) = maybe_new_device {
							(
								(device::NAME, json!(new_device)),
								Some(device::set(Some(device_to_db(new_device)))),
							)
						} else {
							((device::NAME, serde_json::Value::Null), None)
						}
					},
				]
				.into_iter()
				.filter_map(|(sync_param, maybe_db_param)| {
					maybe_db_param.map(|db_param| (sync_param, db_param))
				})
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
								prisma_sync::file_path::SyncId {
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
				if library.thumbnail_exists(node, old_cas_id).await? {
					if let Some(ext) = &file_path.extension {
						generate_thumbnail(ext, &cas_id, full_path, node).await;

						// remove the old thumbnail as we're generating a new one
						let thumb_path = get_thumbnail_path(node, old_cas_id);
						fs::remove_file(&thumb_path)
							.await
							.map_err(|e| FileIOError::from((thumb_path, e)))?;
					}
				}

				let int_kind = kind as i32;

				if object.kind.map(|k| k != int_kind).unwrap_or_default() {
					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::object::SyncId {
								pub_id: object.pub_id.clone(),
							},
							object::kind::NAME,
							json!(int_kind),
						),
						db.object().update(
							object::id::equals(object.id),
							vec![object::kind::set(Some(int_kind))],
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
	location_id: location::id::Type,
	new_path: impl AsRef<Path>,
	old_path: impl AsRef<Path>,
	new_path_metadata: Metadata,
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
		&& !check_file_path_exists::<FilePathError>(
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
			location_id,
			&location_path,
			old_path,
		)?)
		.exec()
		.await?
	{
		let is_dir = maybe_missing(file_path.is_dir, "file_path.is_dir")?;

		let new = IsolatedFilePathData::new(location_id, &location_path, new_path, is_dir)?;

		// If the renamed path is a directory, we have to update every successor
		if is_dir {
			let old = IsolatedFilePathData::new(location_id, &location_path, old_path, is_dir)?;
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
					file_path::materialized_path::set(Some(new_path_materialized_str)),
					file_path::name::set(Some(new.name.to_string())),
					file_path::extension::set(Some(new.extension.to_string())),
					file_path::date_modified::set(Some(
						DateTime::<Utc>::from(new_path_metadata.modified_or_now()).into(),
					)),
				],
			)
			.exec()
			.await?;

		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

pub(super) async fn remove(
	location_id: location::id::Type,
	full_path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = extract_location_path(location_id, library).await?;

	// if it doesn't exist either way, then we don't care
	let Some(file_path) = library.db
		.file_path()
		.find_first(loose_find_existing_file_path_params(
		location_id, &location_path, full_path,
		)?)
		.exec()
		.await? else {
			return Ok(());
	};

	remove_by_file_path(location_id, full_path, &file_path, library).await
}

pub(super) async fn remove_by_file_path(
	location_id: location::id::Type,
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

			let is_dir = maybe_missing(file_path.is_dir, "file_path.is_dir")?;

			// if is doesn't, we can remove it safely from our db
			if is_dir {
				delete_directory(
					library,
					location_id,
					Some(&IsolatedFilePathData::try_from(file_path)?),
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
		}
		Err(e) => return Err(FileIOError::from((path, e)).into()),
	}

	invalidate_query!(library, "search.paths");

	Ok(())
}

pub(super) async fn extract_inode_and_device_from_path(
	location_id: location::id::Type,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<INodeAndDevice, LocationManagerError> {
	let path = path.as_ref();
	let location = find_location(library, location_id)
		.select(location::select!({ path }))
		.exec()
		.await?
		.ok_or(LocationManagerError::MissingLocation(location_id))?;

	let location_path = maybe_missing(&location.path, "location.path")?;

	library
		.db
		.file_path()
		.find_first(loose_find_existing_file_path_params(
			location_id,
			location_path,
			path,
		)?)
		.select(file_path::select!({ inode device }))
		.exec()
		.await?
		.map_or(
			Err(FilePathError::NotFound(path.into()).into()),
			|file_path| {
				Ok((
					inode_from_db(
						&maybe_missing(file_path.inode.as_ref(), "file_path.inode")?[0..8],
					),
					device_from_db(
						&maybe_missing(file_path.device.as_ref(), "file_path.device")?[0..8],
					),
				))
			},
		)
}

pub(super) async fn extract_location_path(
	location_id: location::id::Type,
	library: &Library,
) -> Result<PathBuf, LocationManagerError> {
	find_location(library, location_id)
		.select(location::select!({ path }))
		.exec()
		.await?
		.map_or(
			Err(LocationManagerError::MissingLocation(location_id)),
			// NOTE: The following usage of `PathBuf` doesn't incur a new allocation so it's fine
			|location| Ok(maybe_missing(location.path, "location.path")?.into()),
		)
}
