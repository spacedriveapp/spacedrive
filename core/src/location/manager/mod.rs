use crate::library::LibraryContext;

use std::{path::PathBuf, sync::Arc};

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

#[derive(Error, Debug)]
pub enum LocationManagerError {
	#[error("Tried to call new method on an already initialized location manager")]
	AlreadyInitialized,

	#[error("Unable to send location id to be checked by actor: (error: {0})")]
	ActorSendLocationError(#[from] mpsc::error::SendError<ManagerMessage>),
	#[error("Unable to receive actor response: (error: {0})")]
	ActorResponseError(#[from] oneshot::error::RecvError),

	#[cfg(feature = "location-watcher")]
	#[error("Watcher error: (error: {0})")]
	WatcherError(#[from] notify::Error),

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
	stop_tx: Option<oneshot::Sender<()>>,
}

impl LocationManager {
	#[allow(unused)]
	pub async fn new() -> Result<Arc<Self>, LocationManagerError> {
		let (add_locations_tx, add_locations_rx) = mpsc::channel(128);
		let (remove_locations_tx, remove_locations_rx) = mpsc::channel(128);
		let (stop_tx, stop_rx) = oneshot::channel();

		#[cfg(feature = "location-watcher")]
		tokio::spawn(Self::run_locations_checker(
			add_locations_rx,
			remove_locations_rx,
			stop_rx,
		));

		#[cfg(not(feature = "location-watcher"))]
		tracing::warn!("Location watcher is disabled, locations will not be checked");

		debug!("Location manager initialized");

		Ok(Arc::new(Self {
			add_locations_tx,
			remove_locations_tx,
			stop_tx: Some(stop_tx),
		}))
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

	#[cfg(feature = "location-watcher")]
	async fn run_locations_checker(
		mut add_locations_rx: mpsc::Receiver<ManagerMessage>,
		mut remove_locations_rx: mpsc::Receiver<ManagerMessage>,
		mut stop_rx: oneshot::Receiver<()>,
	) -> Result<(), LocationManagerError> {
		use std::collections::{HashMap, HashSet};

		use futures::stream::{FuturesUnordered, StreamExt};
		use tokio::select;
		use tracing::{info, warn};

		use helpers::{
			check_online, drop_location, get_location, location_check_sleep, unwatch_location,
			watch_location,
		};
		use watcher::LocationWatcher;

		let mut to_check_futures = FuturesUnordered::new();
		let mut to_remove = HashSet::new();
		let mut locations_watched = HashMap::new();
		let mut locations_unwatched = HashMap::new();

		loop {
			select! {
				// To add a new location
				Some((location_id, library_ctx, response_tx)) = add_locations_rx.recv() => {
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
				Some((location_id, library_ctx, response_tx)) = remove_locations_rx.recv() => {
					if let Some(location) = get_location(location_id, &library_ctx).await {
						if let Some(ref local_path_str) = location.local_path.clone() {
							unwatch_location(
								location,
								library_ctx.id,
								local_path_str,
								&mut locations_watched,
								&mut locations_unwatched,
							);
							locations_unwatched.remove(&(location_id, library_ctx.id));
						} else {
							drop_location(
								location_id,
								library_ctx.id,
								"Dropping location from location manager, because we don't have a `local_path` anymore",
								&mut locations_watched,
								&mut locations_unwatched
							);
						}
					} else {
						drop_location(
							location_id,
							library_ctx.id,
							"Removing location from manager, as we failed to fetch from db",
							&mut locations_watched,
							&mut locations_unwatched
						);
					}

					// Marking location as removed, so we don't try to check it when the time comes
					to_remove.insert((location_id, library_ctx.id));

					let _ = response_tx.send(Ok(())); // ignore errors, we handle errors on receiver
				}

				// Periodically checking locations
				Some((location_id, library_ctx)) = to_check_futures.next() => {
					if to_remove.contains(&(location_id, library_ctx.id)) {
						// The time to check came for an already removed library, so we just ignore it
						to_remove.remove(&(location_id, library_ctx.id));
					} else if let Some(location) = get_location(location_id, &library_ctx).await {
						if let Some(ref local_path_str) = location.local_path.clone() {
							if check_online(&location, &library_ctx).await {
								watch_location(
									location,
									library_ctx.id,
									local_path_str,
									&mut locations_watched,
									&mut locations_unwatched
								);
							} else {
								unwatch_location(
									location,
									library_ctx.id,
									local_path_str,
									&mut locations_watched,
									&mut locations_unwatched
								);
							}
							to_check_futures.push(location_check_sleep(location_id, library_ctx));
						} else {
							drop_location(
								location_id,
								library_ctx.id,
								"Dropping location from location manager, because we don't have a `local_path` anymore",
								&mut locations_watched,
								&mut locations_unwatched
							);
						}
					} else {
						drop_location(
							location_id,
							library_ctx.id,
							"Removing location from manager, as we failed to fetch from db",
							&mut locations_watched,
							&mut locations_unwatched
						);
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
