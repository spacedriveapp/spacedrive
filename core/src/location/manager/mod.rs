use crate::{
	job::JobManagerError,
	library::{Library, LibraryManagerEvent},
	prisma::location,
	util::{db::MissingFieldError, error::FileIOError},
	Node,
};

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::executor::block_on;
use thiserror::Error;
use tokio::sync::{
	broadcast::{self, Receiver},
	oneshot, RwLock,
};
use tracing::error;

#[cfg(feature = "location-watcher")]
use tokio::sync::mpsc;
use uuid::Uuid;

use super::file_path_helper::FilePathError;

#[cfg(feature = "location-watcher")]
mod watcher;

#[cfg(feature = "location-watcher")]
mod helpers;

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum ManagementMessageAction {
	Add,
	Remove,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct LocationManagementMessage {
	location_id: location::id::Type,
	library: Arc<Library>,
	action: ManagementMessageAction,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
}

#[derive(Debug)]
#[allow(dead_code)]
enum WatcherManagementMessageAction {
	Stop,
	Reinit,
	IgnoreEventsForPath { path: PathBuf, ignore: bool },
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct WatcherManagementMessage {
	location_id: location::id::Type,
	library: Arc<Library>,
	action: WatcherManagementMessageAction,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
}

#[derive(Error, Debug)]
pub enum LocationManagerError {
	#[cfg(feature = "location-watcher")]
	#[error("Unable to send location management message to location manager actor: (error: {0})")]
	ActorSendLocationError(#[from] mpsc::error::SendError<LocationManagementMessage>),

	#[cfg(feature = "location-watcher")]
	#[error("Unable to send path to be ignored by watcher actor: (error: {0})")]
	ActorIgnorePathError(#[from] mpsc::error::SendError<watcher::IgnorePath>),

	#[cfg(feature = "location-watcher")]
	#[error("Unable to watcher management message to watcher manager actor: (error: {0})")]
	ActorIgnorePathMessageError(#[from] mpsc::error::SendError<WatcherManagementMessage>),

	#[error("Unable to receive actor response: (error: {0})")]
	ActorResponseError(#[from] oneshot::error::RecvError),

	#[cfg(feature = "location-watcher")]
	#[error("Watcher error: (error: {0})")]
	WatcherError(#[from] notify::Error),

	#[error("Failed to stop or reinit a watcher: {reason}")]
	FailedToStopOrReinitWatcher { reason: String },

	#[error("Missing location from database: <id='{0}'>")]
	MissingLocation(location::id::Type),

	#[error("Non local location: <id='{0}'>")]
	NonLocalLocation(location::id::Type),

	#[error("failed to move file '{}' for reason: {reason}", .path.display())]
	MoveError { path: Box<Path>, reason: String },

	#[error("Tried to update a non-existing file: <path='{0}'>")]
	UpdateNonExistingFile(PathBuf),
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("File path related error (error: {0})")]
	FilePathError(#[from] FilePathError),
	#[error("Corrupted location pub_id on database: (error: {0})")]
	CorruptedLocationPubId(#[from] uuid::Error),
	#[error("Job Manager error: (error: {0})")]
	JobManager(#[from] JobManagerError),
	#[error("missing-field")]
	MissingField(#[from] MissingFieldError),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

type OnlineLocations = BTreeSet<Vec<u8>>;

#[must_use = "'LocationManagerActor::start' must be used to start the actor"]
pub struct LocationManagerActor {
	#[cfg(feature = "location-watcher")]
	location_management_rx: mpsc::Receiver<LocationManagementMessage>,
	#[cfg(feature = "location-watcher")]
	watcher_management_rx: mpsc::Receiver<WatcherManagementMessage>,
	#[cfg(feature = "location-watcher")]
	stop_rx: oneshot::Receiver<()>,
}

impl LocationManagerActor {
	pub fn start(self, node: Arc<Node>) {
		tokio::spawn({
			let node = node.clone();
			let rx = node.libraries.rx.clone();
			async move {
				if let Err(err) = rx
					.subscribe(|event| {
						let node = node.clone();
						async move {
							match event {
								LibraryManagerEvent::Load(library) => {
									for location in library
										.db
										.location()
										.find_many(vec![])
										.exec()
										.await
										.unwrap_or_else(|e| {
											error!(
													"Failed to get locations from database for location manager: {:#?}",
													e
												);
											vec![]
										}) {
										if let Err(e) =
											node.locations.add(location.id, library.clone()).await
										{
											error!(
												"Failed to add location to location manager: {:#?}",
												e
											);
										}
									}
								}
								LibraryManagerEvent::Edit(_) => {}
								LibraryManagerEvent::InstancesModified(_) => {}
								LibraryManagerEvent::Delete(_) => {
									#[cfg(debug_assertions)]
									todo!("TODO: Remove locations from location manager"); // TODO
								}
							}
						}
					})
					.await
				{
					error!("Core may become unstable! LocationManager's library manager subscription aborted with error: {err:?}");
				}
			}
		});

		#[cfg(feature = "location-watcher")]
		tokio::spawn(Locations::run_locations_checker(
			self.location_management_rx,
			self.watcher_management_rx,
			self.stop_rx,
			node,
		));

		#[cfg(not(feature = "location-watcher"))]
		tracing::warn!("Location watcher is disabled, locations will not be checked");
	}
}

pub struct Locations {
	online_locations: RwLock<OnlineLocations>,
	pub online_tx: broadcast::Sender<OnlineLocations>,
	#[cfg(feature = "location-watcher")]
	location_management_tx: mpsc::Sender<LocationManagementMessage>,
	#[cfg(feature = "location-watcher")]
	watcher_management_tx: mpsc::Sender<WatcherManagementMessage>,
	stop_tx: Option<oneshot::Sender<()>>,
}

impl Locations {
	pub fn new() -> (Self, LocationManagerActor) {
		let online_tx = broadcast::channel(16).0;

		#[cfg(feature = "location-watcher")]
		{
			let (location_management_tx, location_management_rx) = mpsc::channel(128);
			let (watcher_management_tx, watcher_management_rx) = mpsc::channel(128);
			let (stop_tx, stop_rx) = oneshot::channel();

			(
				Self {
					online_locations: Default::default(),
					online_tx,
					location_management_tx,
					watcher_management_tx,
					stop_tx: Some(stop_tx),
				},
				LocationManagerActor {
					location_management_rx,
					watcher_management_rx,
					stop_rx,
				},
			)
		}

		#[cfg(not(feature = "location-watcher"))]
		{
			tracing::warn!("Location watcher is disabled, locations will not be checked");
			(
				Self {
					online_tx,
					online_locations: Default::default(),
					stop_tx: None,
				},
				LocationManagerActor {},
			)
		}
	}

	#[inline]
	#[allow(unused_variables)]
	async fn location_management_message(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		action: ManagementMessageAction,
	) -> Result<(), LocationManagerError> {
		#[cfg(feature = "location-watcher")]
		{
			let (tx, rx) = oneshot::channel();

			self.location_management_tx
				.send(LocationManagementMessage {
					location_id,
					library,
					action,
					response_tx: tx,
				})
				.await?;

			rx.await?
		}

		#[cfg(not(feature = "location-watcher"))]
		Ok(())
	}

	#[inline]
	#[allow(unused_variables)]
	async fn watcher_management_message(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		action: WatcherManagementMessageAction,
	) -> Result<(), LocationManagerError> {
		#[cfg(feature = "location-watcher")]
		{
			let (tx, rx) = oneshot::channel();

			self.watcher_management_tx
				.send(WatcherManagementMessage {
					location_id,
					library,
					action,
					response_tx: tx,
				})
				.await?;

			rx.await?
		}

		#[cfg(not(feature = "location-watcher"))]
		Ok(())
	}

	pub async fn add(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		self.location_management_message(location_id, library, ManagementMessageAction::Add)
			.await
	}

	pub async fn remove(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		self.location_management_message(location_id, library, ManagementMessageAction::Remove)
			.await
	}

	pub async fn stop_watcher(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		self.watcher_management_message(location_id, library, WatcherManagementMessageAction::Stop)
			.await
	}

	pub async fn reinit_watcher(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		self.watcher_management_message(
			location_id,
			library,
			WatcherManagementMessageAction::Reinit,
		)
		.await
	}

	pub async fn temporary_stop(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<StopWatcherGuard, LocationManagerError> {
		self.stop_watcher(location_id, library.clone()).await?;

		Ok(StopWatcherGuard {
			location_id,
			library: Some(library),
			manager: self,
		})
	}

	pub async fn temporary_ignore_events_for_path(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		path: impl AsRef<Path>,
	) -> Result<IgnoreEventsForPathGuard, LocationManagerError> {
		let path = path.as_ref().to_path_buf();

		self.watcher_management_message(
			location_id,
			library.clone(),
			WatcherManagementMessageAction::IgnoreEventsForPath {
				path: path.clone(),
				ignore: true,
			},
		)
		.await?;

		Ok(IgnoreEventsForPathGuard {
			location_id,
			library: Some(library),
			manager: self,
			path: Some(path),
		})
	}

	#[cfg(feature = "location-watcher")]
	async fn run_locations_checker(
		mut location_management_rx: mpsc::Receiver<LocationManagementMessage>,
		mut watcher_management_rx: mpsc::Receiver<WatcherManagementMessage>,
		mut stop_rx: oneshot::Receiver<()>,
		node: Arc<Node>,
	) -> Result<(), LocationManagerError> {
		use std::collections::{HashMap, HashSet};

		use futures::stream::{FuturesUnordered, StreamExt};
		use tokio::select;
		use tracing::{info, warn};

		use helpers::{
			check_online, drop_location, get_location, handle_ignore_path_request,
			handle_reinit_watcher_request, handle_remove_location_request,
			handle_stop_watcher_request, location_check_sleep, unwatch_location, watch_location,
		};
		use watcher::LocationWatcher;

		let mut to_check_futures = FuturesUnordered::new();
		let mut to_remove = HashSet::new();
		let mut locations_watched = HashMap::new();
		let mut locations_unwatched = HashMap::new();
		let mut forced_unwatch = HashSet::new();

		loop {
			select! {
				// Location management messages
				Some(LocationManagementMessage{
					location_id,
					library,
					action,
					response_tx
				}) = location_management_rx.recv() => {
					match action {

						// To add a new location
						ManagementMessageAction::Add => {
							response_tx.send(
							if let Some(location) = get_location(location_id, &library).await {
								match check_online(&location, &node, &library).await {
									Ok(is_online) => {

										LocationWatcher::new(location, library.clone(), node.clone())
										.await
										.map(|mut watcher| {
											if is_online {
												watcher.watch();
												locations_watched.insert(
													(location_id, library.id),
													watcher
												);
											} else {
												locations_unwatched.insert(
													(location_id, library.id),
													watcher
												);
											}

											to_check_futures.push(
												location_check_sleep(location_id, library)
											);
										}
									)
									},
									Err(e) => {
										error!("Error while checking online status of location {location_id}: {e}");
										Ok(()) // TODO: Probs should be error but that will break startup when location is offline
									}
								}
							} else {
								warn!(
									"Location not found in database to be watched: {}",
									location_id
								);
								Ok(()) // TODO: Probs should be error but that will break startup when location is offline
							}).ok(); // ignore errors, we handle errors on receiver
						},

						// To remove an location
						ManagementMessageAction::Remove => {
							handle_remove_location_request(
								location_id,
								library,
								response_tx,
								&mut forced_unwatch,
								&mut locations_watched,
								&mut locations_unwatched,
								&mut to_remove,
							).await;
						},
					}
				}

				// Watcher management messages
				Some(WatcherManagementMessage{
					location_id,
					library,
					action,
					response_tx,
				}) = watcher_management_rx.recv() => {
					match action {
						// To stop a watcher
						WatcherManagementMessageAction::Stop => {
							handle_stop_watcher_request(
								location_id,
								library,
								response_tx,
								&mut forced_unwatch,
								&mut locations_watched,
								&mut locations_unwatched,
							).await;
						},

						// To reinit a stopped watcher
						WatcherManagementMessageAction::Reinit => {
							handle_reinit_watcher_request(
								location_id,
								library,
								response_tx,
								&mut forced_unwatch,
								&mut locations_watched,
								&mut locations_unwatched,
							).await;
						},

						// To ignore or not events for a path
						WatcherManagementMessageAction::IgnoreEventsForPath { path, ignore } => {
							handle_ignore_path_request(
								location_id,
								library,
								path,
								ignore,
								response_tx,
								&locations_watched,
							);
						},
					}
				}

				// Periodically checking locations
				Some((location_id, library)) = to_check_futures.next() => {
					let key = (location_id, library.id);

					if to_remove.contains(&key) {
						// The time to check came for an already removed library, so we just ignore it
						to_remove.remove(&key);
					} else if let Some(location) = get_location(location_id, &library).await {
						// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
						if location.instance_id == Some(library.config.instance_id) {
							let is_online = match check_online(&location, &node, &library).await {
								Ok(is_online) => is_online,
								Err(e) => {
									error!("Error while checking online status of location {location_id}: {e}");
									continue;
								}
							};

							if is_online
								&& !forced_unwatch.contains(&key)
							{
								watch_location(
									location,
									library.id,
									&mut locations_watched,
									&mut locations_unwatched,
								);
							} else {
								unwatch_location(
									location,
									library.id,
									&mut locations_watched,
									&mut locations_unwatched,
								);
							}
							to_check_futures.push(location_check_sleep(location_id, library));
						} else {
							drop_location(
								location_id,
								library.id,
								"Dropping location from location manager, because \
								it isn't a location in the current node",
								&mut locations_watched,
								&mut locations_unwatched
							);
							forced_unwatch.remove(&key);
						}
					} else {
						drop_location(
							location_id,
							library.id,
							"Removing location from manager, as we failed to fetch from db",
							&mut locations_watched,
							&mut locations_unwatched,
						);
						forced_unwatch.remove(&key);
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

	pub async fn is_online(&self, id: &Uuid) -> bool {
		let online_locations = self.online_locations.read().await;
		online_locations.iter().any(|v| v == id.as_bytes())
	}

	pub async fn get_online(&self) -> OnlineLocations {
		self.online_locations.read().await.clone()
	}

	async fn broadcast_online(&self) {
		self.online_tx.send(self.get_online().await).ok();
	}

	pub async fn add_online(&self, id: Uuid) {
		{
			self.online_locations
				.write()
				.await
				.insert(id.as_bytes().to_vec());
		}
		self.broadcast_online().await;
	}

	pub async fn remove_online(&self, id: &Uuid) {
		{
			let mut online_locations = self.online_locations.write().await;
			online_locations.retain(|v| v != id.as_bytes());
		}
		self.broadcast_online().await;
	}

	pub fn online_rx(&self) -> Receiver<OnlineLocations> {
		self.online_tx.subscribe()
	}
}

impl Drop for Locations {
	fn drop(&mut self) {
		if let Some(stop_tx) = self.stop_tx.take() {
			if stop_tx.send(()).is_err() {
				error!("Failed to send stop signal to location manager");
			}
		}
	}
}

#[must_use = "this `StopWatcherGuard` must be held for some time, so the watcher is stopped"]
pub struct StopWatcherGuard<'m> {
	manager: &'m Locations,
	location_id: location::id::Type,
	library: Option<Arc<Library>>,
}

impl Drop for StopWatcherGuard<'_> {
	fn drop(&mut self) {
		if cfg!(feature = "location-watcher") {
			// FIXME: change this Drop to async drop in the future
			if let Err(e) = block_on(self.manager.reinit_watcher(
				self.location_id,
				self.library.take().expect("library should be set"),
			)) {
				error!("Failed to reinit watcher on stop watcher guard drop: {e}");
			}
		}
	}
}

#[must_use = "this `IgnoreEventsForPathGuard` must be held for some time, so the watcher can ignore events for the desired path"]
pub struct IgnoreEventsForPathGuard<'m> {
	manager: &'m Locations,
	path: Option<PathBuf>,
	location_id: location::id::Type,
	library: Option<Arc<Library>>,
}

impl Drop for IgnoreEventsForPathGuard<'_> {
	fn drop(&mut self) {
		if cfg!(feature = "location-watcher") {
			// FIXME: change this Drop to async drop in the future
			if let Err(e) = block_on(self.manager.watcher_management_message(
				self.location_id,
				self.library.take().expect("library should be set"),
				WatcherManagementMessageAction::IgnoreEventsForPath {
					path: self.path.take().expect("path should be set"),
					ignore: false,
				},
			)) {
				error!("Failed to un-ignore path on watcher guard drop: {e}");
			}
		}
	}
}
