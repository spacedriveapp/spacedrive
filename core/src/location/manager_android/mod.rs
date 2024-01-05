use crate::{
	library::{Library, LibraryManagerEvent},
	prisma::location,
	Node,
};

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use thiserror::Error;
use tokio::sync::{
	broadcast::{self, Receiver},
	mpsc, oneshot, RwLock,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::{file_path_helper::FilePathError, get_location_path_from_location_id};
type OnlineLocations = BTreeSet<Vec<u8>>;

mod android_inotify;

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
	response_tx: oneshot::Sender<Result<(), AndroidLocationManagerError>>,
}

#[derive(Clone, Debug)]
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
	response_tx: oneshot::Sender<Result<(), AndroidLocationManagerError>>,
}

#[must_use = "'AndroidLocationManagerActor::start' must be used to start the actor"]
pub struct AndroidLocationManagerActor {
	location_management_rx: mpsc::Receiver<LocationManagementMessage>,
	watcher_management_rx: mpsc::Receiver<WatcherManagementMessage>,
	stop_rx: oneshot::Receiver<()>,
}

impl AndroidLocationManagerActor {
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
										node.android_locations
											.add(location.id, library.clone())
											.await.expect("Failed to add location");
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

pub struct AndroidLocations {
	online_locations: RwLock<OnlineLocations>,
	pub online_tx: broadcast::Sender<OnlineLocations>,
	android_watcher: inotify::Inotify,
}

impl AndroidLocations {
	pub fn new() -> (Self, AndroidLocationManagerActor) {
		let android_watcher = android_inotify::init();
		let online_tx = broadcast::channel(16).0;
		let (_, location_management_rx) = mpsc::channel(128);
		let (_, watcher_management_rx) = mpsc::channel(128);
		let (_, stop_rx) = oneshot::channel();

		(
			Self {
				online_locations: Default::default(),
				online_tx,
				android_watcher,
			},
			AndroidLocationManagerActor {
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
			.await
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
			.await
			.map_err(|err| AndroidLocationManagerError::WatcherError(err))?;

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
