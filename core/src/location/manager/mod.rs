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

type ManagerMessage = (
	LocationId,
	LibraryContext,
	oneshot::Sender<Result<(), LocationManagerError>>,
);

type IgnorePathManagerMessage = (
	LocationId,
	LibraryContext,
	PathBuf,
	bool,
	oneshot::Sender<Result<(), LocationManagerError>>,
);

#[derive(Error, Debug)]
pub enum LocationManagerError {
	#[error("Unable to send location id to be checked by actor: (error: {0})")]
	ActorSendLocationError(#[from] mpsc::error::SendError<ManagerMessage>),

	#[cfg(feature = "location-watcher")]
	#[error("Unable to send path to be ignored by watcher actor: (error: {0})")]
	ActorIgnorePathError(#[from] mpsc::error::SendError<watcher::IgnorePath>),

	#[cfg(feature = "location-watcher")]
	#[error(
		"Unable to send path to be ignored by watcher on location manager actor: (error: {0})"
	)]
	ActorIgnorePathMessageError(#[from] mpsc::error::SendError<IgnorePathManagerMessage>),

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
	add_locations_tx: mpsc::Sender<ManagerMessage>,
	remove_locations_tx: mpsc::Sender<ManagerMessage>,
	stop_watcher_tx: mpsc::Sender<ManagerMessage>,
	reinit_watcher_tx: mpsc::Sender<ManagerMessage>,
	ignore_path_tx: mpsc::Sender<IgnorePathManagerMessage>,
	stop_tx: Option<oneshot::Sender<()>>,
}

impl LocationManager {
	#[allow(unused)]
	pub fn new() -> Arc<Self> {
		let (add_locations_tx, add_locations_rx) = mpsc::channel(128);
		let (remove_locations_tx, remove_locations_rx) = mpsc::channel(128);
		let (stop_watcher_tx, stop_watcher_rx) = mpsc::channel(128);
		let (reinit_watcher_tx, reinit_watcher_rx) = mpsc::channel(128);
		let (ignore_path_tx, ignore_path_rx) = mpsc::channel(128);
		let (stop_tx, stop_rx) = oneshot::channel();

		#[cfg(feature = "location-watcher")]
		tokio::spawn(Self::run_locations_checker(
			AddAndRemoveLocation {
				add_rx: add_locations_rx,
				remove_rx: remove_locations_rx,
			},
			StopAndReinitWatcher {
				stop_rx: stop_watcher_rx,
				reinit_rx: reinit_watcher_rx,
			},
			ignore_path_rx,
			stop_rx,
		));

		#[cfg(not(feature = "location-watcher"))]
		tracing::warn!("Location watcher is disabled, locations will not be checked");

		debug!("Location manager initialized");

		Arc::new(Self {
			add_locations_tx,
			remove_locations_tx,
			stop_watcher_tx,
			reinit_watcher_tx,
			ignore_path_tx,
			stop_tx: Some(stop_tx),
		})
	}

	pub async fn add(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.add_locations_tx
				.send((location_id, library_ctx, tx))
				.await?;

			rx.await?
		} else {
			Ok(())
		}
	}

	pub async fn remove(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.remove_locations_tx
				.send((location_id, library_ctx, tx))
				.await?;

			rx.await?
		} else {
			Ok(())
		}
	}

	pub async fn stop_watcher(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.stop_watcher_tx
				.send((location_id, library_ctx, tx))
				.await?;

			rx.await?
		} else {
			Ok(())
		}
	}

	pub async fn reinit_watcher(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<(), LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.reinit_watcher_tx
				.send((location_id, library_ctx, tx))
				.await?;

			rx.await?
		} else {
			Ok(())
		}
	}

	pub async fn temporary_stop(
		&self,
		location_id: LocationId,
		library_ctx: LibraryContext,
	) -> Result<StopWatcherGuard, LocationManagerError> {
		if cfg!(feature = "location-watcher") {
			self.stop_watcher(location_id, library_ctx.clone()).await?;
		}

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
		if cfg!(feature = "location-watcher") {
			let (tx, rx) = oneshot::channel();

			self.ignore_path_tx
				.send((location_id, library_ctx.clone(), path.clone(), true, tx))
				.await?;

			rx.await??;
		}

		Ok(IgnoreEventsForPathGuard {
			location_id,
			library_ctx: Some(library_ctx),
			manager: self,
			path: Some(path),
		})
	}

	#[cfg(feature = "location-watcher")]
	async fn run_locations_checker(
		mut location: AddAndRemoveLocation,
		mut watcher: StopAndReinitWatcher,
		mut ignore_path_rx: mpsc::Receiver<IgnorePathManagerMessage>,
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
				// To add a new location
				Some((location_id, library_ctx, response_tx)) = location.add_rx.recv() => {
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
				}

				// To remove an location
				Some(message) = location.remove_rx.recv() => {
					handle_remove_location_request(
						message,
						&mut forced_unwatch,
						&mut locations_watched,
						&mut locations_unwatched,
						&mut to_remove,
					).await;
				}

				// To stop a watcher
				Some(message) = watcher.stop_rx.recv() => {
					handle_stop_watcher_request(
						message,
						&mut forced_unwatch,
						&mut locations_watched,
						&mut locations_unwatched,
					).await;
				}

				// To reinit a stopped watcher
				Some(message) = watcher.reinit_rx.recv() => {
					handle_reinit_watcher_request(
						message,
						&mut forced_unwatch,
						&mut locations_watched,
						&mut locations_unwatched,
					).await;
				}

				// To ignore or not events for a path
				Some(message) = ignore_path_rx.recv() => {
					handle_ignore_path_request(message, &locations_watched);
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

#[cfg(feature = "location-watcher")]
struct AddAndRemoveLocation {
	add_rx: mpsc::Receiver<ManagerMessage>,
	remove_rx: mpsc::Receiver<ManagerMessage>,
}

#[cfg(feature = "location-watcher")]
struct StopAndReinitWatcher {
	stop_rx: mpsc::Receiver<ManagerMessage>,
	reinit_rx: mpsc::Receiver<ManagerMessage>,
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
			let (tx, rx) = oneshot::channel();
			if self
				.manager
				.ignore_path_tx
				.blocking_send((
					self.location_id,
					self.library_ctx.take().unwrap(),
					self.path.take().unwrap(),
					false,
					tx,
				))
				.is_err()
			{
				error!("Failed to send un-ignore path request to location manager on ignore events for path watcher guard drop");
			} else if let Err(e) = rx.blocking_recv() {
				error!("Failed to un-ignore path on watcher guard drop: {e}");
			}
		}
	}
}
