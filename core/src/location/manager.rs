use crate::{
	invalidate_query,
	library::LibraryContext,
	object::{
		identifier_job::assemble_object_metadata,
		preview::{
			can_generate_thumbnail_for_image, generate_image_thumbnail, THUMBNAIL_CACHE_DIR_NAME,
		},
	},
	prisma::{file_path, location, object},
};

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	str::FromStr,
	time::Duration,
};

use crate::object::identifier_job::ObjectCreationMetadata;
use crate::object::validation::hash::file_checksum;
use chrono::{FixedOffset, Utc};
use futures::{stream::FuturesUnordered, StreamExt};
use int_enum::IntEnum;
use notify::event::AccessKind;
use notify::{
	event::{AccessMode, CreateKind, ModifyKind, RemoveKind, RenameMode},
	Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use once_cell::sync::OnceCell;
use prisma_client_rust::{raw, PrismaValue};
use sd_file_ext::extensions::ImageExtension;
use thiserror::Error;
use tokio::{
	fs,
	io::{self, ErrorKind},
	runtime::Handle,
	select,
	sync::{mpsc, oneshot},
	task::{block_in_place, JoinHandle},
	time::sleep,
};
use tracing::{debug, error, info, trace, warn};

use super::{
	delete_directory, fetch_location, file_path_helper::create_file_path, get_location,
	indexer::indexer_job::indexer_job_location, subtract_location_path,
};

static LOCATION_MANAGER: OnceCell<LocationManager> = OnceCell::new();
const LOCATION_CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub type LocationId = i32;

#[derive(Error, Debug)]
pub enum LocationManagerError {
	#[error("Tried to call new method on an already initialized location manager")]
	AlreadyInitialized,
	#[error("Unable to send location id to be checked by actor: (error: {0})")]
	ActorSendAddLocationError(#[from] mpsc::error::SendError<(LocationId, LibraryContext)>),
	#[error("Unable to send location id to be removed from actor: (error: {0})")]
	ActorSendRemoveLocationError(#[from] mpsc::error::SendError<LocationId>),
	#[error("Watcher error: (error: {0})")]
	WatcherError(#[from] notify::Error),
	#[error("Location missing local path: <id='{0}'>")]
	LocationMissingLocalPath(LocationId),
	#[error("Unable to extract materialized path from location: <id='{0}', path='{1:?}'>")]
	UnableToExtractMaterializedPath(LocationId, PathBuf),
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("I/O error: {0}")]
	IOError(#[from] io::Error),
}

file_path::include!(file_path_with_object { object });

#[derive(Debug)]
pub struct LocationManager {
	add_locations_tx: mpsc::Sender<(LocationId, LibraryContext)>,
	remove_locations_tx: mpsc::Sender<LocationId>,
	stop_tx: Option<oneshot::Sender<()>>,
}

impl LocationManager {
	pub fn global() -> &'static Self {
		LOCATION_MANAGER
			.get()
			.expect("Location manager not initialized")
	}

	pub async fn init() -> Result<&'static Self, LocationManagerError> {
		if LOCATION_MANAGER.get().is_some() {
			return Err(LocationManagerError::AlreadyInitialized);
		}

		let (add_locations_tx, add_locations_rx) = mpsc::channel(128);
		let (remove_locations_tx, remove_locations_rx) = mpsc::channel(128);
		let (stop_tx, stop_rx) = oneshot::channel();

		tokio::spawn(Self::run_locations_checker(
			add_locations_rx,
			remove_locations_rx,
			stop_rx,
		));

		let manager = Self {
			add_locations_tx,
			remove_locations_tx,
			stop_tx: Some(stop_tx),
		};

		LOCATION_MANAGER.set(manager).unwrap(); // SAFETY: We checked that it's not set before

		debug!("Location manager initialized");

		Ok(Self::global())
	}

	pub async fn add(
		&self,
		location_id: i32,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		self.add_locations_tx
			.send((location_id, library_ctx))
			.await
			.map_err(Into::into)
	}

	pub async fn remove(&self, location_id: LocationId) -> Result<(), LocationManagerError> {
		self.remove_locations_tx
			.send(location_id)
			.await
			.map_err(Into::into)
	}

	async fn run_locations_checker(
		mut add_locations_rx: mpsc::Receiver<(LocationId, LibraryContext)>,
		mut remove_locations_rx: mpsc::Receiver<LocationId>,
		mut stop_rx: oneshot::Receiver<()>,
	) -> Result<(), LocationManagerError> {
		let mut to_check_futures = FuturesUnordered::new();
		let mut to_remove = HashSet::new();
		let mut locations_watched = HashMap::new();
		let mut locations_unwatched = HashMap::new();

		loop {
			select! {
				// To add a new location
				Some((location_id, library_ctx)) = add_locations_rx.recv() => {
					if let Some(location) = get_location(location_id, &library_ctx).await {
						let is_online = check_online(&location, &library_ctx).await;
						let mut watcher = LocationWatcher::new(location, library_ctx.clone()).await?;
						if is_online {
							watcher.watch();
							locations_watched.insert(location_id, watcher);
						} else {
							locations_unwatched.insert(location_id, watcher);
						}

						to_check_futures.push(location_check_sleep(location_id, library_ctx));
					}
				}

				// To remove an location
				Some(location_id) = remove_locations_rx.recv() => {
					to_remove.insert(location_id);
				}

				// Periodically checking locations
				Some((location_id, library_ctx)) = to_check_futures.next() => {
					if let Some(location) = get_location(location_id, &library_ctx).await {
						if let Some(ref local_path_str) = location.local_path.clone() {
							if to_remove.contains(&location_id) {
								unwatch_location(
									location,
									local_path_str,
									&mut locations_watched,
									&mut locations_unwatched
								);
								locations_unwatched.remove(&location_id);
								to_remove.remove(&location_id);
							} else {
								if check_online(&location, &library_ctx).await {
									watch_location(
										location,
										local_path_str,
										&mut locations_watched,
										&mut locations_unwatched
									);
								} else {
									unwatch_location(
										location,
										local_path_str,
										&mut locations_watched,
										&mut locations_unwatched
									);
								}
								to_check_futures.push(location_check_sleep(location_id, library_ctx));
							}
						} else {
							warn!("Dropping location from location manager, \
							 because we don't have a `local_path` anymore: \
							 <id='{location_id}', library_id='{}'>", library_ctx.id);
							if let Some(mut watcher) = locations_watched.remove(&location_id) {
								watcher.unwatch();
							} else {
								locations_unwatched.remove(&location_id);
							}
						}
					} else {
						warn!("Removing location from manager, as we failed to fetch from db: \
						<id='{}'>", location_id);
						if let Some(mut watcher) = locations_watched.remove(&location_id) {
							watcher.unwatch();
						} else {
							locations_unwatched.remove(&location_id);
						}
						to_remove.remove(&location_id);
					}
				}

				_ = &mut stop_rx => {
					info!("Stopping location manager");
					break;
				}
			}
		}

		Ok(())
	}
}

async fn check_online(location: &location::Data, library_ctx: &LibraryContext) -> bool {
	if let Some(ref local_path) = location.local_path {
		match fs::metadata(local_path).await {
			Ok(_) => {
				if !location.is_online {
					set_location_online(location.id, library_ctx, true).await;
				}
				true
			}
			Err(e) if e.kind() == ErrorKind::NotFound => {
				if location.is_online {
					set_location_online(location.id, library_ctx, false).await;
				}
				false
			}
			Err(e) => {
				error!("Failed to check if location is online: {:#?}", e);
				false
			}
		}
	} else {
		// In this case, we don't have a `local_path`, but this location was marked as online
		if location.is_online {
			set_location_online(location.id, library_ctx, false).await;
		}
		false
	}
}

async fn set_location_online(location_id: i32, library_ctx: &LibraryContext, online: bool) {
	if let Err(e) = library_ctx
		.db
		.location()
		.update(
			location::id::equals(location_id),
			vec![location::is_online::set(online)],
		)
		.exec()
		.await
	{
		error!(
			"Failed to update location to online: (id: {}, error: {:#?})",
			location_id, e
		);
	}
}

async fn location_check_sleep(
	location_id: i32,
	library_ctx: LibraryContext,
) -> (i32, LibraryContext) {
	sleep(LOCATION_CHECK_INTERVAL).await;
	(location_id, library_ctx)
}

fn watch_location(
	location: location::Data,
	location_path: impl AsRef<Path>,
	locations_watched: &mut HashMap<i32, LocationWatcher>,
	locations_unwatched: &mut HashMap<i32, LocationWatcher>,
) {
	let location_id = location.id;
	if let Some(mut watcher) = locations_unwatched.remove(&location_id) {
		if watcher.check_path(location_path) {
			watcher.watch();
		} else {
			watcher.update_data(location, true);
		}

		locations_watched.insert(location_id, watcher);
	}
}

fn unwatch_location(
	location: location::Data,
	location_path: impl AsRef<Path>,
	locations_watched: &mut HashMap<i32, LocationWatcher>,
	locations_unwatched: &mut HashMap<i32, LocationWatcher>,
) {
	let location_id = location.id;
	if let Some(mut watcher) = locations_watched.remove(&location_id) {
		if watcher.check_path(location_path) {
			watcher.unwatch();
		} else {
			watcher.update_data(location, false)
		}

		locations_unwatched.insert(location_id, watcher);
	}
}

impl Drop for LocationManager {
	fn drop(&mut self) {
		if let Some(stop_tx) = self.stop_tx.take() {
			if stop_tx.send(()).is_err() {
				error!("Failed to send stop signal to location manager");
			}
		}
	}
}

#[derive(Debug)]
struct LocationWatcher {
	location: location::Data,
	path: PathBuf,
	watcher: RecommendedWatcher,
	handle: Option<JoinHandle<()>>,
	stop_tx: Option<oneshot::Sender<()>>,
}

impl LocationWatcher {
	async fn new(
		location: location::Data,
		library_ctx: LibraryContext,
	) -> Result<Self, LocationManagerError> {
		let (events_tx, events_rx) = mpsc::unbounded_channel();
		let (stop_tx, stop_rx) = oneshot::channel();

		let watcher = RecommendedWatcher::new(
			move |result| {
				if !events_tx.is_closed() {
					if events_tx.send(result).is_err() {
						error!(
						"Unable to send watcher event to location manager for location: <id='{}'>",
						location.id
					);
					}
				} else {
					error!(
						"Tried to send location file system events to a closed channel: <id='{}'",
						location.id
					);
				}
			},
			Config::default(),
		)?;

		let handle = tokio::spawn(Self::handle_watch_events(
			location.id,
			library_ctx,
			events_rx,
			stop_rx,
		));
		let path = PathBuf::from(
			location
				.local_path
				.as_ref()
				.ok_or(LocationManagerError::LocationMissingLocalPath(location.id))?,
		);

		Ok(Self {
			location,
			path,
			watcher,
			handle: Some(handle),
			stop_tx: Some(stop_tx),
		})
	}

	async fn handle_watch_events(
		location_id: i32,
		library_ctx: LibraryContext,
		mut events_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
		mut stop_rx: oneshot::Receiver<()>,
	) {
		loop {
			select! {
				Some(event) = events_rx.recv() => {
					match event {
						Ok(event) => {
							if Self::check_event(&event) {
								if let Err(e) = Self::handle_event(location_id, &library_ctx, event).await {
									error!(
										"Failed to handle location file system event: \
										<id='{location_id}', error='{e:#?}'>",
									);
								}
							}
						}
						Err(e) => {
							error!("watch error: {:#?}", e);
						}
					}
				}
				_ = &mut stop_rx => {
					debug!("Stop Location Manager event handler for location: <id='{}'>", location_id);
					break
				}
			}
		}
	}

	fn check_event(event: &Event) -> bool {
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

	async fn handle_event(
		location_id: i32,
		library_ctx: &LibraryContext,
		event: Event,
	) -> Result<(), LocationManagerError> {
		debug!("Received event: {:#?}", event);
		if let Some(location) = fetch_location(library_ctx, location_id)
			.include(indexer_job_location::include())
			.exec()
			.await?
		{
			// if location is offline return early
			// this prevents ....
			if !location.is_online {
				info!(
					"Location is offline, skipping event: <id='{}'>",
					location.id
				);
				return Ok(());
			}
			match event.kind {
				EventKind::Access(access_kind) => {
					// This
					if access_kind == AccessKind::Close(AccessMode::Write) {
						// If a file was closed with write mode, then it was updated
						Self::handle_file_creation_or_update(location, event, library_ctx).await?;
					} else {
						trace!("Ignoring access event: {:#?}", event);
					}
				}
				EventKind::Create(create_kind) => {
					if create_kind == CreateKind::Folder {
						Self::handle_create_dir_event(location, event, library_ctx).await?;
					} else {
						trace!("Ignored create event: {:#?}", event);
					}
				}
				EventKind::Modify(ref modify_kind) => {
					let modify_kind = modify_kind.clone();
					Self::handle_modify_event(location, event, modify_kind, library_ctx).await?;
				}
				EventKind::Remove(remove_kind) => {
					Self::handle_remove_event(location, event, remove_kind, library_ctx).await?;
				}
				other_event_kind => {
					debug!("Other event that we don't handle for now: {other_event_kind:#?}");
				}
			}
		}
		Ok(())
	}

	async fn handle_create_dir_event(
		location: indexer_job_location::Data,
		event: Event,
		library_ctx: &LibraryContext,
	) -> Result<(), LocationManagerError> {
		if let Some(ref location_local_path) = location.local_path {
			debug!(
				"Location: <root_path ='{location_local_path}'> creating directory: {}",
				event.paths[0].display()
			);

			if let Some(subpath) = subtract_location_path(&location_local_path, &event.paths[0]) {
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
						library_ctx,
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

	async fn handle_file_creation_or_update(
		location: indexer_job_location::Data,
		event: Event,
		library_ctx: &LibraryContext,
	) -> Result<(), LocationManagerError> {
		if let Some(ref location_local_path) = location.local_path {
			if let Some(file_path) =
				get_existing_file_path(&location, &event.paths[0], library_ctx).await?
			{
				Self::update_file(location_local_path, file_path, event, library_ctx).await
			} else {
				// We received None because it is a new file
				Self::create_file(location.id, location_local_path, event, library_ctx).await
			}
		} else {
			Err(LocationManagerError::LocationMissingLocalPath(location.id))
		}
	}

	async fn update_file(
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
							object::integrity_checksum::set(
								if object.integrity_checksum.is_some() {
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

	async fn create_file(
		location_id: i32,
		location_local_path: &str,
		event: Event,
		library_ctx: &LibraryContext,
	) -> Result<(), LocationManagerError> {
		debug!(
			"Location: <root_path ='{location_local_path}'> creating file: {}",
			event.paths[0].display()
		);
		if let Some(materialized_path) =
			subtract_location_path(&location_local_path, &event.paths[0])
		{
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
								Utc::now().with_timezone(&FixedOffset::east(0)),
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

	async fn handle_modify_event(
		location: indexer_job_location::Data,
		event: Event,
		modify_kind: ModifyKind,
		library_ctx: &LibraryContext,
	) -> Result<(), LocationManagerError> {
		debug!("modified {modify_kind:#?}");

		match modify_kind {
			ModifyKind::Data(_) => {
				// We ignore data changes here, because AccessKind::Close(Write) is a more generic
				// event for data changes
			}
			ModifyKind::Metadata(_) => {
				// Metadata modifications are ignored as we already update `date_modified` on every
				// file modification, at EventKind::Access
			}
			ModifyKind::Name(modify_name) => {
				// There are 3 kinds of rename events, To, From and Both.
				// But we can only update our data in the Both kind...
				if matches!(modify_name, RenameMode::Both) {
					let old_path = extract_materialized_path(&location, &event.paths[0])?
						.to_string_lossy()
						.to_string();
					let new_path = extract_materialized_path(&location, &event.paths[1])?;

					if let Some(file_path) =
						get_existing_file_path(&location, &event.paths[0], library_ctx).await?
					{
						// If the renamed path is a directory, we have to update every successor
						if file_path.is_dir {
							let updated = library_ctx
								.db
								._execute_raw(raw!(
								"UPDATE file_path SET materialized_path = REPLACE(materialized_path, {}, {}) WHERE location_id = {}",
									PrismaValue::String(old_path),
									PrismaValue::String(
										new_path
											.to_string_lossy()
											.to_string()
									),
									PrismaValue::Int(location.id as i64)
								))
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
										new_path.to_string_lossy().to_string(),
									),
									file_path::name::set(
										new_path.file_stem().unwrap().to_string_lossy().to_string(),
									),
									file_path::extension::set(
										new_path
											.extension()
											.map(|s| s.to_string_lossy().to_string()),
									),
								],
							)
							.exec()
							.await?;
					}
				}
			}
			ModifyKind::Other | ModifyKind::Any => {
				debug!("Ignoring modify events of Other and Any kinds for now");
			}
		}

		Ok(())
	}

	async fn handle_remove_event(
		location: indexer_job_location::Data,
		event: Event,
		remove_kind: RemoveKind,
		library_ctx: &LibraryContext,
	) -> Result<(), LocationManagerError> {
		debug!("removed {remove_kind:#?}");

		// check if path exists in our db, if it doesn't, then we don't care
		if let Some(file_path) =
			get_existing_file_path(&location, &event.paths[0], library_ctx).await?
		{
			// check file still exists on disk
			match fs::metadata(&event.paths[0]).await {
				Ok(_) => {
					todo!("file has changed in some way, re-identify it")
				}
				Err(e) if e.kind() == ErrorKind::NotFound => {
					// if is doesn't, we can remove it safely from our db
					if file_path.is_dir {
						delete_directory(
							library_ctx,
							location.id,
							Some(file_path.materialized_path),
						)
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

	fn check_path(&self, path: impl AsRef<Path>) -> bool {
		self.path == path.as_ref()
	}

	fn watch(&mut self) {
		if let Err(e) = self.watcher.watch(&self.path, RecursiveMode::Recursive) {
			error!(
				"Unable to watch location: (path: {}, error: {e:#?})",
				self.path.display()
			);
		} else {
			debug!("Now watching location: (path: {})", self.path.display());
		}
	}

	fn unwatch(&mut self) {
		if let Err(e) = self.watcher.unwatch(&self.path) {
			/**************************************** TODO: ****************************************
			 * According to an unit test, this error may occur when a subdirectory is removed	   *
			 * and we try to unwatch the parent directory then we have to check the implications   *
			 * of unwatch error for this case.   												   *
			 **************************************************************************************/
			error!(
				"Unable to unwatch location: (path: {}, error: {e:#?})",
				self.path.display()
			);
		} else {
			debug!("Stop watching location: (path: {})", self.path.display());
		}
	}

	fn update_data(&mut self, location: location::Data, to_watch: bool) {
		assert_eq!(
			self.location.id, location.id,
			"Updated location data must have the same id"
		);
		let path = PathBuf::from(location.local_path.as_ref().unwrap_or_else(|| {
			panic!(
				"Tried to watch a location without local_path: <id='{}'>",
				location.id
			)
		}));

		if self.path != path {
			self.unwatch();
			self.path = path;
			if to_watch {
				self.watch();
			}
		}
		self.location = location;
	}
}

impl Drop for LocationWatcher {
	fn drop(&mut self) {
		if let Some(stop_tx) = self.stop_tx.take() {
			if stop_tx.send(()).is_err() {
				error!(
					"Failed to send stop signal to location watcher: <id='{}'>",
					self.location.id
				);
			}

			// FIXME: change this Drop to async drop in the future
			if let Some(handle) = self.handle.take() {
				if let Err(e) =
					block_in_place(move || Handle::current().block_on(async move { handle.await }))
				{
					error!("Failed to join watcher task: {e:#?}")
				}
			}
		}
	}
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
	location_id: i32,
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

/***************************************************************************************************
 * Some tests to validate our assumptions of events through different file systems                 *
 ***************************************************************************************************
 * Events dispatched on Linux:								    						           *
 * 		Create File:																			   *
 *			1) EventKind::Create(CreateKind::File)												   *
 *			2) EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any))						   *
 *				or EventKind::Modify(ModifyKind::Data(DataChange::Any))							   *
 *          3) EventKind::Access(AccessKind::Close(AccessMode::Write)))							   *
 *		Create Directory:																		   *
 *			1) EventKind::Create(CreateKind::Folder)											   *
 *      Update File:										   									   *
 *			1) EventKind::Modify(ModifyKind::Data(DataChange::Any))								   *
 *			2) EventKind::Access(AccessKind::Close(AccessMode::Write)))							   *
 *		Update File (rename):																	   *
 *			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
 *			1) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
 *			1) EventKind::Modify(ModifyKind::Name(RenameMode::Both))							   *
 *		Update Directory (rename):																   *
 *			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
 *			1) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
 *			1) EventKind::Modify(ModifyKind::Name(RenameMode::Both))							   *
 *	 	Delete File:																			   *
 *			1) EventKind::Remove(RemoveKind::File)												   *
 *		Delete Directory:																		   *
 *			1) EventKind::Remove(RemoveKind::Folder)											   *
 *																								   *
 * Events dispatched on MacOS:																	   *
 * TODO																							   *
 *																								   *
 * Events dispatched on Windows:																   *
 * TODO																							   *
 *																								   *
 * Events dispatched on Android:																   *
 * TODO																							   *
 *																								   *
 * Events dispatched on iOS:																	   *
 * TODO																							   *
 *																								   *
 **************************************************************************************************/
#[cfg(test)]
mod tests {
	use notify::{
		event::{AccessKind, AccessMode, CreateKind, ModifyKind, RemoveKind, RenameMode},
		Config, Event, EventKind, RecommendedWatcher, Watcher,
	};
	use std::{path::Path, time::Duration};
	use tempfile::{tempdir, TempDir};
	use tokio::{fs, io::AsyncWriteExt, sync::mpsc, time::sleep};
	use tracing::debug;
	use tracing_test::traced_test;

	async fn setup_watcher() -> (
		TempDir,
		RecommendedWatcher,
		mpsc::UnboundedReceiver<notify::Result<Event>>,
	) {
		let (events_tx, events_rx) = mpsc::unbounded_channel();

		let watcher = RecommendedWatcher::new(
			move |result| {
				events_tx
					.send(result)
					.expect("Unable to send watcher event");
			},
			Config::default(),
		)
		.expect("Failed to create watcher");

		(tempdir().unwrap(), watcher, events_rx)
	}

	async fn expect_event(
		mut events_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
		path: impl AsRef<Path>,
		expected_event: EventKind,
	) {
		let path = path.as_ref();
		let mut tries = 0;
		loop {
			match events_rx.try_recv() {
				Ok(maybe_event) => {
					let event = maybe_event.expect("Failed to receive event");
					debug!("Received event: {event:#?}");
					// In case of file creation, we expect to see an close event on write mode
					if event.paths[0] == path && event.kind == expected_event {
						debug!("Received expected event: {expected_event:#?}");
						break;
					}
				}
				Err(e) => {
					debug!("No event yet: {e:#?}");
					tries += 1;
					sleep(Duration::from_millis(100)).await;
				}
			}

			if tries == 10 {
				panic!("No expected event received after 10 tries");
			}
		}
	}

	#[tokio::test]
	#[traced_test]
	async fn create_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		expect_event(
			events_rx,
			&file_path,
			EventKind::Access(AccessKind::Close(AccessMode::Write)),
		)
		.await;

		watcher
			.unwatch(root_dir.path())
			.expect("Failed to unwatch root directory");
	}

	#[tokio::test]
	#[traced_test]
	async fn create_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Folder)).await;

		watcher
			.unwatch(root_dir.path())
			.expect("Failed to unwatch root directory");
	}

	#[tokio::test]
	#[traced_test]
	async fn update_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let mut file = fs::OpenOptions::new()
			.append(true)
			.open(&file_path)
			.await
			.expect("Failed to open file");

		// Writing then sync data before closing the file
		file.write_all(b"\nanother test")
			.await
			.expect("Failed to write to file");
		file.sync_all().await.expect("Failed to flush file");
		drop(file);

		expect_event(
			events_rx,
			&file_path,
			EventKind::Access(AccessKind::Close(AccessMode::Write)),
		)
		.await;

		watcher
			.unwatch(root_dir.path())
			.expect("Failed to unwatch root directory");
	}

	#[tokio::test]
	#[traced_test]
	async fn update_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let new_dir_name = root_dir.path().join("inner2");

		fs::rename(&dir_path, &new_dir_name)
			.await
			.expect("Failed to rename directory");

		expect_event(
			events_rx,
			&dir_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
		)
		.await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		watcher
			.unwatch(root_dir.path())
			.expect("Failed to unwatch root directory");
	}

	#[tokio::test]
	#[traced_test]
	async fn delete_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		fs::remove_file(&file_path)
			.await
			.expect("Failed to remove file");

		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::File)).await;

		watcher
			.unwatch(root_dir.path())
			.expect("Failed to unwatch root directory");
	}

	#[tokio::test]
	#[traced_test]
	async fn delete_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		debug!("First unwatching the inner directory before removing it");
		watcher
			.unwatch(&dir_path)
			.expect("Failed to unwatch inner directory");

		fs::remove_dir(&dir_path)
			.await
			.expect("Failed to remove directory");

		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Folder)).await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		watcher
			.unwatch(root_dir.path())
			.expect("Failed to unwatch root directory");
	}
}
