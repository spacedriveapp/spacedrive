use crate::{library::LibraryContext, prisma::location};

use std::path::Path;
use std::{collections::HashSet, time::Duration};

use futures::{stream::FuturesUnordered, StreamExt};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use thiserror::Error;
use tokio::{
	fs, io, select,
	sync::{mpsc, oneshot},
	time::sleep,
};
use tracing::{error, info, warn};

static LOCATION_MANAGER: OnceCell<LocationManager> = OnceCell::new();
const LOCATION_CHECK_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Error, Debug)]
pub enum LocationManagerError {
	#[error("Tried to call new method on an already initialized location manager")]
	AlreadyInitialized,
	#[error("Unable to send location id to be checked by actor: (error: {0})")]
	ActorSendAddLocationError(#[from] mpsc::error::SendError<(i32, LibraryContext)>),
	#[error("Unable to send location id to be removed from actor: (error: {0})")]
	ActorSendRemoveLocationError(#[from] mpsc::error::SendError<i32>),
	#[error("Watcher error: (error: {0})")]
	WatcherError(#[from] notify::Error),
}

#[derive(Debug)]
pub struct LocationManager {
	add_locations_tx: mpsc::Sender<(i32, LibraryContext)>,
	remove_locations_tx: mpsc::Sender<i32>,
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

	pub async fn remove(&self, location_id: i32) -> Result<(), LocationManagerError> {
		self.remove_locations_tx
			.send(location_id)
			.await
			.map_err(Into::into)
	}

	async fn run_locations_checker(
		mut add_locations_rx: mpsc::Receiver<(i32, LibraryContext)>,
		mut remove_locations_rx: mpsc::Receiver<i32>,
		mut stop_rx: oneshot::Receiver<()>,
	) -> Result<(), LocationManagerError> {
		let mut to_check_futures = FuturesUnordered::new();
		let mut to_remove = HashSet::new();

		let (events_tx, events_rx) = mpsc::unbounded_channel();

		let mut watcher = RecommendedWatcher::new(
			move |result| {
				if events_tx.send(result).is_err() {
					error!("Unable to send watcher event to location manager");
				}
			},
			Config::default(),
		)?;

		tokio::spawn(handle_watch_events(events_rx));

		loop {
			select! {
				Some((location_id, library_ctx)) = add_locations_rx.recv() => {
					if let Some(location) = get_location(location_id, &library_ctx).await {
						if check_online(&location, &library_ctx).await {
							// SAFETY:: This unwrap is ok because we check if we have a `local_path`
							// on `check_online` function above
							let local_path = Path::new(location.local_path.as_ref().unwrap());
							if let Err(e) = watcher.watch(local_path, RecursiveMode::Recursive) {
								error!(
									"Unable to watch location: (path: {}, error: {e:#?}) ",
									local_path.display()
								);
							}
						}

						to_check_futures.push(location_check_sleep(location_id, library_ctx));
					}
				}

				Some(location_id) = remove_locations_rx.recv() => {
					to_remove.insert(location_id);
				}

				Some((location_id, library_ctx)) = to_check_futures.next() => {
					if let Some(location) = get_location(location_id, &library_ctx).await {
						if let Some(ref local_path) = location.local_path {
							let local_path = Path::new(local_path);
							if to_remove.contains(&location_id) {
								to_remove.remove(&location_id);
								if let Err(e) = watcher.unwatch(local_path) {
									error!(
										"Unable to unwatch location: (path: {}, error: {e:#?})",
										local_path.display()
									);
								}
							} else {
								if check_online(&location, &library_ctx).await {
									if let Err(e) = watcher.watch(local_path, RecursiveMode::Recursive) {
										error!(
											"Unable to watch location: (path: {}, error: {e:#?})",
											local_path.display()
										);
									}
								} else if let Err(e) = watcher.unwatch(local_path) {
									error!(
										"Unable to unwatch location: (path: {}, error: {e:#?})"
										, local_path.display()
									);
								}
								to_check_futures.push(location_check_sleep(location_id, library_ctx));
							}
						} else {
							warn!("Dropping location from location manager, but leaking watchers
							 because we don't have a `local_path` to unwatch: 
							 (location_id: {location_id}, library_id: {})", library_ctx.id);
						}
					}  else {
						warn!("Dropping location from location manager, but leaking watchers because
						 we weren't able to fetch from db: 
						 (location_id: {location_id}, library_id: {})", library_ctx.id);
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

async fn handle_watch_events(mut events_rx: mpsc::UnboundedReceiver<notify::Result<Event>>) {
	while let Some(event) = events_rx.recv().await {
		match event {
			Ok(event) => {
				info!("Received event: {:#?}", event);
			}
			Err(e) => {
				error!("watch error: {:#?}", e);
			}
		}
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
