use crate::{library::LoadedLibrary, prisma::location, util::db::maybe_missing, Node};

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use tokio::{fs, io::ErrorKind, sync::oneshot, time::sleep};
use tracing::{error, warn};
use uuid::Uuid;

use super::{watcher::LocationWatcher, LocationManagerError};

type LibraryId = Uuid;
type LocationAndLibraryKey = (location::id::Type, LibraryId);

const LOCATION_CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub(super) async fn check_online(
	location: &location::Data,
	node: &Node,
	library: &LoadedLibrary,
) -> Result<bool, LocationManagerError> {
	let pub_id = Uuid::from_slice(&location.pub_id)?;

	let location_path = maybe_missing(&location.path, "location.path").map(Path::new)?;

	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id == Some(library.config.instance_id) {
		match fs::metadata(&location_path).await {
			Ok(_) => {
				node.location_manager.add_online(pub_id).await;
				Ok(true)
			}
			Err(e) if e.kind() == ErrorKind::NotFound => {
				node.location_manager.remove_online(&pub_id).await;
				Ok(false)
			}
			Err(e) => {
				error!("Failed to check if location is online: {:#?}", e);
				Ok(false)
			}
		}
	} else {
		// In this case, we don't have a `local_path`, but this location was marked as online
		node.location_manager.remove_online(&pub_id).await;
		Err(LocationManagerError::NonLocalLocation(location.id))
	}
}

pub(super) async fn location_check_sleep(
	location_id: location::id::Type,
	library: Arc<LoadedLibrary>,
) -> (location::id::Type, Arc<LoadedLibrary>) {
	sleep(LOCATION_CHECK_INTERVAL).await;
	(location_id, library)
}

pub(super) fn watch_location(
	location: location::Data,
	library_id: LibraryId,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	let location_id = location.id;
	let location_path = location.path.as_ref();
	let Some(location_path) = location_path.map(Path::new) else {
        return
    };

	if let Some(mut watcher) = locations_unwatched.remove(&(location_id, library_id)) {
		if watcher.check_path(location_path) {
			watcher.watch();
		}

		locations_watched.insert((location_id, library_id), watcher);
	}
}

pub(super) fn unwatch_location(
	location: location::Data,
	library_id: LibraryId,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	let location_id = location.id;
	let location_path = location.path.as_ref();
	let Some(location_path) = location_path.map(Path::new) else {
        return
    };

	if let Some(mut watcher) = locations_watched.remove(&(location_id, library_id)) {
		if watcher.check_path(location_path) {
			watcher.unwatch();
		}

		locations_unwatched.insert((location_id, library_id), watcher);
	}
}

pub(super) fn drop_location(
	location_id: location::id::Type,
	library_id: LibraryId,
	message: &str,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	warn!("{message}: <id='{location_id}', library_id='{library_id}'>",);
	if let Some(mut watcher) = locations_watched.remove(&(location_id, library_id)) {
		watcher.unwatch();
	} else {
		locations_unwatched.remove(&(location_id, library_id));
	}
}

pub(super) async fn get_location(
	location_id: location::id::Type,
	library: &LoadedLibrary,
) -> Option<location::Data> {
	library
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

pub(super) async fn handle_remove_location_request(
	location_id: location::id::Type,
	library: Arc<LoadedLibrary>,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	to_remove: &mut HashSet<LocationAndLibraryKey>,
) {
	let key = (location_id, library.id);
	if let Some(location) = get_location(location_id, &library).await {
		// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
		if location.instance_id == Some(library.config.instance_id) {
			unwatch_location(location, library.id, locations_watched, locations_unwatched);
			locations_unwatched.remove(&key);
			forced_unwatch.remove(&key);
		} else {
			drop_location(
		 		location_id,
		 		library.id,
		 		"Dropping location from location manager, because we don't have a `local_path` anymore",
		 		locations_watched,
		 		locations_unwatched
		 	);
		}
	} else {
		drop_location(
			location_id,
			library.id,
			"Removing location from manager, as we failed to fetch from db",
			locations_watched,
			locations_unwatched,
		);
	}

	// Marking location as removed, so we don't try to check it when the time comes
	to_remove.insert(key);

	let _ = response_tx.send(Ok(())); // ignore errors, we handle errors on receiver
}

pub(super) async fn handle_stop_watcher_request(
	location_id: location::id::Type,
	library: Arc<LoadedLibrary>,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	async fn inner(
		location_id: location::id::Type,
		library: Arc<LoadedLibrary>,
		forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
		locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
		locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library.id);
		if !forced_unwatch.contains(&key) && locations_watched.contains_key(&key) {
			get_location(location_id, &library)
				.await
				.ok_or_else(|| LocationManagerError::FailedToStopOrReinitWatcher {
					reason: String::from("failed to fetch location from db"),
				})
				.map(|location| {
					unwatch_location(location, library.id, locations_watched, locations_unwatched);
					forced_unwatch.insert(key);
				})
		} else {
			Ok(())
		}
	}

	let _ = response_tx.send(
		inner(
			location_id,
			library,
			forced_unwatch,
			locations_watched,
			locations_unwatched,
		)
		.await,
	); // ignore errors, we handle errors on receiver
}

pub(super) async fn handle_reinit_watcher_request(
	location_id: location::id::Type,
	library: Arc<LoadedLibrary>,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	async fn inner(
		location_id: location::id::Type,
		library: Arc<LoadedLibrary>,
		forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
		locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
		locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library.id);
		if forced_unwatch.contains(&key) && locations_unwatched.contains_key(&key) {
			get_location(location_id, &library)
				.await
				.ok_or_else(|| LocationManagerError::FailedToStopOrReinitWatcher {
					reason: String::from("failed to fetch location from db"),
				})
				.map(|location| {
					watch_location(location, library.id, locations_watched, locations_unwatched);
					forced_unwatch.remove(&key);
				})
		} else {
			Ok(())
		}
	}

	let _ = response_tx.send(
		inner(
			location_id,
			library,
			forced_unwatch,
			locations_watched,
			locations_unwatched,
		)
		.await,
	); // ignore errors, we handle errors on receiver
}

pub(super) fn handle_ignore_path_request(
	location_id: location::id::Type,
	library: Arc<LoadedLibrary>,
	path: PathBuf,
	ignore: bool,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	locations_watched: &HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	let _ = response_tx.send(
		if let Some(watcher) = locations_watched.get(&(location_id, library.id)) {
			watcher.ignore_path(path, ignore)
		} else {
			Ok(())
		},
	); // ignore errors, we handle errors on receiver
}
