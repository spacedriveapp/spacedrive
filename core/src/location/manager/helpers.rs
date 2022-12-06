use crate::{library::LibraryContext, prisma::location};

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	time::Duration,
};

use tokio::{fs, io::ErrorKind, time::sleep};
use tracing::error;

use super::{watcher::LocationWatcher, LocationId};

const LOCATION_CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub(super) async fn check_online(location: &location::Data, library_ctx: &LibraryContext) -> bool {
	if let Some(ref local_path) = location.local_path {
		match fs::metadata(local_path).await {
			Ok(_) => {
				if !location.is_online {
					set_location_online(location.id, library_ctx, true).await;
				}
				true
			}
			Err(e) if e.kind() == ErrorKind::NotFound => {
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

pub(super) async fn set_location_online(
	location_id: LocationId,
	library_ctx: &LibraryContext,
	online: bool,
) {
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

pub(super) async fn location_check_sleep(
	location_id: LocationId,
	library_ctx: LibraryContext,
) -> (LocationId, LibraryContext) {
	sleep(LOCATION_CHECK_INTERVAL).await;
	(location_id, library_ctx)
}

pub(super) fn watch_location(
	location: location::Data,
	location_path: impl AsRef<Path>,
	locations_watched: &mut HashMap<LocationId, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationId, LocationWatcher>,
) {
	let location_id = location.id;
	if let Some(mut watcher) = locations_unwatched.remove(&location_id) {
		if watcher.check_path(location_path) {
			watcher.watch();
		} else {
			watcher.update_data(location, true);
		}

		locations_watched.insert(location_id, watcher);
	}
}

pub(super) fn unwatch_location(
	location: location::Data,
	location_path: impl AsRef<Path>,
	locations_watched: &mut HashMap<LocationId, LocationWatcher>,
	locations_unwatched: &mut HashMap<LocationId, LocationWatcher>,
) {
	let location_id = location.id;
	if let Some(mut watcher) = locations_watched.remove(&location_id) {
		if watcher.check_path(location_path) {
			watcher.unwatch();
		} else {
			watcher.update_data(location, false)
		}

		locations_unwatched.insert(location_id, watcher);
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
