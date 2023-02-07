use crate::{library::LibraryContext, prisma::location};

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	time::Duration,
};

use tokio::{fs, io::ErrorKind, sync::oneshot, time::sleep};
use tracing::{error, warn};
use uuid::Uuid;

use super::{watcher::LocationWatcher, LocationId, LocationManagerError};

type LibraryId = Uuid;
type LocationAndLibraryKey = (LocationId, LibraryId);

const LOCATION_CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub(super) async fn check_online(location: &location::Data, library_ctx: &LibraryContext) -> bool {
	let pub_id = &location.pub_id;

	if let Some(ref local_path) = location.local_path {
		match fs::metadata(local_path).await {
			Ok(_) => {
				library_ctx.location_manager().add_online(pub_id).await;
				// if !location.is_online {
				// 	set_location_online(location.id, library_ctx, true).await;
				// }
				true
			}
			Err(e) if e.kind() == ErrorKind::NotFound => {
				library_ctx.location_manager().remove_online(pub_id).await;
				// if location.is_online {
				// 	set_location_online(location.id, library_ctx, false).await;
				// }
				false
			}
			Err(e) => {
				error!("Failed to check if location is online: {:#?}", e);
				false
			}
		}
	} else {
		// In this case, we don't have a `local_path`, but this location was marked as online
		library_ctx.location_manager().remove_online(pub_id).await;
		// if location.is_online {
		// 	set_location_online(location.id, library_ctx, false).await;
		// }
		false
	}
}

// pub(super) async fn set_location_online(
// 	location_id: LocationId,
// 	library_ctx: &LibraryContext,
// 	online: bool,
// ) {
// 	if let Err(e) = library_ctx
// 		.db
// 		.location()
// 		.update(
// 			location::id::equals(location_id),
// 			vec![location::is_online::set(online)],
// 		)
// 		.exec()
// 		.await
// 	{
// 		error!(
// 			"Failed to update location to online: (id: {}, error: {:#?})",
// 			location_id, e
// 		);
// 	}
// }

pub(super) async fn location_check_sleep(
	location_id: LocationId,
	library_ctx: LibraryContext,
) -> (LocationId, LibraryContext) {
	sleep(LOCATION_CHECK_INTERVAL).await;
	(location_id, library_ctx)
}

pub(super) fn watch_location(
	location: location::Data,
	library_id: LibraryId,
	location_path: impl AsRef<Path>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	let location_id = location.id;
	if let Some(mut watcher) = locations_unwatched.remove(&(location_id, library_id)) {
		if watcher.check_path(location_path) {
			watcher.watch();
		} else {
			watcher.update_data(location, true);
		}

		locations_watched.insert((location_id, library_id), watcher);
	}
}

pub(super) fn unwatch_location(
	location: location::Data,
	library_id: LibraryId,
	location_path: impl AsRef<Path>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	let location_id = location.id;
	if let Some(mut watcher) = locations_watched.remove(&(location_id, library_id)) {
		if watcher.check_path(location_path) {
			watcher.unwatch();
		} else {
			watcher.update_data(location, false)
		}

		locations_unwatched.insert((location_id, library_id), watcher);
	}
}

pub(super) fn drop_location(
	location_id: LocationId,
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
	location_id: i32,
	library_ctx: &LibraryContext,
) -> Option<location::Data> {
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

pub(super) fn subtract_location_path(
	location_path: impl AsRef<Path>,
	current_path: impl AsRef<Path>,
) -> Option<PathBuf> {
	let location_path = location_path.as_ref();
	let current_path = current_path.as_ref();

	if let Ok(stripped) = current_path.strip_prefix(location_path) {
		Some(stripped.to_path_buf())
	} else {
		error!(
			"Failed to strip location root path ({}) from current path ({})",
			location_path.display(),
			current_path.display()
		);
		None
	}
}

pub(super) async fn handle_remove_location_request(
	location_id: LocationId,
	library_ctx: LibraryContext,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	to_remove: &mut HashSet<LocationAndLibraryKey>,
) {
	let key = (location_id, library_ctx.id);
	if let Some(location) = get_location(location_id, &library_ctx).await {
		if let Some(ref local_path_str) = location.local_path.clone() {
			unwatch_location(
				location,
				library_ctx.id,
				local_path_str,
				locations_watched,
				locations_unwatched,
			);
			locations_unwatched.remove(&key);
			forced_unwatch.remove(&key);
		} else {
			drop_location(
				location_id,
				library_ctx.id,
				"Dropping location from location manager, because we don't have a `local_path` anymore",
				locations_watched,
				locations_unwatched
			);
		}
	} else {
		drop_location(
			location_id,
			library_ctx.id,
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
	location_id: LocationId,
	library_ctx: LibraryContext,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	async fn inner(
		location_id: LocationId,
		library_ctx: LibraryContext,
		forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
		locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
		locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library_ctx.id);
		if !forced_unwatch.contains(&key) && locations_watched.contains_key(&key) {
			get_location(location_id, &library_ctx)
				.await
				.ok_or_else(|| LocationManagerError::FailedToStopOrReinitWatcher {
					reason: String::from("failed to fetch location from db"),
				})
				.map(|location| {
					location
						.local_path
						.clone()
						.ok_or(LocationManagerError::LocationMissingLocalPath(location_id))
						.map(|local_path_str| {
							unwatch_location(
								location,
								library_ctx.id,
								local_path_str,
								locations_watched,
								locations_unwatched,
							);
							forced_unwatch.insert(key);
						})
				})?
		} else {
			Ok(())
		}
	}

	let _ = response_tx.send(
		inner(
			location_id,
			library_ctx,
			forced_unwatch,
			locations_watched,
			locations_unwatched,
		)
		.await,
	); // ignore errors, we handle errors on receiver
}

pub(super) async fn handle_reinit_watcher_request(
	location_id: LocationId,
	library_ctx: LibraryContext,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
	locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	async fn inner(
		location_id: LocationId,
		library_ctx: LibraryContext,
		forced_unwatch: &mut HashSet<LocationAndLibraryKey>,
		locations_watched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
		locations_unwatched: &mut HashMap<LocationAndLibraryKey, LocationWatcher>,
	) -> Result<(), LocationManagerError> {
		let key = (location_id, library_ctx.id);
		if forced_unwatch.contains(&key) && locations_unwatched.contains_key(&key) {
			get_location(location_id, &library_ctx)
				.await
				.ok_or_else(|| LocationManagerError::FailedToStopOrReinitWatcher {
					reason: String::from("failed to fetch location from db"),
				})
				.map(|location| {
					location
						.local_path
						.clone()
						.ok_or(LocationManagerError::LocationMissingLocalPath(location_id))
						.map(|local_path_str| {
							watch_location(
								location,
								library_ctx.id,
								local_path_str,
								locations_watched,
								locations_unwatched,
							);
							forced_unwatch.remove(&key);
						})
				})?
		} else {
			Ok(())
		}
	}

	let _ = response_tx.send(
		inner(
			location_id,
			library_ctx,
			forced_unwatch,
			locations_watched,
			locations_unwatched,
		)
		.await,
	); // ignore errors, we handle errors on receiver
}

pub(super) fn handle_ignore_path_request(
	location_id: LocationId,
	library_ctx: LibraryContext,
	path: PathBuf,
	ignore: bool,
	response_tx: oneshot::Sender<Result<(), LocationManagerError>>,
	locations_watched: &HashMap<LocationAndLibraryKey, LocationWatcher>,
) {
	let _ = response_tx.send(
		if let Some(watcher) = locations_watched.get(&(location_id, library_ctx.id)) {
			watcher.ignore_path(path, ignore)
		} else {
			Ok(())
		},
	); // ignore errors, we handle errors on receiver
}
