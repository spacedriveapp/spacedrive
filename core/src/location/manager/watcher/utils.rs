use crate::{
	invalidate_query,
	library::Library,
	location::{
		create_file_path, delete_directory, find_location,
		indexer::reverse_update_directories_sizes, location_with_indexer_rules,
		manager::LocationManagerError, scan_location_sub_path, update_location_size,
	},
	object::{
		media::{
			media_data_extractor::{can_extract_media_data_for_image, extract_media_data},
			media_data_image_to_query_params,
			old_thumbnail::get_indexed_thumbnail_path,
		},
		old_file_identifier::FileMetadata,
		validation::hash::file_checksum,
	},
	Node,
};

use sd_core_file_path_helper::{
	check_file_path_exists, filter_existing_file_path_params,
	isolated_file_path_data::extract_normalized_materialized_path_str,
	loose_find_existing_file_path_params, path_is_hidden, FilePathError, FilePathMetadata,
	IsolatedFilePathData, MetadataExt,
};
use sd_core_prisma_helpers::file_path_with_object;

use sd_file_ext::{extensions::ImageExtension, kind::ObjectKind};
use sd_prisma::{
	prisma::{file_path, location, media_data, object},
	prisma_sync,
};
use sd_sync::OperationFactory;
use sd_utils::{
	db::{inode_from_db, inode_to_db, maybe_missing},
	error::FileIOError,
	msgpack, uuid_to_bytes,
};

#[cfg(target_family = "unix")]
use sd_core_file_path_helper::get_inode;

#[cfg(target_family = "windows")]
use sd_core_file_path_helper::get_inode_from_path;

use std::{
	collections::{HashMap, HashSet},
	ffi::OsStr,
	fs::Metadata,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use chrono::{DateTime, FixedOffset, Local, Utc};
use notify::Event;
use tokio::{
	fs,
	io::{self, ErrorKind},
	spawn,
	time::Instant,
};
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

use super::{INode, HUNDRED_MILLIS};

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
		iso_file_path.to_parts(),
		None,
		FilePathMetadata::from_path(path, metadata)?,
	)
	.await?;

	// scan the new directory
	scan_location_sub_path(node, library, location, &children_materialized_path).await?;

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

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
	library @ Library {
		id: library_id,
		db,
		sync,
		..
	}: &Library,
) -> Result<(), LocationManagerError> {
	let path = path.as_ref();
	let location_path = location_path.as_ref();

	trace!(
		"Location: <root_path ='{}'> creating file: {}",
		location_path.display(),
		path.display()
	);

	let iso_file_path = IsolatedFilePathData::new(location_id, location_path, path, false)?;
	let iso_file_path_parts = iso_file_path.to_parts();
	let extension = iso_file_path_parts.extension.to_string();

	let metadata = FilePathMetadata::from_path(path, metadata)?;

	// First we check if already exist a file with this same inode number
	// if it does, we just update it
	if let Some(file_path) = db
		.file_path()
		.find_unique(file_path::location_id_inode(
			location_id,
			inode_to_db(metadata.inode),
		))
		.include(file_path_with_object::include())
		.exec()
		.await?
	{
		trace!("File already exists with that inode: {}", iso_file_path);
		return inner_update_file(location_path, &file_path, path, node, library, None).await;

	// If we can't find an existing file with the same inode, we check if there is a file with the same path
	} else if let Some(file_path) = db
		.file_path()
		.find_unique(file_path::location_id_materialized_path_name_extension(
			location_id,
			iso_file_path_parts.materialized_path.to_string(),
			iso_file_path_parts.name.to_string(),
			iso_file_path_parts.extension.to_string(),
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
			Some(metadata.inode),
		)
		.await;
	}

	let parent_iso_file_path = iso_file_path.parent();
	if !parent_iso_file_path.is_root()
		&& !check_file_path_exists::<FilePathError>(&parent_iso_file_path, db).await?
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

	let created_file =
		create_file_path(library, iso_file_path_parts, cas_id.clone(), metadata).await?;

	object::select!(object_ids { id pub_id });

	let existing_object = db
		.object()
		.find_first(vec![object::file_paths::some(vec![
			file_path::cas_id::equals(cas_id.clone()),
			file_path::pub_id::not(created_file.pub_id.clone()),
		])])
		.select(object_ids::select())
		.exec()
		.await?;

	let object_ids::Data {
		id: object_id,
		pub_id: object_pub_id,
	} = if let Some(object) = existing_object {
		object
	} else {
		let pub_id = uuid_to_bytes(Uuid::new_v4());
		let date_created: DateTime<FixedOffset> =
			DateTime::<Local>::from(fs_metadata.created_or_now()).into();
		let int_kind = kind as i32;
		sync.write_ops(
			db,
			(
				sync.shared_create(
					prisma_sync::object::SyncId {
						pub_id: pub_id.clone(),
					},
					[
						(object::date_created::NAME, msgpack!(date_created)),
						(object::kind::NAME, msgpack!(int_kind)),
					],
				),
				db.object()
					.create(
						pub_id.to_vec(),
						vec![
							object::date_created::set(Some(date_created)),
							object::kind::set(Some(int_kind)),
						],
					)
					.select(object_ids::select()),
			),
		)
		.await?
	};

	sync.write_op(
		db,
		sync.shared_update(
			prisma_sync::location::SyncId {
				pub_id: created_file.pub_id.clone(),
			},
			file_path::object::NAME,
			msgpack!(prisma_sync::object::SyncId {
				pub_id: object_pub_id.clone()
			}),
		),
		db.file_path().update(
			file_path::pub_id::equals(created_file.pub_id.clone()),
			vec![file_path::object::connect(object::pub_id::equals(
				object_pub_id.clone(),
			))],
		),
	)
	.await?;

	if !extension.is_empty() && matches!(kind, ObjectKind::Image | ObjectKind::Video) {
		// Running in a detached task as thumbnail generation can take a while and we don't want to block the watcher

		if let Some(cas_id) = cas_id {
			spawn({
				let extension = extension.clone();
				let path = path.to_path_buf();
				let node = node.clone();
				let library_id = *library_id;

				async move {
					if let Err(e) = node
						.thumbnailer
						.generate_single_indexed_thumbnail(&extension, cas_id, path, library_id)
						.await
					{
						error!("Failed to generate thumbnail in the watcher: {e:#?}");
					}
				}
			});
		}

		// TODO: Currently we only extract media data for images, remove this if later
		if matches!(kind, ObjectKind::Image) {
			if let Ok(image_extension) = ImageExtension::from_str(&extension) {
				if can_extract_media_data_for_image(&image_extension) {
					if let Ok(media_data) = extract_media_data(path)
						.await
						.map_err(|e| error!("Failed to extract media data: {e:#?}"))
					{
						let (sync_params, db_params) = media_data_image_to_query_params(media_data);

						sync.write_ops(
							db,
							(
								sync.shared_create(
									prisma_sync::media_data::SyncId {
										object: prisma_sync::object::SyncId {
											pub_id: object_pub_id.clone(),
										},
									},
									sync_params,
								),
								db.media_data().upsert(
									media_data::object_id::equals(object_id),
									media_data::create(
										object::id::equals(object_id),
										db_params.clone(),
									),
									db_params,
								),
							),
						)
						.await?;
					}
				}
			}
		}
	}

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

	Ok(())
}

pub(super) async fn update_file(
	location_id: location::id::Type,
	full_path: impl AsRef<Path>,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();

	let metadata = match fs::metadata(full_path).await {
		Ok(metadata) => metadata,
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			// If the file doesn't exist anymore, it was just a temporary file
			return Ok(());
		}
		Err(e) => return Err(FileIOError::from((full_path, e)).into()),
	};

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
			&metadata,
			node,
			library,
		)
		.await
	}
	.map(|_| {
		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	})
}

async fn inner_update_file(
	location_path: impl AsRef<Path>,
	file_path: &file_path_with_object::Data,
	full_path: impl AsRef<Path>,
	node: &Arc<Node>,
	library @ Library { db, sync, .. }: &Library,
	maybe_new_inode: Option<INode>,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = location_path.as_ref();

	let current_inode =
		inode_from_db(&maybe_missing(file_path.inode.as_ref(), "file_path.inode")?[0..8]);

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

	let inode = if let Some(inode) = maybe_new_inode {
		inode
	} else {
		#[cfg(target_family = "unix")]
		{
			get_inode(&fs_metadata)
		}

		#[cfg(target_family = "windows")]
		{
			// FIXME: This is a workaround for Windows, because we can't get the inode from the metadata
			get_inode_from_path(full_path).await?
		}
	};

	let is_hidden = path_is_hidden(full_path, &fs_metadata);
	if file_path.cas_id != cas_id {
		let (sync_params, db_params): (Vec<_>, Vec<_>) = {
			use file_path::*;

			[
				(
					(cas_id::NAME, msgpack!(file_path.cas_id)),
					Some(cas_id::set(file_path.cas_id.clone())),
				),
				(
					(
						size_in_bytes_bytes::NAME,
						msgpack!(fs_metadata.len().to_be_bytes().to_vec()),
					),
					Some(size_in_bytes_bytes::set(Some(
						fs_metadata.len().to_be_bytes().to_vec(),
					))),
				),
				{
					let date = DateTime::<Utc>::from(fs_metadata.modified_or_now()).into();

					(
						(date_modified::NAME, msgpack!(date)),
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
						(integrity_checksum::NAME, msgpack!(checksum)),
						Some(integrity_checksum::set(checksum)),
					)
				},
				{
					if current_inode != inode {
						(
							(inode::NAME, msgpack!(inode)),
							Some(inode::set(Some(inode_to_db(inode)))),
						)
					} else {
						((inode::NAME, msgpack!(nil)), None)
					}
				},
				{
					if is_hidden != file_path.hidden.unwrap_or_default() {
						(
							(hidden::NAME, msgpack!(inode)),
							Some(hidden::set(Some(is_hidden))),
						)
					} else {
						((hidden::NAME, msgpack!(nil)), None)
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
			let int_kind = kind as i32;

			if db
				.file_path()
				.count(vec![file_path::object_id::equals(Some(object.id))])
				.exec()
				.await? == 1
			{
				if object.kind.map(|k| k != int_kind).unwrap_or_default() {
					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::object::SyncId {
								pub_id: object.pub_id.clone(),
							},
							object::kind::NAME,
							msgpack!(int_kind),
						),
						db.object().update(
							object::id::equals(object.id),
							vec![object::kind::set(Some(int_kind))],
						),
					)
					.await?;
				}
			} else {
				let pub_id = uuid_to_bytes(Uuid::new_v4());
				let date_created: DateTime<FixedOffset> =
					DateTime::<Local>::from(fs_metadata.created_or_now()).into();

				sync.write_ops(
					db,
					(
						sync.shared_create(
							prisma_sync::object::SyncId {
								pub_id: pub_id.clone(),
							},
							[
								(object::date_created::NAME, msgpack!(date_created)),
								(object::kind::NAME, msgpack!(int_kind)),
							],
						),
						db.object().create(
							pub_id.to_vec(),
							vec![
								object::date_created::set(Some(date_created)),
								object::kind::set(Some(int_kind)),
							],
						),
					),
				)
				.await?;

				sync.write_op(
					db,
					sync.shared_update(
						prisma_sync::location::SyncId {
							pub_id: file_path.pub_id.clone(),
						},
						file_path::object::NAME,
						msgpack!(prisma_sync::object::SyncId {
							pub_id: pub_id.clone()
						}),
					),
					db.file_path().update(
						file_path::pub_id::equals(file_path.pub_id.clone()),
						vec![file_path::object::connect(object::pub_id::equals(pub_id))],
					),
				)
				.await?;
			}

			if let Some(old_cas_id) = &file_path.cas_id {
				// if this file had a thumbnail previously, we update it to match the new content
				if library.thumbnail_exists(node, old_cas_id).await? {
					if let Some(ext) = file_path.extension.clone() {
						// Running in a detached task as thumbnail generation can take a while and we don't want to block the watcher
						if let Some(cas_id) = cas_id {
							let node = Arc::clone(node);
							let path = full_path.to_path_buf();
							let library_id = library.id;
							let old_cas_id = old_cas_id.clone();
							spawn(async move {
								let was_overwritten = old_cas_id == cas_id;
								if let Err(e) = node
									.thumbnailer
									.generate_single_indexed_thumbnail(
										&ext, cas_id, path, library_id,
									)
									.await
								{
									error!("Failed to generate thumbnail in the watcher: {e:#?}");
								}

								// If only a few bytes changed, cas_id will probably remains intact
								// so we overwrote our previous thumbnail, so we can't remove it
								if !was_overwritten {
									// remove the old thumbnail as we're generating a new one
									let thumb_path =
										get_indexed_thumbnail_path(&node, &old_cas_id, library_id);
									if let Err(e) = fs::remove_file(&thumb_path).await {
										error!(
											"Failed to remove old thumbnail: {:#?}",
											FileIOError::from((thumb_path, e))
										);
									}
								}
							});
						}
					}
				}
			}

			// TODO: Change this if to include ObjectKind::Video in the future
			if let Some(ext) = &file_path.extension {
				if let Ok(image_extension) = ImageExtension::from_str(ext) {
					if can_extract_media_data_for_image(&image_extension)
						&& matches!(kind, ObjectKind::Image)
					{
						if let Ok(media_data) = extract_media_data(full_path)
							.await
							.map_err(|e| error!("Failed to extract media data: {e:#?}"))
						{
							let (sync_params, db_params) =
								media_data_image_to_query_params(media_data);

							sync.write_ops(
								db,
								(
									sync.shared_create(
										prisma_sync::media_data::SyncId {
											object: prisma_sync::object::SyncId {
												pub_id: object.pub_id.clone(),
											},
										},
										sync_params,
									),
									db.media_data().upsert(
										media_data::object_id::equals(object.id),
										media_data::create(
											object::id::equals(object.id),
											db_params.clone(),
										),
										db_params,
									),
								),
							)
							.await?;
						}
					}
				}
			}
		}

		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	} else if is_hidden != file_path.hidden.unwrap_or_default() {
		sync.write_ops(
			db,
			(
				vec![sync.shared_update(
					prisma_sync::file_path::SyncId {
						pub_id: file_path.pub_id.clone(),
					},
					file_path::hidden::NAME,
					msgpack!(is_hidden),
				)],
				db.file_path().update(
					file_path::pub_id::equals(file_path.pub_id.clone()),
					vec![file_path::hidden::set(Some(is_hidden))],
				),
			),
		)
		.await?;

		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

fn rename_starts_with(path1: &str, path2: &str) -> String {
	if cfg!(target_os = "ios") {
		format!("{}{}/", path1, path2)
	} else {
		format!("{}/{}/", path1, path2) // Somehow this works, when it shouldn't. I'm not sure why, but I'm not going to question it.
	}
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
	let Library { db, sync, .. } = library;

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

	debug!(
		"Location: <root_path ='{}'> renaming file: {} to {}",
		location_path.display(),
		old_path.display(),
		new_path.display()
	);

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
		debug!("Is dir: {}", is_dir);

		let new = IsolatedFilePathData::new(location_id, &location_path, new_path, is_dir)?;
		let new_parts = new.to_parts();

		// If the renamed path is a directory, we have to update every successor
		if is_dir {
			let old = IsolatedFilePathData::new(location_id, &location_path, old_path, is_dir)?;
			let old_parts = old.to_parts();

			debug!("Old: {:#?}", old_parts);

			let starts_with = rename_starts_with(old_parts.materialized_path, old_parts.name);

			debug!("Starts with: {}", starts_with);
			let paths = db
				.file_path()
				.find_many(vec![
					file_path::location_id::equals(Some(location_id)),
					file_path::materialized_path::starts_with(starts_with.clone()),
				])
				.select(file_path::select!({
					id
					pub_id
					materialized_path
				}))
				.exec()
				.await?;

			debug!("Paths: {:#?}", paths);
			debug!("Updating {len} file_paths", len = paths.len());

			let len = paths.len();
			let (sync_params, db_params): (Vec<_>, Vec<_>) = paths
				.into_iter()
				.filter_map(|path| path.materialized_path.map(|mp| (path.id, path.pub_id, mp)))
				.map(|(id, pub_id, mp)| {
					let new_path = mp.replace(
						&starts_with,
						&rename_starts_with(new_parts.materialized_path, new_parts.name),
					);
					debug!("New path: {}", new_path);

					(
						sync.shared_update(
							sd_prisma::prisma_sync::file_path::SyncId { pub_id },
							file_path::materialized_path::NAME,
							msgpack!(&new_path),
						),
						db.file_path().update(
							file_path::id::equals(id),
							vec![file_path::materialized_path::set(Some(new_path))],
						),
					)
				})
				.unzip();

			sync.write_ops(db, (sync_params, db_params)).await?;

			debug!("Updated {len} file_paths");
		}

		let is_hidden = path_is_hidden(new_path, &new_path_metadata);

		let date_modified = DateTime::<Utc>::from(new_path_metadata.modified_or_now()).into();

		let (sync_params, db_params): (Vec<_>, Vec<_>) = [
			(
				(
					file_path::materialized_path::NAME,
					msgpack!(new_path_materialized_str),
				),
				file_path::materialized_path::set(Some(new_path_materialized_str)),
			),
			(
				(file_path::name::NAME, msgpack!(new_parts.name)),
				file_path::name::set(Some(new_parts.name.to_string())),
			),
			(
				(file_path::extension::NAME, msgpack!(new_parts.extension)),
				file_path::extension::set(Some(new_parts.extension.to_string())),
			),
			(
				(file_path::date_modified::NAME, msgpack!(&date_modified)),
				file_path::date_modified::set(Some(date_modified)),
			),
			(
				(file_path::hidden::NAME, msgpack!(is_hidden)),
				file_path::hidden::set(Some(is_hidden)),
			),
		]
		.into_iter()
		.unzip();

		sync.write_ops(
			db,
			(
				sync_params
					.into_iter()
					.map(|(k, v)| {
						sync.shared_update(
							prisma_sync::file_path::SyncId {
								pub_id: file_path.pub_id.clone(),
							},
							k,
							v,
						)
					})
					.collect(),
				db.file_path()
					.update(file_path::pub_id::equals(file_path.pub_id), db_params),
			),
		)
		.await?;

		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
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
	let Some(file_path) = library
		.db
		.file_path()
		.find_first(loose_find_existing_file_path_params(
			location_id,
			&location_path,
			full_path,
		)?)
		.exec()
		.await?
	else {
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
			let Library { sync, db, .. } = library;

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
				sync.write_op(
					db,
					sync.shared_delete(prisma_sync::file_path::SyncId {
						pub_id: file_path.pub_id.clone(),
					}),
					db.file_path().delete(file_path::id::equals(file_path.id)),
				)
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
	invalidate_query!(library, "search.objects");

	Ok(())
}

pub(super) async fn extract_inode_from_path(
	location_id: location::id::Type,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<INode, LocationManagerError> {
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
		.select(file_path::select!({ inode }))
		.exec()
		.await?
		.map_or(
			Err(FilePathError::NotFound(path.into()).into()),
			|file_path| {
				Ok(inode_from_db(
					&maybe_missing(file_path.inode.as_ref(), "file_path.inode")?[0..8],
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

pub(super) async fn recalculate_directories_size(
	candidates: &mut HashMap<PathBuf, Instant>,
	buffer: &mut Vec<(PathBuf, Instant)>,
	location_id: location::id::Type,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let mut location_path_cache = None;
	let mut should_invalidate = false;
	let mut should_update_location_size = false;
	buffer.clear();

	for (path, instant) in candidates.drain() {
		if instant.elapsed() > HUNDRED_MILLIS * 5 {
			if location_path_cache.is_none() {
				location_path_cache = Some(PathBuf::from(maybe_missing(
					find_location(library, location_id)
						.select(location::select!({ path }))
						.exec()
						.await?
						.ok_or(LocationManagerError::MissingLocation(location_id))?
						.path,
					"location.path",
				)?))
			}

			if let Some(location_path) = &location_path_cache {
				if path != *location_path {
					trace!(
						"Reverse calculating directory sizes starting at {} until {}",
						path.display(),
						location_path.display(),
					);
					reverse_update_directories_sizes(path, location_id, location_path, library)
						.await?;
					should_invalidate = true;
				} else {
					should_update_location_size = true;
				}
			}
		} else {
			buffer.push((path, instant));
		}
	}

	if should_update_location_size {
		update_location_size(location_id, library).await?;
	}

	if should_invalidate {
		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	}

	candidates.extend(buffer.drain(..));

	Ok(())
}
