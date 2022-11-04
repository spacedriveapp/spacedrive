use crate::{
	job::Job,
	library::LibraryContext,
	location::{
		fetch_location,
		indexer::indexer_job::{indexer_job_location, IndexerJob, IndexerJobInit},
	},
	prisma::location,
};

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	time::Duration,
};

use crate::object::identifier_job::current_dir_identifier_job::{
	CurrentDirFileIdentifierJob, CurrentDirFileIdentifierJobInit,
};
use crate::prisma::{file_path, object};
use futures::{stream::FuturesUnordered, StreamExt};
use notify::event::CreateKind;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use thiserror::Error;
use tokio::{
	fs, io,
	runtime::Handle,
	select,
	sync::{mpsc, oneshot},
	task::{block_in_place, JoinHandle},
	time::sleep,
};
use tracing::{debug, error, info, warn};

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
}

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

async fn get_location(location_id: i32, library_ctx: &LibraryContext) -> Option<location::Data> {
	library_ctx
		.db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await
		.unwrap_or_else(|err| {
			error!("Failed to get location data from location_id: {:#?}", err);
			None
		})
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
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
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

fn strip_location_root_path(
	location_path: impl AsRef<Path>,
	current_path: impl AsRef<Path>,
) -> Option<PathBuf> {
	let location_path = location_path.as_ref();
	let current_path = current_path.as_ref();

	if let Ok(stripped) = current_path.strip_prefix(location_path) {
		Some(stripped.to_path_buf())
	} else {
		error!(
			"Failed to strip location root path ({}) from current path ({})",
			location_path.display(),
			current_path.display()
		);
		None
	}
}

fn strip_location_root_path_and_filename(
	location_path: impl AsRef<Path>,
	current_path: impl AsRef<Path>,
) -> Option<PathBuf> {
	let location_path = location_path.as_ref();
	let current_path = current_path.as_ref();

	if let Ok(stripped) = current_path.strip_prefix(location_path) {
		if let Some(parent) = stripped.parent() {
			return Some(parent.to_path_buf());
		}
	}
	error!(
		"Failed to strip location root path ({}) and filename from current path ({})",
		location_path.display(),
		current_path.display()
	);

	None
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
							Self::handle_event(location_id, &library_ctx, event).await;
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

	async fn handle_event(location_id: i32, library_ctx: &LibraryContext, event: Event) {
		debug!("Received event: {:#?}", event);
		match event.kind {
			EventKind::Create(create_kind) => {
				debug!("created {create_kind:#?}");
				if let Some(location) = fetch_location(library_ctx, location_id)
					.include(indexer_job_location::include())
					.exec()
					.await
					.unwrap_or_else(|err| {
						error!("Failed to get location data from location_id: {:#?}", err);
						None
					}) {
					if let Some(local_path) = location.local_path.clone() {
						library_ctx
							.queue_job(Job::new(IndexerJobInit { location }, IndexerJob {}))
							.await;

						let maybe_root_path = match create_kind {
							CreateKind::File => {
								strip_location_root_path_and_filename(&local_path, &event.paths[0])
							}
							CreateKind::Folder => {
								strip_location_root_path(&local_path, &event.paths[0])
							}
							_ => None,
						};

						if let Some(root_path) = maybe_root_path {
							library_ctx
								.queue_job(Job::new(
									CurrentDirFileIdentifierJobInit {
										location_id,
										root_path,
									},
									CurrentDirFileIdentifierJob {},
								))
								.await;
						}
					} else {
						error!(
							"Missing local_path at location in watcher: <id='{}'>",
							location_id
						);
					}
				}
			}
			EventKind::Modify(modify_kind) => {
				// TODO: Handle file modifications, to recompute object metadata and handle file renames
				debug!("modified {modify_kind:#?}");
			}
			EventKind::Remove(remove_kind) => {
				debug!("removed {remove_kind:#?}");
				if let Some(location) = get_location(location_id, library_ctx).await {
					if let Some(ref local_path) = location.local_path {
						let file_paths = event
							.paths
							.iter()
							.filter_map(|path| strip_location_root_path(local_path, path))
							.map(|path| path.to_string_lossy().to_string())
							.collect();

						if let Ok(file_paths) = library_ctx
							.db
							.file_path()
							.find_many(vec![file_path::materialized_path::in_vec(file_paths)])
							.exec()
							.await
						{
							let file_paths_ids =
								file_paths.iter().map(|file_path| file_path.id).collect();
							let object_ids = file_paths
								.iter()
								.filter_map(|file_path| file_path.object_id)
								.collect();

							if let Err(e) = library_ctx
								.db
								.file_path()
								.delete_many(vec![file_path::id::in_vec(file_paths_ids)])
								.exec()
								.await
							{
								error!(
									"Failed to delete file_paths from location: <id='{}'>; {e:#?}",
									location_id
								);
							} else if let Err(e) = library_ctx
								.db
								.object()
								.delete_many(vec![object::id::in_vec(object_ids)])
								.exec()
								.await
							{
								error!(
									"Failed to delete file_paths from location: <id='{}'>; {e:#?}",
									location_id
								);
							}
						}
					}
				}
			}
			other_event_kind => {
				debug!("Other event that we don't handle for now: {other_event_kind:#?}");
			}
		}
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
