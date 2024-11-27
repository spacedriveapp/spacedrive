use crate::{
	invalidate_query,
	library::Library,
	location::{
		create_file_path, delete_directory, find_location,
		indexer::reverse_update_directories_sizes, location_with_indexer_rules,
		manager::LocationManagerError, scan_location_sub_path, update_location_size,
	},
	object::validation::hash::file_checksum,
	Node,
};

use sd_core_file_path_helper::{
	check_file_path_exists, filter_existing_file_path_params,
	isolated_file_path_data::extract_normalized_materialized_path_str,
	loose_find_existing_file_path_params, path_is_hidden, FilePathError, FilePathMetadata,
	IsolatedFilePathData, MetadataExt,
};
use sd_core_heavy_lifting::{
	file_identifier::FileMetadata,
	media_processor::{
		exif_media_data, ffmpeg_media_data, generate_single_thumbnail, get_thumbnails_directory,
		ThumbnailKind,
	},
};
use sd_core_indexer_rules::{
	seed::{GitIgnoreRules, GITIGNORE},
	IndexerRuler, RulerDecision,
};
use sd_core_prisma_helpers::{
	file_path_watcher_remove, file_path_with_object, object_ids, CasId, ObjectPubId,
};

use sd_file_ext::{
	extensions::{AudioExtension, ImageExtension, VideoExtension},
	kind::ObjectKind,
};
use sd_prisma::{
	prisma::{device, file_path, location, object},
	prisma_sync,
};
use sd_sync::{option_sync_db_entry, sync_db_entry, sync_entry, OperationFactory};
use sd_utils::{
	chain_optional_iter,
	db::{inode_from_db, inode_to_db, maybe_missing, size_in_bytes_to_db},
	error::FileIOError,
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
use futures_concurrency::future::Join;
use notify::Event;
use tokio::{
	fs,
	io::{self, ErrorKind},
	spawn,
	time::{sleep, Instant},
};
use tracing::{error, instrument, trace, warn};

use super::{INode, HUNDRED_MILLIS, ONE_SECOND};

pub(super) async fn reject_event(
	event: &Event,
	ignore_paths: &HashSet<PathBuf>,
	location_path: Option<&Path>,
	indexer_ruler: Option<&IndexerRuler>,
) -> bool {
	// if path includes .DS_Store, .spacedrive file creation or is in the `ignore_paths` set, we ignore
	if event.paths.iter().any(|p| {
		p.file_name()
			.and_then(OsStr::to_str)
			.map_or(false, |name| name == ".DS_Store" || name == ".spacedrive")
			|| ignore_paths.contains(p)
	}) {
		trace!("Rejected by ignored paths");
		return true;
	}

	if let Some(indexer_ruler) = indexer_ruler {
		let ruler_decisions = event
			.paths
			.iter()
			.map(|path| async move { (path, fs::metadata(path).await) })
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.filter_map(|(path, res)| {
				res.map(|metadata| (path, metadata))
					.map_err(|e| {
						if e.kind() != ErrorKind::NotFound {
							error!(?e, path = %path.display(), "Failed to get metadata for path;");
						}
					})
					.ok()
			})
			.map(|(path, metadata)| {
				let mut independent_ruler = indexer_ruler.clone();

				async move {
					let path_to_check_gitignore = if metadata.is_dir() {
						Some(path.as_path())
					} else {
						path.parent()
					};

					if let (Some(path_to_check_gitignore), Some(location_path)) =
						(path_to_check_gitignore, location_path.as_ref())
					{
						if independent_ruler.has_system(&GITIGNORE) {
							if let Some(rules) = GitIgnoreRules::get_rules_if_in_git_repo(
								location_path,
								path_to_check_gitignore,
							)
							.await
							{
								trace!("Found gitignore rules to follow");
								independent_ruler.extend(rules.map(Into::into));
							}
						}
					}

					independent_ruler.evaluate_path(path, &metadata).await
				}
			})
			.collect::<Vec<_>>()
			.join()
			.await;

		if !ruler_decisions.is_empty()
			&& ruler_decisions.into_iter().all(|res| {
				matches!(
					res.map_err(|e| trace!(?e, "Failed to evaluate path;"))
						// In case of error, we accept the path as a safe default
						.unwrap_or(RulerDecision::Accept),
					RulerDecision::Reject
				)
			}) {
			trace!("Rejected by indexer ruler");
			return true;
		}
	}

	false
}

#[instrument(skip_all, fields(path = %path.as_ref().display()), err)]
pub(super) async fn create_dir(
	location_id: location::id::Type,
	path: impl AsRef<Path> + Send,
	metadata: &Metadata,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), LocationManagerError> {
	let location = find_location(library, location_id)
		.include(location_with_indexer_rules::include())
		.exec()
		.await?
		.ok_or(LocationManagerError::LocationNotFound(location_id))?;

	let path = path.as_ref();

	let location_path = maybe_missing(&location.path, "location.path")?;

	trace!(new_directory = %path.display(), "Creating directory;");

	let iso_file_path = IsolatedFilePathData::new(location.id, location_path, path, true)?;

	let parent_iso_file_path = iso_file_path.parent();
	if !parent_iso_file_path.is_root()
		&& !check_file_path_exists::<FilePathError>(&parent_iso_file_path, &library.db).await?
	{
		warn!(%iso_file_path, "Watcher found a directory without parent;");

		return Ok(());
	};

	let children_materialized_path = iso_file_path
		.materialized_path_for_children()
		.expect("We're in the create dir function lol");

	create_file_path(
		library,
		iso_file_path.to_parts(),
		None,
		FilePathMetadata::from_path(path, metadata)?,
	)
	.await?;

	spawn({
		let node = Arc::clone(node);
		let library = Arc::clone(library);

		async move {
			// Wait a bit for any files being moved into the new directory to be indexed by the watcher
			sleep(ONE_SECOND).await;

			trace!(%iso_file_path, "Scanning new directory;");

			// scan the new directory
			if let Err(e) =
				scan_location_sub_path(&node, &library, location, &children_materialized_path).await
			{
				error!(?e, "Failed to scan new directory;");
			}
		}
	});

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

	Ok(())
}

#[instrument(skip_all, fields(path = %path.as_ref().display()), err)]
pub(super) async fn create_file(
	location_id: location::id::Type,
	path: impl AsRef<Path> + Send,
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
	location_path: impl AsRef<Path> + Send,
	path: impl AsRef<Path> + Send,
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

	trace!(new_file = %path.display(), "Creating file;");

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
		trace!(%iso_file_path, "File already exists with that inode;");

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
		trace!(%iso_file_path, "File already exists with that iso_file_path;");

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
		warn!(%iso_file_path, "Watcher found a file without parent;");

		return Ok(());
	};

	// generate provisional object
	let FileMetadata {
		cas_id,
		kind,
		fs_metadata,
	} = FileMetadata::new(&location_path, &iso_file_path).await?;

	let created_file =
		create_file_path(library, iso_file_path_parts, cas_id.clone(), metadata).await?;

	let existing_object = db
		.object()
		.find_first(vec![object::file_paths::some(vec![
			file_path::cas_id::equals(cas_id.clone().map(Into::into)),
			file_path::pub_id::not(created_file.pub_id.clone()),
		])])
		.select(object_ids::select())
		.exec()
		.await?;

	let is_new_file = existing_object.is_none();

	let object_ids::Data {
		id: object_id,
		pub_id: object_pub_id,
	} = if let Some(object) = existing_object {
		object
	} else {
		let pub_id: ObjectPubId = ObjectPubId::new();
		let date_created: DateTime<FixedOffset> =
			DateTime::<Local>::from(fs_metadata.created_or_now()).into();
		let int_kind = kind as i32;

		let device_pub_id = sync.device_pub_id.to_db();

		let (sync_params, db_params) = [
			sync_db_entry!(date_created, object::date_created),
			sync_db_entry!(int_kind, object::kind),
			(
				sync_entry!(
					prisma_sync::device::SyncId {
						pub_id: device_pub_id.clone()
					},
					object::device
				),
				object::device::connect(device::pub_id::equals(device_pub_id)),
			),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		sync.write_op(
			db,
			sync.shared_create(
				prisma_sync::object::SyncId {
					pub_id: pub_id.to_db(),
				},
				sync_params,
			),
			db.object()
				.create(pub_id.into(), db_params)
				.select(object_ids::select()),
		)
		.await?
	};

	sync.write_op(
		db,
		sync.shared_update(
			prisma_sync::location::SyncId {
				pub_id: created_file.pub_id.clone(),
			},
			[sync_entry!(
				prisma_sync::object::SyncId {
					pub_id: object_pub_id.clone()
				},
				file_path::object
			)],
		),
		db.file_path()
			.update(
				file_path::pub_id::equals(created_file.pub_id.clone()),
				vec![file_path::object::connect(object::pub_id::equals(
					object_pub_id.clone(),
				))],
			)
			.select(file_path::select!({ id })),
	)
	.await?;

	// If the file is a duplicate of an existing file, we don't need to generate thumbnails nor extract media data
	if is_new_file
		&& !extension.is_empty()
		&& matches!(
			kind,
			ObjectKind::Image | ObjectKind::Video | ObjectKind::Audio
		) {
		// Running in a detached task as thumbnail generation can take a while and we don't want to block the watcher
		if matches!(kind, ObjectKind::Image | ObjectKind::Video) {
			if let Some(cas_id) = cas_id {
				spawn({
					let extension = extension.clone();
					let path = path.to_path_buf();
					let thumbnails_directory =
						get_thumbnails_directory(node.config.data_directory());
					let library_id = *library_id;

					async move {
						if let Err(e) = generate_single_thumbnail(
							&thumbnails_directory,
							extension,
							cas_id,
							path,
							ThumbnailKind::Indexed(library_id),
						)
						.await
						{
							error!(?e, "Failed to generate thumbnail in the watcher;");
						}
					}
				});
			}
		}

		match kind {
			ObjectKind::Image => {
				if let Ok(image_extension) = ImageExtension::from_str(&extension) {
					if exif_media_data::can_extract(image_extension) {
						if let Ok(Some(exif_data)) = exif_media_data::extract(path)
							.await
							.map_err(|e| error!(?e, "Failed to extract image media data;"))
						{
							exif_media_data::save(
								[(exif_data, object_id, object_pub_id.into())],
								db,
								sync,
							)
							.await?;
						}
					}
				}
			}

			ObjectKind::Audio => {
				if let Ok(audio_extension) = AudioExtension::from_str(&extension) {
					if ffmpeg_media_data::can_extract_for_audio(audio_extension) {
						if let Ok(ffmpeg_data) = ffmpeg_media_data::extract(path)
							.await
							.map_err(|e| error!(?e, "Failed to extract audio media data;"))
						{
							ffmpeg_media_data::save([(ffmpeg_data, object_id)], db).await?;
						}
					}
				}
			}

			ObjectKind::Video => {
				if let Ok(video_extension) = VideoExtension::from_str(&extension) {
					if ffmpeg_media_data::can_extract_for_video(video_extension) {
						if let Ok(ffmpeg_data) = ffmpeg_media_data::extract(path)
							.await
							.map_err(|e| error!(?e, "Failed to extract video media data;"))
						{
							ffmpeg_media_data::save([(ffmpeg_data, object_id)], db).await?;
						}
					}
				}
			}

			_ => {
				// Do nothing
			}
		}
	}

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

	Ok(())
}

#[instrument(skip_all, fields(path = %path.as_ref().display()), err)]
pub(super) async fn update_file(
	location_id: location::id::Type,
	path: impl AsRef<Path> + Send,
	node: &Arc<Node>,
	library: &Arc<Library>,
) -> Result<(), LocationManagerError> {
	let full_path = path.as_ref();

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
	.map(|()| {
		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	})
}

async fn inner_update_file(
	location_path: impl AsRef<Path> + Send,
	file_path: &file_path_with_object::Data,
	full_path: impl AsRef<Path> + Send,
	node: &Arc<Node>,
	library @ Library { db, sync, .. }: &Library,
	maybe_new_inode: Option<INode>,
) -> Result<(), LocationManagerError> {
	let full_path = full_path.as_ref();
	let location_path = location_path.as_ref();

	let current_inode =
		inode_from_db(&maybe_missing(file_path.inode.as_ref(), "file_path.inode")?[0..8]);

	trace!(
		location_path = %location_path.display(),
		path = %full_path.display(),
		"Updating file;",
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
	if file_path.cas_id.as_deref() != cas_id.as_ref().map(CasId::as_str) {
		let (sync_params, db_params) = chain_optional_iter(
			[
				sync_db_entry!(
					size_in_bytes_to_db(fs_metadata.len()),
					file_path::size_in_bytes_bytes
				),
				sync_db_entry!(
					DateTime::<Utc>::from(fs_metadata.modified_or_now()),
					file_path::date_modified
				),
			],
			[
				option_sync_db_entry!(file_path.cas_id.clone(), file_path::cas_id),
				option_sync_db_entry!(
					if file_path.integrity_checksum.is_some() {
						// TODO: Should this be a skip rather than a null-set?
						// If a checksum was already computed, we need to recompute it
						Some(
							file_checksum(full_path)
								.await
								.map_err(|e| FileIOError::from((full_path, e)))?,
						)
					} else {
						None
					},
					file_path::integrity_checksum
				),
				option_sync_db_entry!(
					(current_inode != inode).then(|| inode_to_db(inode)),
					file_path::inode
				),
				option_sync_db_entry!(
					(is_hidden != file_path.hidden.unwrap_or_default()).then_some(is_hidden),
					file_path::hidden
				),
			],
		)
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		// file content changed
		sync.write_op(
			db,
			sync.shared_update(
				prisma_sync::file_path::SyncId {
					pub_id: file_path.pub_id.clone(),
				},
				sync_params,
			),
			db.file_path()
				.update(
					file_path::pub_id::equals(file_path.pub_id.clone()),
					db_params,
				)
				.select(file_path::select!({ id })),
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
					let (sync_param, db_param) = sync_db_entry!(int_kind, object::kind);
					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::object::SyncId {
								pub_id: object.pub_id.clone(),
							},
							[sync_param],
						),
						db.object()
							.update(object::id::equals(object.id), vec![db_param])
							.select(object::select!({ id })),
					)
					.await?;
				}
			} else {
				let pub_id = ObjectPubId::new();
				let date_created: DateTime<FixedOffset> =
					DateTime::<Local>::from(fs_metadata.created_or_now()).into();

				let device_pub_id = sync.device_pub_id.to_db();

				let (sync_params, db_params) = [
					sync_db_entry!(date_created, object::date_created),
					sync_db_entry!(int_kind, object::kind),
					(
						sync_entry!(
							prisma_sync::device::SyncId {
								pub_id: device_pub_id.clone()
							},
							object::device
						),
						object::device::connect(device::pub_id::equals(device_pub_id)),
					),
				]
				.into_iter()
				.unzip::<_, _, Vec<_>, Vec<_>>();

				sync.write_op(
					db,
					sync.shared_create(
						prisma_sync::object::SyncId {
							pub_id: pub_id.to_db(),
						},
						sync_params,
					),
					db.object().create(pub_id.to_db(), db_params),
				)
				.await?;

				sync.write_op(
					db,
					sync.shared_update(
						prisma_sync::location::SyncId {
							pub_id: file_path.pub_id.clone(),
						},
						[sync_entry!(
							prisma_sync::object::SyncId {
								pub_id: pub_id.to_db()
							},
							file_path::object
						)],
					),
					db.file_path()
						.update(
							file_path::pub_id::equals(file_path.pub_id.clone()),
							vec![file_path::object::connect(object::pub_id::equals(
								pub_id.into(),
							))],
						)
						.select(file_path::select!({ id })),
				)
				.await?;
			}

			if let Some(old_cas_id) = file_path.cas_id.as_ref().map(CasId::from) {
				// if this file had a thumbnail previously, we update it to match the new content
				if library.thumbnail_exists(node, &old_cas_id).await? {
					if let Some(ext) = file_path.extension.clone() {
						// Running in a detached task as thumbnail generation can take a while and we don't want to block the watcher
						if let Some(cas_id) = cas_id {
							let node = Arc::clone(node);
							let path = full_path.to_path_buf();
							let library_id = library.id;
							let old_cas_id = old_cas_id.to_owned();

							spawn(async move {
								let thumbnails_directory =
									get_thumbnails_directory(node.config.data_directory());

								let was_overwritten = old_cas_id == cas_id;
								if let Err(e) = generate_single_thumbnail(
									&thumbnails_directory,
									ext.clone(),
									cas_id,
									path,
									ThumbnailKind::Indexed(library_id),
								)
								.await
								{
									error!(?e, "Failed to generate thumbnail in the watcher;");
								}

								// If only a few bytes changed, cas_id will probably remains intact
								// so we overwrote our previous thumbnail, so we can't remove it
								if !was_overwritten {
									// remove the old thumbnail as we're generating a new one
									let thumb_path = ThumbnailKind::Indexed(library_id)
										.compute_path(node.config.data_directory(), &old_cas_id);
									if let Err(e) = fs::remove_file(&thumb_path).await {
										error!(
											e = ?FileIOError::from((thumb_path, e)),
											"Failed to remove old thumbnail;",
										);
									}
								}
							});
						}
					}
				}
			}

			if let Some(extension) = &file_path.extension {
				match kind {
					ObjectKind::Image => {
						if let Ok(image_extension) = ImageExtension::from_str(extension) {
							if exif_media_data::can_extract(image_extension) {
								if let Ok(Some(exif_data)) = exif_media_data::extract(full_path)
									.await
									.map_err(|e| error!(?e, "Failed to extract media data;"))
								{
									exif_media_data::save(
										[(exif_data, object.id, object.pub_id.as_slice().into())],
										db,
										sync,
									)
									.await?;
								}
							}
						}
					}

					ObjectKind::Audio => {
						if let Ok(audio_extension) = AudioExtension::from_str(extension) {
							if ffmpeg_media_data::can_extract_for_audio(audio_extension) {
								if let Ok(ffmpeg_data) = ffmpeg_media_data::extract(full_path)
									.await
									.map_err(|e| error!(?e, "Failed to extract media data;"))
								{
									ffmpeg_media_data::save([(ffmpeg_data, object.id)], db).await?;
								}
							}
						}
					}

					ObjectKind::Video => {
						if let Ok(video_extension) = VideoExtension::from_str(extension) {
							if ffmpeg_media_data::can_extract_for_video(video_extension) {
								if let Ok(ffmpeg_data) = ffmpeg_media_data::extract(full_path)
									.await
									.map_err(|e| error!(?e, "Failed to extract media data;"))
								{
									ffmpeg_media_data::save([(ffmpeg_data, object.id)], db).await?;
								}
							}
						}
					}

					_ => {
						// Do nothing
					}
				}
			}
		}

		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	} else if is_hidden != file_path.hidden.unwrap_or_default() {
		let (sync_param, db_param) = sync_db_entry!(is_hidden, file_path::hidden);

		sync.write_op(
			db,
			sync.shared_update(
				prisma_sync::file_path::SyncId {
					pub_id: file_path.pub_id.clone(),
				},
				[sync_param],
			),
			db.file_path()
				.update(
					file_path::pub_id::equals(file_path.pub_id.clone()),
					vec![db_param],
				)
				.select(file_path::select!({ id })),
		)
		.await?;

		invalidate_query!(library, "search.paths");
	}

	Ok(())
}

#[instrument(
	skip_all,
	fields(new_path = %new_path.as_ref().display(), old_path = %old_path.as_ref().display()),
	err,
)]
pub(super) async fn rename(
	location_id: location::id::Type,
	new_path: impl AsRef<Path> + Send,
	old_path: impl AsRef<Path> + Send,
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

	// Renaming a file could potentially be a move to another directory,
	// so we check if our parent changed
	if old_path_materialized_str != new_path_materialized_str
		&& !check_file_path_exists::<FilePathError>(
			&IsolatedFilePathData::new(location_id, &location_path, new_path, true)?.parent(),
			db,
		)
		.await?
	{
		return Err(LocationManagerError::MoveError {
			path: new_path.into(),
			reason: "parent directory does not exist",
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
		let new_parts = new.to_parts();

		// If the renamed path is a directory, we have to update every successor
		if is_dir {
			let old = IsolatedFilePathData::new(location_id, &location_path, old_path, is_dir)?;
			let old_parts = old.to_parts();

			let starts_with = format!("{}/{}/", old_parts.materialized_path, old_parts.name);
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

			let total_paths_count = paths.len();
			let (sync_params, db_params) = paths
				.into_iter()
				.filter_map(|path| path.materialized_path.map(|mp| (path.id, path.pub_id, mp)))
				.map(|(id, pub_id, mp)| {
					let new_path = mp.replace(
						&starts_with,
						&format!("{}/{}/", new_parts.materialized_path, new_parts.name),
					);

					let (sync_param, db_param) =
						sync_db_entry!(new_path, file_path::materialized_path);

					(
						sync.shared_update(
							sd_prisma::prisma_sync::file_path::SyncId { pub_id },
							[sync_param],
						),
						db.file_path()
							.update(file_path::id::equals(id), vec![db_param])
							.select(file_path::select!({ id })),
					)
				})
				.unzip::<_, _, Vec<_>, Vec<_>>();

			if !sync_params.is_empty() && !db_params.is_empty() {
				sync.write_ops(db, (sync_params, db_params)).await?;
			}

			trace!(%total_paths_count, "Updated file_paths;");
		}

		let (sync_params, db_params) = [
			sync_db_entry!(new_path_materialized_str, file_path::materialized_path),
			sync_db_entry!(new_parts.name.to_string(), file_path::name),
			sync_db_entry!(new_parts.extension.to_string(), file_path::extension),
			sync_db_entry!(
				DateTime::<Utc>::from(new_path_metadata.modified_or_now()),
				file_path::date_modified
			),
			sync_db_entry!(
				path_is_hidden(new_path, &new_path_metadata),
				file_path::hidden
			),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		sync.write_op(
			db,
			sync.shared_update(
				prisma_sync::file_path::SyncId {
					pub_id: file_path.pub_id.clone(),
				},
				sync_params,
			),
			db.file_path()
				.update(file_path::pub_id::equals(file_path.pub_id), db_params)
				.select(file_path::select!({ id })),
		)
		.await?;

		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	}

	Ok(())
}

#[instrument(skip_all, fields(path = %path.as_ref().display()), err)]
pub(super) async fn remove(
	location_id: location::id::Type,
	path: impl AsRef<Path> + Send,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let full_path = path.as_ref();
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
		.select(file_path_watcher_remove::select())
		.exec()
		.await?
	else {
		return Ok(());
	};

	remove_by_file_path(location_id, full_path, file_path, library).await
}

async fn remove_by_file_path(
	location_id: location::id::Type,
	path: impl AsRef<Path> + Send,
	file_path: file_path_watcher_remove::Data,
	library: &Library,
) -> Result<(), LocationManagerError> {
	// check file still exists on disk
	match fs::metadata(path.as_ref()).await {
		Ok(_) => {
			// It's possible that in the interval of time between the removal file event being
			// received and we reaching this point, the file has been created again for some
			// external reason, so we just error out and hope to receive this new create event
			// later
			return Err(LocationManagerError::FileStillExistsOnDisk(
				path.as_ref().into(),
			));
		}
		Err(e) if e.kind() == ErrorKind::NotFound => {
			let Library { sync, db, .. } = library;

			let is_dir = maybe_missing(file_path.is_dir, "file_path.is_dir")?;

			// if is doesn't, we can remove it safely from our db
			if is_dir {
				delete_directory(
					library,
					location_id,
					Some(&IsolatedFilePathData::try_from(&file_path)?),
				)
				.await?;
			} else {
				sync.write_op(
					db,
					sync.shared_delete(prisma_sync::file_path::SyncId {
						pub_id: file_path.pub_id,
					}),
					db.file_path().delete(file_path::id::equals(file_path.id)),
				)
				.await?;

				if let Some(object) = file_path.object {
					// If this object doesn't have any other file paths, delete it
					if db
						.object()
						.count(vec![
							object::id::equals(object.id),
							// https://www.prisma.io/docs/reference/api-reference/prisma-client-reference#none
							object::file_paths::none(vec![]),
						])
						.exec()
						.await? == 1
					{
						sync.write_op(
							db,
							sync.shared_delete(prisma_sync::object::SyncId {
								pub_id: object.pub_id,
							}),
							db.object()
								.delete(object::id::equals(object.id))
								.select(object::select!({ id })),
						)
						.await?;
					}
				}
			}
		}
		Err(e) => return Err(FileIOError::from((path, e)).into()),
	}

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

	Ok(())
}

#[instrument(skip_all, fields(path = %path.as_ref().display()), err)]
pub(super) async fn extract_inode_from_path(
	location_id: location::id::Type,
	path: impl AsRef<Path> + Send,
	library: &Library,
) -> Result<INode, LocationManagerError> {
	let path = path.as_ref();
	let location = find_location(library, location_id)
		.select(location::select!({ path }))
		.exec()
		.await?
		.ok_or(LocationManagerError::LocationNotFound(location_id))?;

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

#[instrument(skip_all, err)]
pub(super) async fn extract_location_path(
	location_id: location::id::Type,
	library: &Library,
) -> Result<PathBuf, LocationManagerError> {
	find_location(library, location_id)
		.select(location::select!({ path }))
		.exec()
		.await?
		.map_or(
			Err(LocationManagerError::LocationNotFound(location_id)),
			// NOTE: The following usage of `PathBuf` doesn't incur a new allocation so it's fine
			|location| Ok(maybe_missing(location.path, "location.path")?.into()),
		)
}
#[instrument(skip_all, err)]
pub(super) async fn recalculate_directories_size(
	candidates: &mut HashMap<PathBuf, Instant>,
	buffer: &mut Vec<(PathBuf, Instant)>,
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
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
						.ok_or(LocationManagerError::LocationNotFound(location_id))?
						.path,
					"location.path",
				)?))
			}

			if let Some(location_path) = &location_path_cache {
				if path != *location_path {
					trace!(
						start_directory = %path.display(),
						end_directory = %location_path.display(),
						"Reverse calculating directory sizes;",
					);
					let mut non_critical_errors = vec![];
					reverse_update_directories_sizes(
						path,
						location_id,
						location_path,
						&library.db,
						&library.sync,
						&mut non_critical_errors,
					)
					.await
					.map_err(sd_core_heavy_lifting::Error::from)?;

					if !non_critical_errors.is_empty() {
						error!(
							?non_critical_errors,
							"Reverse calculating directory sizes finished errors;",
						);
					}

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
		update_location_size(location_id, location_pub_id, library).await?;
	}

	if should_invalidate {
		invalidate_query!(library, "search.paths");
		invalidate_query!(library, "search.objects");
	}

	candidates.extend(buffer.drain(..));

	Ok(())
}
