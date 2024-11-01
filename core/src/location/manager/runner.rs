use crate::{
	library::{Library, LibraryId},
	Node,
};

use sd_core_prisma_helpers::location_ids_and_path;

use sd_prisma::prisma::location;
use sd_utils::db::maybe_missing;

use std::{
	collections::{HashMap, HashSet},
	io::ErrorKind,
	path::PathBuf,
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use futures::stream::StreamExt;
use futures_concurrency::stream::Merge;
use tokio::{
	fs,
	sync::oneshot,
	time::{interval, MissedTickBehavior},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, instrument, trace, warn};
use uuid::Uuid;

use super::{
	watcher::LocationWatcher, LocationManagementMessage, LocationManagerError,
	ManagementMessageAction, WatcherManagementMessage, WatcherManagementMessageAction,
};

type LocationIdAndLibraryId = (location::id::Type, LibraryId);

struct Runner {
	node: Arc<Node>,
	device_pub_id_to_db: Vec<u8>,
	locations_to_check: HashMap<location::id::Type, Arc<Library>>,
	locations_watched: HashMap<LocationIdAndLibraryId, LocationWatcher>,
	locations_unwatched: HashMap<LocationIdAndLibraryId, LocationWatcher>,
	forced_unwatch: HashSet<LocationIdAndLibraryId>,
}
impl Runner {
	async fn new(node: Arc<Node>) -> Self {
		Self {
			device_pub_id_to_db: node.config.get().await.id.to_db(),
			node,
			locations_to_check: HashMap::new(),
			locations_watched: HashMap::new(),
			locations_unwatched: HashMap::new(),
			forced_unwatch: HashSet::new(),
		}
	}

	fn check_same_device(&self, location: &location_ids_and_path::Data) -> bool {
		location
			.device
			.as_ref()
			.is_some_and(|device| device.pub_id == self.device_pub_id_to_db)
	}

	async fn add_location(
		&mut self,
		location_id: i32,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		if let Some(location) = get_location(location_id, &library).await? {
			check_online(&location, &self.node, &library, &self.device_pub_id_to_db)
				.await
				.and_then(|is_online| {
					LocationWatcher::new(location, Arc::clone(&library), Arc::clone(&self.node))
						.map(|mut watcher| {
							if is_online {
								trace!(%location_id, "Location is online, watching it!;");
								watcher.watch();
								self.locations_watched
									.insert((location_id, library.id), watcher);
							} else {
								self.locations_unwatched
									.insert((location_id, library.id), watcher);
							}

							self.locations_to_check
								.insert(location_id, Arc::clone(&library));
						})
				})
		} else {
			Err(LocationManagerError::LocationNotFound(location_id))
		}
	}

	async fn remove_location(
		&mut self,
		location_id: i32,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library.id);

		if let Some(location) = get_location(location_id, &library).await? {
			if self.check_same_device(&location) {
				self.unwatch_location(location, library.id);
				self.locations_unwatched.remove(&key);
				self.forced_unwatch.remove(&key);
			} else {
				self.drop_location(
					location_id,
					library.id,
					"Dropping location from location manager, because it isn't from this device",
				);
			}
		} else {
			self.drop_location(
				location_id,
				library.id,
				"Removing location from location manager, as we failed to fetch from db",
			);
		}

		// Removing location from checker
		self.locations_to_check.remove(&location_id);

		Ok(())
	}

	#[instrument(skip(self, reason))]
	fn drop_location(
		&mut self,
		location_id: location::id::Type,
		library_id: LibraryId,
		reason: &'static str,
	) {
		warn!(%reason);
		if let Some(mut watcher) = self.locations_watched.remove(&(location_id, library_id)) {
			watcher.unwatch();
		} else {
			self.locations_unwatched.remove(&(location_id, library_id));
		}
	}

	fn watch_location(
		&mut self,
		location_ids_and_path::Data {
			id: location_id,
			path: maybe_location_path,
			..
		}: location_ids_and_path::Data,
		library_id: LibraryId,
	) {
		if let Some(location_path) = maybe_location_path {
			if let Some(mut watcher) = self.locations_unwatched.remove(&(location_id, library_id)) {
				if watcher.check_path(location_path) {
					watcher.watch();
				}

				self.locations_watched
					.insert((location_id, library_id), watcher);
			}
		}
	}

	fn unwatch_location(
		&mut self,
		location_ids_and_path::Data {
			id: location_id,
			path: maybe_location_path,
			..
		}: location_ids_and_path::Data,
		library_id: LibraryId,
	) {
		if let Some(location_path) = maybe_location_path {
			if let Some(mut watcher) = self.locations_watched.remove(&(location_id, library_id)) {
				if watcher.check_path(location_path) {
					watcher.unwatch();
				}

				self.locations_unwatched
					.insert((location_id, library_id), watcher);
			}
		}
	}

	#[instrument(skip(self, library), fields(library_id = %library.id), err)]
	async fn pause_watcher(
		&mut self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library.id);

		if !self.forced_unwatch.contains(&key) && self.locations_watched.contains_key(&key) {
			get_location(location_id, &library)
				.await?
				.ok_or(LocationManagerError::LocationNotFound(location_id))
				.map(|location| {
					self.unwatch_location(location, library.id);
					self.forced_unwatch.insert(key);
				})
		} else {
			Ok(())
		}
	}

	#[instrument(skip(self, library), fields(library_id = %library.id), err)]
	async fn resume_watcher(
		&mut self,
		location_id: location::id::Type,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library.id);

		if self.forced_unwatch.contains(&key) && self.locations_unwatched.contains_key(&key) {
			get_location(location_id, &library)
				.await?
				.ok_or(LocationManagerError::LocationNotFound(location_id))
				.map(|location| {
					self.watch_location(location, library.id);
					self.forced_unwatch.remove(&key);
				})
		} else {
			Ok(())
		}
	}

	async fn ignore_events_for_path(
		&self,
		location_id: location::id::Type,
		library: Arc<Library>,
		path: PathBuf,
		ignore: bool,
	) {
		if let Some(watcher) = self.locations_watched.get(&(location_id, library.id)) {
			watcher.ignore_path(path, ignore).await
		}
	}

	async fn handle_location_management_message(
		&mut self,
		location_id: location::id::Type,
		library: Arc<Library>,
		action: ManagementMessageAction,
		ack: oneshot::Sender<Result<(), LocationManagerError>>,
	) {
		ack.send(match action {
			ManagementMessageAction::Add => self.add_location(location_id, library).await,
			ManagementMessageAction::Remove => self.remove_location(location_id, library).await,
		})
		.expect("Ack channel closed")
	}

	async fn handle_watcher_management_message(
		&mut self,
		location_id: location::id::Type,
		library: Arc<Library>,
		action: WatcherManagementMessageAction,
		ack: oneshot::Sender<Result<(), LocationManagerError>>,
	) {
		ack.send(match action {
			WatcherManagementMessageAction::Pause => self.pause_watcher(location_id, library).await,
			WatcherManagementMessageAction::Resume => {
				self.resume_watcher(location_id, library).await
			}
			WatcherManagementMessageAction::IgnoreEventsForPath { path, ignore } => {
				self.ignore_events_for_path(location_id, library, path, ignore)
					.await;
				Ok(())
			}
		})
		.expect("Ack channel closed")
	}

	async fn check_locations(
		&mut self,
		locations_to_check_buffer: &mut Vec<(location::id::Type, Arc<Library>)>,
	) -> Result<(), Vec<LocationManagerError>> {
		let mut errors = vec![];
		locations_to_check_buffer.clear();
		locations_to_check_buffer.extend(self.locations_to_check.drain());

		for (location_id, library) in locations_to_check_buffer.drain(..) {
			if let Err(e) = self
				.check_single_location(location_id, Arc::clone(&library))
				.await
			{
				self.drop_location(
					location_id,
					library.id,
					"Removing location from manager, as we failed to check if it was online",
				);
				self.forced_unwatch.remove(&(location_id, library.id));
				errors.push(e);
			}
		}

		Ok(())
	}

	async fn check_single_location(
		&mut self,
		location_id: i32,
		library: Arc<Library>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library.id);

		if let Some(location) = get_location(location_id, &library).await? {
			if self.check_same_device(&location) {
				if check_online(&location, &self.node, &library, &self.device_pub_id_to_db).await?
					&& !self.forced_unwatch.contains(&key)
				{
					self.watch_location(location, library.id);
				} else {
					self.unwatch_location(location, library.id);
				}

				self.locations_to_check.insert(location_id, library);
			} else {
				self.drop_location(
					location_id,
					library.id,
					"Dropping location from location manager, because \
							it isn't a location in the current device",
				);
				self.forced_unwatch.remove(&key);
			}

			Ok(())
		} else {
			Err(LocationManagerError::LocationNotFound(location_id))
		}
	}
}

pub(super) async fn run(
	location_management_rx: chan::Receiver<LocationManagementMessage>,
	watcher_management_rx: chan::Receiver<WatcherManagementMessage>,
	stop_rx: chan::Receiver<()>,
	node: Arc<Node>,
) {
	enum StreamMessage {
		LocationManagementMessage(LocationManagementMessage),
		WatcherManagementMessage(WatcherManagementMessage),
		CheckLocations,
		Stop,
	}

	let mut locations_to_check_buffer = vec![];

	let mut check_locations_interval = interval(Duration::from_secs(2));
	check_locations_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

	let mut runner = Runner::new(node).await;

	let mut msg_stream = pin!((
		location_management_rx.map(StreamMessage::LocationManagementMessage),
		watcher_management_rx.map(StreamMessage::WatcherManagementMessage),
		IntervalStream::new(check_locations_interval).map(|_| StreamMessage::CheckLocations),
		stop_rx.map(|()| StreamMessage::Stop),
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			StreamMessage::LocationManagementMessage(LocationManagementMessage {
				location_id,
				library,
				action,
				ack,
			}) => {
				runner
					.handle_location_management_message(location_id, library, action, ack)
					.await
			}
			// Watcher management messages
			StreamMessage::WatcherManagementMessage(WatcherManagementMessage {
				location_id,
				library,
				action,
				ack,
			}) => {
				runner
					.handle_watcher_management_message(location_id, library, action, ack)
					.await
			}
			StreamMessage::CheckLocations => {
				if let Err(errors) = runner.check_locations(&mut locations_to_check_buffer).await {
					warn!(?errors, "Errors while checking locations;");
				}
			}
			StreamMessage::Stop => {
				debug!("Stopping location manager");
				break;
			}
		}
	}
}

#[instrument(skip(library), fields(library_id = %library.id), err)]
async fn get_location(
	location_id: location::id::Type,
	library: &Library,
) -> Result<Option<location_ids_and_path::Data>, LocationManagerError> {
	library
		.db
		.location()
		.find_unique(location::id::equals(location_id))
		.select(location_ids_and_path::select())
		.exec()
		.await
		.map_err(Into::into)
}

#[instrument(
	skip_all,
	fields(%location_id, library_id = %library.id),
	err,
)]
async fn check_online(
	location_ids_and_path::Data {
		id: location_id,
		pub_id,
		device,
		path,
	}: &location_ids_and_path::Data,
	node: &Node,
	library: &Library,
	device_pub_id_to_db: &[u8],
) -> Result<bool, LocationManagerError> {
	let pub_id = Uuid::from_slice(pub_id)?;

	if device
		.as_ref()
		.is_some_and(|device| device.pub_id == device_pub_id_to_db)
	{
		match fs::metadata(maybe_missing(path, "location.path")?).await {
			Ok(_) => {
				node.locations.add_online(pub_id).await;
				Ok(true)
			}
			Err(e) if e.kind() == ErrorKind::NotFound => {
				node.locations.remove_online(&pub_id).await;
				Ok(false)
			}
			Err(e) => {
				error!(
					?e,
					"Failed to check if location is online, will consider as offline;"
				);
				Ok(false)
			}
		}
	} else {
		// In this case, we don't have a `local_path`, but this location was marked as online
		node.locations.remove_online(&pub_id).await;
		Err(LocationManagerError::NonLocalLocation(*location_id))
	}
}
