use crate::{
	library::{Library, LibraryManagerEvent},
	Node,
};

use sd_core_file_path_helper::FilePathError;

use sd_prisma::prisma::location;
use sd_utils::{db::MissingFieldError, error::FileIOError};

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use async_channel as chan;
use futures::executor::block_on;
use thiserror::Error;
use tokio::{
	spawn,
	sync::{
		broadcast::{self, Receiver},
		oneshot, RwLock,
	},
};
use tracing::{debug, error, instrument, trace};
use uuid::Uuid;

mod runner;
mod watcher;

#[derive(Clone, Copy, Debug)]
enum ManagementMessageAction {
	Add,
	Remove,
}

#[derive(Debug)]
pub struct LocationManagementMessage {
	location_id: location::id::Type,
	library: Arc<Library>,
	action: ManagementMessageAction,
	ack: oneshot::Sender<Result<(), LocationManagerError>>,
}

#[derive(Debug)]
enum WatcherManagementMessageAction {
	Pause,
	Resume,
	IgnoreEventsForPath { path: PathBuf, ignore: bool },
}

#[derive(Debug)]
pub struct WatcherManagementMessage {
	location_id: location::id::Type,
	library: Arc<Library>,
	action: WatcherManagementMessageAction,
	ack: oneshot::Sender<Result<(), LocationManagerError>>,
}

#[derive(Error, Debug)]
pub enum LocationManagerError {
	#[error("location not found in database: <id={0}>")]
	LocationNotFound(location::id::Type),

	#[error("watcher error: {0}")]
	Watcher(#[from] notify::Error),

	#[error("non local location: <id='{0}'>")]
	NonLocalLocation(location::id::Type),

	#[error("file still exists on disk after remove event received: <path='{}'>", .0.display())]
	FileStillExistsOnDisk(Box<Path>),

	#[error("failed to move file '{}' for reason: {reason}", .path.display())]
	MoveError {
		path: Box<Path>,
		reason: &'static str,
	},

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("corrupted location pub_id on database: {0}")]
	CorruptedLocationPubId(#[from] uuid::Error),
	#[error("missing field: {0}")]
	MissingField(#[from] MissingFieldError),

	#[error(transparent)]
	FilePath(#[from] FilePathError),
	#[error(transparent)]
	IndexerRuler(#[from] sd_core_indexer_rules::Error),
	#[error(transparent)]
	JobSystem(#[from] sd_core_heavy_lifting::Error),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),
}

type OnlineLocations = BTreeSet<Vec<u8>>;

#[must_use = "'LocationManagerActor::start' must be used to start the actor"]
pub struct LocationManagerActor {
	location_management_rx: chan::Receiver<LocationManagementMessage>,
	watcher_management_rx: chan::Receiver<WatcherManagementMessage>,
	stop_rx: chan::Receiver<()>,
}

impl LocationManagerActor {
	pub fn start(self, node: Arc<Node>) {
		spawn({
			let node = node.clone();
			let rx = node.libraries.rx.clone();
			async move {
				if let Err(e) = rx
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
												?e,
												"Failed to get locations from database for location manager;",
											);

											vec![]
										}) {
										if let Err(e) =
											node.locations.add(location.id, library.clone()).await
										{
											error!(
												?e,
												"Failed to add location to location manager;",
											);
										}
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
					error!(
						?e,
						"Core may become unstable! LocationManager's \
						library manager subscription aborted with error;",
					);
				}
			}
		});

		spawn({
			let node = Arc::clone(&node);
			let Self {
				location_management_rx,
				watcher_management_rx,
				stop_rx,
			} = self;

			async move {
				while let Err(e) = spawn({
					runner::run(
						location_management_rx.clone(),
						watcher_management_rx.clone(),
						stop_rx.clone(),
						Arc::clone(&node),
					)
				})
				.await
				{
					if e.is_panic() {
						error!(?e, "Location manager panicked;");
					} else {
						trace!("Location manager received shutdown signal and will exit...");
						break;
					}
					trace!("Restarting location manager processing task...");
				}

				debug!("Location manager gracefully shutdown");
			}
		});
	}
}

pub struct Locations {
	online_locations: RwLock<OnlineLocations>,
	pub online_tx: broadcast::Sender<OnlineLocations>,

	location_management_tx: chan::Sender<LocationManagementMessage>,

	watcher_management_tx: chan::Sender<WatcherManagementMessage>,
	stop_tx: chan::Sender<()>,
}

impl Locations {
	pub fn new() -> (Self, LocationManagerActor) {
		let (location_management_tx, location_management_rx) = chan::bounded(128);
		let (watcher_management_tx, watcher_management_rx) = chan::bounded(128);
		let (stop_tx, stop_rx) = chan::bounded(1);

		debug!("Starting location manager actor");

		(
			Self {
				online_locations: Default::default(),
				online_tx: broadcast::channel(16).0,
				location_management_tx,
				watcher_management_tx,
				stop_tx,
			},
			LocationManagerActor {
				location_management_rx,
				watcher_management_rx,
				stop_rx,
			},
		)
	}

	#[instrument(skip(self, library), fields(library_id = %library.id), err)]
	#[inline]
	async fn location_management_message(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		action: ManagementMessageAction,
	) -> Result<(), LocationManagerError> {
		let (tx, rx) = oneshot::channel();
		trace!("Sending location management message to location manager actor");

		self.location_management_tx
			.send(LocationManagementMessage {
				location_id,
				library,
				action,
				ack: tx,
			})
			.await
			.expect("Location manager actor channel closed sending new location message");

		rx.await
			.expect("Ack channel closed for location management message response")
	}

	#[instrument(skip(self, library), fields(library_id = %library.id), err)]
	#[inline]
	#[allow(unused_variables)]
	async fn watcher_management_message(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		action: WatcherManagementMessageAction,
	) -> Result<(), LocationManagerError> {
		let (tx, rx) = oneshot::channel();
		trace!("Sending watcher management message to location manager actor");

		self.watcher_management_tx
			.send(WatcherManagementMessage {
				location_id,
				library,
				action,
				ack: tx,
			})
			.await
			.expect("Location manager actor channel closed sending new watcher message");

		rx.await
			.expect("Ack channel closed for watcher management message response")
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

	pub async fn pause_watcher(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		self.watcher_management_message(location_id, library, WatcherManagementMessageAction::Pause)
			.await
	}

	pub async fn resume_watcher(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		self.watcher_management_message(
			location_id,
			library,
			WatcherManagementMessageAction::Resume,
		)
		.await
	}

	pub async fn temporary_watcher_pause(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<PauseWatcherGuard<'_>, LocationManagerError> {
		self.pause_watcher(location_id, library.clone()).await?;

		Ok(PauseWatcherGuard {
			location_id,
			library: Some(library),
			manager: self,
		})
	}

	pub async fn temporary_ignore_events_for_path(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		path: impl AsRef<Path> + Send,
	) -> Result<IgnoreEventsForPathGuard<'_>, LocationManagerError> {
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
		// SAFETY: This will never block as we only have 1 sender and this channel has 1 slot
		if self.stop_tx.send_blocking(()).is_err() {
			error!("Failed to send stop signal to location manager");
		}
	}
}

#[must_use = "this `StopWatcherGuard` must be held for some time, so the watcher is stopped"]
pub struct PauseWatcherGuard<'m> {
	manager: &'m Locations,
	location_id: location::id::Type,
	library: Option<Arc<Library>>,
}

impl Drop for PauseWatcherGuard<'_> {
	fn drop(&mut self) {
		// FIXME: change this Drop to async drop in the future
		if let Err(e) = block_on(self.manager.resume_watcher(
			self.location_id,
			self.library.take().expect("library should be set"),
		)) {
			error!(?e, "Failed to resume watcher on stop watcher guard drop;");
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
		// FIXME: change this Drop to async drop in the future
		if let Err(e) = block_on(self.manager.watcher_management_message(
			self.location_id,
			self.library.take().expect("library should be set"),
			WatcherManagementMessageAction::IgnoreEventsForPath {
				path: self.path.take().expect("path should be set"),
				ignore: false,
			},
		)) {
			error!(?e, "Failed to un-ignore path on watcher guard drop;");
		}
	}
}
