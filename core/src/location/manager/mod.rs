use crate::library::LibraryContext;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::executor::block_on;
use thiserror::Error;
use tokio::{
	io,
	sync::{mpsc, oneshot},
};
use tracing::{debug, error};

#[cfg(feature = "location-watcher")]
mod watcher;

#[cfg(feature = "location-watcher")]
mod helpers;

pub type LocationId = i32;

#[derive(Clone, Copy, Debug)]
enum ManagementMessageAction {
	Add,
	Remove,
}

#[derive(Debug)]
pub struct LocationManagementMessage {
	location_id: LocationId,
	library_ctx: LibraryContext,
	action: ManagementMessageAction,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
}

#[derive(Debug)]
enum WatcherManagementMessageAction {
	Stop,
	Reinit,
	IgnoreEventsForPath { path: PathBuf, ignore: bool },
}

#[derive(Debug)]
pub struct WatcherManagementMessage {
	location_id: LocationId,
	library_ctx: LibraryContext,
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

	#[error("Location missing local path: <id='{0}'>")]
	LocationMissingLocalPath(LocationId),
	#[error("Tried to update a non-existing file: <path='{0}'>")]
	UpdateNonExistingFile(PathBuf),
	#[error("Unable to extract materialized path from location: <id='{0}', path='{1:?}'>")]
	UnableToExtractMaterializedPath(LocationId, PathBuf),
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("I/O error: {0}")]
	IOError(#[from] io::Error),
}

#[derive(Debug)]
pub struct LocationManager {
	location_management_tx: mpsc::Sender<LocationManagementMessage>,
	watcher_management_tx: mpsc::Sender<WatcherManagementMessage>,
	stop_tx: Option<oneshot::Sender<()>>,
}

impl LocationManager {
	#[allow(unused)]
	pub fn new() -> Arc<Self> {
		let (location_management_tx, location_management_rx) = mpsc::channel(128);
		let (watcher_management_tx, watcher_management_rx) = mpsc::channel(128);
		let (stop_tx, stop_rx) = oneshot::channel();

		#[cfg(feature = "location-watcher")]
		tokio::spawn(Self::run_locations_checker(
			location_management_rx,
			watcher_management_rx,
			stop_rx,
		));

		#[cfg(not(feature = "location-watcher"))]
		tracing::warn!("Location watcher is disabled, locations will not be checked");

		debug!("Location manager initialized");

		Arc::new(Self {
			location_management_tx,
			watcher_management_tx,
			stop_tx: Some(stop_tx),
		})
	}

	#[inline]
	async fn location_management_message(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
		action: ManagementMessageAction,
	) -> Result<(), LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.location_management_tx
				.send(LocationManagementMessage {
					location_id,
					library_ctx,
					action,
					response_tx: tx,
				})
				.await?;

			rx.await?
		} else {
			Ok(())
		}
	}

	#[inline]
	async fn watcher_management_message(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
		action: WatcherManagementMessageAction,
	) -> Result<(), LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.watcher_management_tx
				.send(WatcherManagementMessage {
					location_id,
					library_ctx,
					action,
					response_tx: tx,
				})
				.await?;

			rx.await?
		} else {
			Ok(())
		}
	}

	pub async fn add(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		self.location_management_message(location_id, library_ctx, ManagementMessageAction::Add)
			.await
	}

	pub async fn remove(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		self.location_management_message(location_id, library_ctx, ManagementMessageAction::Remove)
			.await
	}

	pub async fn stop_watcher(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		self.watcher_management_message(
			location_id,
			library_ctx,
			WatcherManagementMessageAction::Stop,
		)
		.await
	}

	pub async fn reinit_watcher(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		self.watcher_management_message(
			location_id,
			library_ctx,
			WatcherManagementMessageAction::Reinit,
		)
		.await
	}

	pub async fn temporary_stop(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<StopWatcherGuard, LocationManagerError> {
		self.stop_watcher(location_id, library_ctx.clone()).await?;

		Ok(StopWatcherGuard {
			location_id,
			library_ctx: Some(library_ctx),
			manager: self,
		})
	}

	pub async fn temporary_ignore_events_for_path(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
		path: impl AsRef<Path>,
	) -> Result<IgnoreEventsForPathGuard, LocationManagerError> {
		let path = path.as_ref().to_path_buf();

		self.watcher_management_message(
			location_id,
			library_ctx.clone(),
			WatcherManagementMessageAction::IgnoreEventsForPath {
				path: path.clone(),
				ignore: true,
			},
		)
		.await?;

		Ok(IgnoreEventsForPathGuard {
			location_id,
			library_ctx: Some(library_ctx),
			manager: self,
			path: Some(path),
		})
	}

	#[cfg(feature = "location-watcher")]
	async fn run_locations_checker(
		mut location_management_rx: mpsc::Receiver<LocationManagementMessage>,
		mut watcher_management_rx: mpsc::Receiver<WatcherManagementMessage>,
		mut stop_rx: oneshot::Receiver<()>,
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
					library_ctx,
					action,
					response_tx
				}) = location_management_rx.recv() => {
					match action {

						// To add a new location
						ManagementMessageAction::Add => {
							if let Some(location) = get_location(location_id, &library_ctx).await {
								let is_online = check_online(&location, &library_ctx).await;
								let _ = response_tx.send(
									LocationWatcher::new(location, library_ctx.clone())
										.await
										.map(|mut watcher| {
											if is_online {
												watcher.watch();
												locations_watched.insert(
													(location_id, library_ctx.id),
													watcher
												);
											} else {
												locations_unwatched.insert(
													(location_id, library_ctx.id),
													watcher
												);
											}

											to_check_futures.push(
												location_check_sleep(location_id, library_ctx)
											);
										}
									)
								); // ignore errors, we handle errors on receiver
							} else {
								warn!(
									"Location not found in database to be watched: {}",
									location_id
								);
							}
						},

						// To remove an location
						ManagementMessageAction::Remove => {
							handle_remove_location_request(
								location_id,
								library_ctx,
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
					library_ctx,
					action,
					response_tx,
				}) = watcher_management_rx.recv() => {
					match action {
						// To stop a watcher
						WatcherManagementMessageAction::Stop => {
							handle_stop_watcher_request(
								location_id,
								library_ctx,
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
								library_ctx,
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
								library_ctx,
								path,
								ignore,
								response_tx,
								&locations_watched,
							);
						},
					}
				}

				// Periodically checking locations
				Some((location_id, library_ctx)) = to_check_futures.next() => {
					let key = (location_id, library_ctx.id);

					if to_remove.contains(&key) {
						// The time to check came for an already removed library, so we just ignore it
						to_remove.remove(&key);
					} else if let Some(location) = get_location(location_id, &library_ctx).await {
						if let Some(ref local_path_str) = location.local_path.clone() {
							if check_online(&location, &library_ctx).await
								&& !forced_unwatch.contains(&key)
							{
								watch_location(
									location,
									library_ctx.id,
									local_path_str,
									&mut locations_watched,
									&mut locations_unwatched,
								);
							} else {
								unwatch_location(
									location,
									library_ctx.id,
									local_path_str,
									&mut locations_watched,
									&mut locations_unwatched,
								);
							}
							to_check_futures.push(location_check_sleep(location_id, library_ctx));
						} else {
							drop_location(
								location_id,
								library_ctx.id,
								"Dropping location from location manager, because \
								we don't have a `local_path` anymore",
								&mut locations_watched,
								&mut locations_unwatched
							);
							forced_unwatch.remove(&key);
						}
					} else {
						drop_location(
							location_id,
							library_ctx.id,
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

#[must_use = "this `StopWatcherGuard` must be held for some time, so the watcher is stopped"]
pub struct StopWatcherGuard<'m> {
	manager: &'m LocationManager,
	location_id: LocationId,
	library_ctx: Option<LibraryContext>,
}

impl Drop for StopWatcherGuard<'_> {
	fn drop(&mut self) {
		if cfg!(feature = "location-watcher") {
			// FIXME: change this Drop to async drop in the future
			if let Err(e) = block_on(
				self.manager
					.reinit_watcher(self.location_id, self.library_ctx.take().unwrap()),
			) {
				error!("Failed to reinit watcher on stop watcher guard drop: {e}");
			}
		}
	}
}

#[must_use = "this `IgnoreEventsForPathGuard` must be held for some time, so the watcher can ignore events for the desired path"]
pub struct IgnoreEventsForPathGuard<'m> {
	manager: &'m LocationManager,
	path: Option<PathBuf>,
	location_id: LocationId,
	library_ctx: Option<LibraryContext>,
}

impl Drop for IgnoreEventsForPathGuard<'_> {
	fn drop(&mut self) {
		if cfg!(feature = "location-watcher") {
			// FIXME: change this Drop to async drop in the future
			if let Err(e) = block_on(self.manager.watcher_management_message(
				self.location_id,
				self.library_ctx.take().unwrap(),
				WatcherManagementMessageAction::IgnoreEventsForPath {
					path: self.path.take().unwrap(),
					ignore: false,
				},
			)) {
				error!("Failed to un-ignore path on watcher guard drop: {e}");
			}
		}
	}
}
