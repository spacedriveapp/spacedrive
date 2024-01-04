use crate::{
	job::JobManagerError,
	library::{Library, LibraryManagerEvent},
	prisma::location,
	util::{db::MissingFieldError, error::FileIOError},
	Node,
};

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use thiserror::Error;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info};

use super::{
	file_path_helper::FilePathError,
	get_location_path_from_location_id,
	manager::{LocationManagementMessage, WatcherManagementMessage},
};
type OnlineLocations = BTreeSet<Vec<u8>>;

mod android_inotify;

pub struct LocationManagerActor {
	// #[cfg(feature = "location-watcher")]
	location_management_rx: mpsc::Receiver<LocationManagementMessage>,
	// #[cfg(feature = "location-watcher")]
	watcher_management_rx: mpsc::Receiver<WatcherManagementMessage>,
	// #[cfg(feature = "location-watcher")]
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
										node.locations.add(location.id, library.clone()).await;
									}
								}
								LibraryManagerEvent::Edit(_) => {}
								LibraryManagerEvent::InstancesModified(_) => {}
								LibraryManagerEvent::Delete(_) => {
									#[cfg(debug_assertions)]
									error!("TODO: Remove locations from location manager"); // TODO
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
	}
}

pub struct Locations {
	online_locations: RwLock<OnlineLocations>,
	android_watcher: inotify::Inotify,
}

impl Locations {
	pub fn new() -> (Self, LocationManagerActor) {
		let android_watcher = android_inotify::init();
		let (_, location_management_rx) = mpsc::channel(128);
		let (_, watcher_management_rx) = mpsc::channel(128);
		let (_, stop_rx) = oneshot::channel();

		(
			Self {
				online_locations: Default::default(),
				android_watcher,
			},
			LocationManagerActor {
				location_management_rx,
				watcher_management_rx,
				stop_rx,
			},
		)
	}

	pub async fn add(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), AndroidLocationManagerError> {
		let _path = get_location_path_from_location_id(&library.db, location_id)
			.await
			.map_err(|_| AndroidLocationManagerError::MissingLocation(location_id))?;

		let directory_path = _path.to_str().ok_or(AndroidLocationManagerError::FilePath(
			FilePathError::LocationNotFound(location_id),
		))?;

		let inotify = &self.android_watcher;

		android_inotify::add_watcher(inotify, directory_path)
			.map_err(|err| AndroidLocationManagerError::WatcherError(err))?;

		Ok(())
	}

	pub async fn remove(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), AndroidLocationManagerError> {
		let _path = get_location_path_from_location_id(&library.db, location_id)
			.await
			.map_err(|_| AndroidLocationManagerError::MissingLocation(location_id))?;

		let directory_path = _path.to_str().ok_or(AndroidLocationManagerError::FilePath(
			FilePathError::LocationNotFound(location_id),
		))?;

		let inotify = &self.android_watcher;

		android_inotify::remove_watcher(inotify, directory_path)
			.map_err(|err| AndroidLocationManagerError::WatcherError(err))?;

		Ok(())
	}
}

#[derive(Error, Debug)]
pub enum AndroidLocationManagerError {
	// #[cfg(feature = "location-watcher")]
	#[error("Watcher error: (error: {0})")]
	WatcherError(#[from] android_inotify::AndroidWatcherError),

	#[error("Location not found: (location_id: {0})")]
	MissingLocation(location::id::Type),

	#[error("File path related error (error: {0})")]
	FilePath(#[from] FilePathError),
}
