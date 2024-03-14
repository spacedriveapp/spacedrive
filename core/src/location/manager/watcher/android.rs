//! Android file system watcher implementation.
//! TODO: Still being worked on by @Rocky43007 on Branch Rocky43007:location-watcher-test-3
//! DO NOT TOUCH FOR NOW

use crate::{invalidate_query, library::Library, location::manager::LocationManagerError, Node};

use sd_android_fs_watcher::WatcherEvent;
use sd_prisma::prisma::location;

use std::{
	collections::{BTreeMap, HashMap},
	path::{Path, PathBuf},
	sync::Arc,
};

use async_trait::async_trait;
use notify::Event;
use tokio::time::Instant;
use tracing::{error, info, trace};

use super::{
	utils::{recalculate_directories_size, remove, update_file},
	EventHandler, HUNDRED_MILLIS, ONE_SECOND,
};

#[derive(Debug)]
pub(super) struct AndroidEventHandler<'lib> {
	location_id: location::id::Type,
	library: &'lib Arc<Library>,
	node: &'lib Arc<Node>,
	last_events_eviction_check: Instant,
	rename_from: HashMap<PathBuf, Instant>,
	recently_renamed_from: BTreeMap<PathBuf, Instant>,
	files_to_update: HashMap<PathBuf, Instant>,
	reincident_to_update_files: HashMap<PathBuf, Instant>,
	to_recalculate_size: HashMap<PathBuf, Instant>,
	path_and_instant_buffer: Vec<(PathBuf, Instant)>,
}

#[async_trait]
impl<'lib> EventHandler<'lib> for AndroidEventHandler<'lib> {
	fn new(
		location_id: location::id::Type,
		library: &'lib Arc<Library>,
		node: &'lib Arc<Node>,
	) -> Self {
		Self {
			location_id,
			library,
			node,
			last_events_eviction_check: Instant::now(),
			rename_from: HashMap::new(),
			recently_renamed_from: BTreeMap::new(),
			files_to_update: HashMap::new(),
			reincident_to_update_files: HashMap::new(),
			to_recalculate_size: HashMap::new(),
			path_and_instant_buffer: Vec::new(),
		}
	}

	async fn handle_event(&mut self, event: WatcherEvent) -> Result<(), LocationManagerError> {
		info!("Received Android event: {:#?}", event);

		// let Event {
		// 	kind, mut paths, ..
		// } = event;

		// match kind {
		// 	EventKind::Create(CreateKind::File)
		// 	| EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
		// 		// When we receive a create, modify data or metadata events of the abore kinds
		// 		// we just mark the file to be updated in a near future
		// 		// each consecutive event of these kinds that we receive for the same file
		// 		// we just store the path again in the map below, with a new instant
		// 		// that effectively resets the timer for the file to be updated
		// 		let path = paths.remove(0);
		// 		if self.files_to_update.contains_key(&path) {
		// 			if let Some(old_instant) =
		// 				self.files_to_update.insert(path.clone(), Instant::now())
		// 			{
		// 				self.reincident_to_update_files
		// 					.entry(path)
		// 					.or_insert(old_instant);
		// 			}
		// 		} else {
		// 			self.files_to_update.insert(path, Instant::now());
		// 		}
		// 	}

		// 	EventKind::Create(CreateKind::Folder) => {
		// 		let path = &paths[0];

		// 		// Don't need to dispatch a recalculate directory event as `create_dir` dispatches
		// 		// a `scan_location_sub_path` function, which recalculates the size already

		// 		create_dir(
		// 			self.location_id,
		// 			path,
		// 			&fs::metadata(path)
		// 				.await
		// 				.map_err(|e| FileIOError::from((path, e)))?,
		// 			self.node,
		// 			self.library,
		// 		)
		// 		.await?;
		// 	}
		// 	EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
		// 		// Just in case we can't garantee that we receive the Rename From event before the
		// 		// Rename Both event. Just a safeguard
		// 		if self.recently_renamed_from.remove(&paths[0]).is_none() {
		// 			self.rename_from.insert(paths.remove(0), Instant::now());
		// 		}
		// 	}

		// 	EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
		// 		let from_path = &paths[0];
		// 		let to_path = &paths[1];

		// 		self.rename_from.remove(from_path);
		// 		rename(
		// 			self.location_id,
		// 			to_path,
		// 			from_path,
		// 			fs::metadata(to_path)
		// 				.await
		// 				.map_err(|e| FileIOError::from((to_path, e)))?,
		// 			self.library,
		// 		)
		// 		.await?;
		// 		self.recently_renamed_from
		// 			.insert(paths.swap_remove(0), Instant::now());
		// 	}
		// 	EventKind::Remove(_) => {
		// 		let path = paths.remove(0);
		// 		if let Some(parent) = path.parent() {
		// 			if parent != Path::new("") {
		// 				self.to_recalculate_size
		// 					.insert(parent.to_path_buf(), Instant::now());
		// 			}
		// 		}

		// 		remove(self.location_id, &path, self.library).await?;
		// 	}
		// 	other_event_kind => {
		// 		trace!("Other Linux event that we don't handle for now: {other_event_kind:#?}");
		// 	}
		// }

		Ok(())
	}

	async fn tick(&mut self) {
		if self.last_events_eviction_check.elapsed() > HUNDRED_MILLIS {
			if let Err(e) = self.handle_to_update_eviction().await {
				error!("Error while handling recently created or update files eviction: {e:#?}");
			}

			if let Err(e) = self.handle_rename_from_eviction().await {
				error!("Failed to remove file_path: {e:#?}");
			}

			self.recently_renamed_from
				.retain(|_, instant| instant.elapsed() < HUNDRED_MILLIS);

			if !self.to_recalculate_size.is_empty() {
				if let Err(e) = recalculate_directories_size(
					&mut self.to_recalculate_size,
					&mut self.path_and_instant_buffer,
					self.location_id,
					self.library,
				)
				.await
				{
					error!("Failed to recalculate directories size: {e:#?}");
				}
			}

			self.last_events_eviction_check = Instant::now();
		}
	}
}

impl AndroidEventHandler<'_> {
	async fn handle_to_update_eviction(&mut self) -> Result<(), LocationManagerError> {
		self.path_and_instant_buffer.clear();
		let mut should_invalidate = false;

		for (path, created_at) in self.files_to_update.drain() {
			if created_at.elapsed() < HUNDRED_MILLIS * 5 {
				self.path_and_instant_buffer.push((path, created_at));
			} else {
				if let Some(parent) = path.parent() {
					if parent != Path::new("") {
						self.to_recalculate_size
							.insert(parent.to_path_buf(), Instant::now());
					}
				}
				self.reincident_to_update_files.remove(&path);
				update_file(self.location_id, &path, self.node, self.library).await?;
				should_invalidate = true;
			}
		}

		self.files_to_update
			.extend(self.path_and_instant_buffer.drain(..));

		self.path_and_instant_buffer.clear();

		// We have to check if we have any reincident files to update and update them after a bigger
		// timeout, this way we keep track of files being update frequently enough to bypass our
		// eviction check above
		for (path, created_at) in self.reincident_to_update_files.drain() {
			if created_at.elapsed() < ONE_SECOND * 10 {
				self.path_and_instant_buffer.push((path, created_at));
			} else {
				if let Some(parent) = path.parent() {
					if parent != Path::new("") {
						self.to_recalculate_size
							.insert(parent.to_path_buf(), Instant::now());
					}
				}
				self.files_to_update.remove(&path);
				update_file(self.location_id, &path, self.node, self.library).await?;
				should_invalidate = true;
			}
		}

		if should_invalidate {
			invalidate_query!(self.library, "search.paths");
		}

		self.reincident_to_update_files
			.extend(self.path_and_instant_buffer.drain(..));

		Ok(())
	}

	async fn handle_rename_from_eviction(&mut self) -> Result<(), LocationManagerError> {
		self.path_and_instant_buffer.clear();
		let mut should_invalidate = false;

		for (path, instant) in self.rename_from.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if let Some(parent) = path.parent() {
					if parent != Path::new("") {
						self.to_recalculate_size
							.insert(parent.to_path_buf(), Instant::now());
					}
				}
				remove(self.location_id, &path, self.library).await?;
				should_invalidate = true;
				trace!("Removed file_path due timeout: {}", path.display());
			} else {
				self.path_and_instant_buffer.push((path, instant));
			}
		}

		if should_invalidate {
			invalidate_query!(self.library, "search.paths");
		}

		for (path, instant) in self.path_and_instant_buffer.drain(..) {
			self.rename_from.insert(path, instant);
		}

		Ok(())
	}
}
