//! On MacOS, we use the FSEvents backend of notify-rs and Rename events are pretty complicated;
//! There are just (ModifyKind::Name(RenameMode::Any) events and nothing else.
//! This means that we have to link the old path with the new path to know which file was renamed.
//! But you can't forget that renames events aren't always the case that I file name was modified,
//! but its path was modified. So we have to check if the file was moved. When a file is moved
//! inside the same location, we received 2 events: one for the old path and one for the new path.
//! But when a file is moved to another location, we only receive the old path event... This
//! way we have to handle like a file deletion, and the same applies for when a file is moved to our
//! current location from anywhere else, we just receive the new path rename event, which means a
//! creation.

use crate::{
	invalidate_query,
	library::Library,
	location::{
		file_path_helper::{check_existing_file_path, get_inode_and_device, MaterializedPath},
		manager::LocationManagerError,
		LocationId,
	},
};

use std::{
	collections::{BTreeMap, HashMap},
	path::PathBuf,
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, io, time::Instant};
use tracing::{error, trace, warn};

use super::{
	utils::{
		create_dir, create_dir_or_file, create_file, extract_inode_and_device_from_path,
		extract_location_path, remove, rename, update_file,
	},
	EventHandler, INodeAndDevice, InstantAndPath, HUNDRED_MILLIS, ONE_SECOND,
};

#[derive(Debug)]
pub(super) struct MacOsEventHandler<'lib> {
	location_id: LocationId,
	library: &'lib Library,
	recently_created_files: BTreeMap<PathBuf, Instant>,
	last_check_created_files: Instant,
	latest_created_dir: Option<PathBuf>,
	last_check_rename: Instant,
	old_paths_map: HashMap<INodeAndDevice, InstantAndPath>,
	new_paths_map: HashMap<INodeAndDevice, InstantAndPath>,
	paths_map_buffer: Vec<(INodeAndDevice, InstantAndPath)>,
}

#[async_trait]
impl<'lib> EventHandler<'lib> for MacOsEventHandler<'lib> {
	fn new(location_id: LocationId, library: &'lib Library) -> Self
	where
		Self: Sized,
	{
		Self {
			location_id,
			library,
			recently_created_files: BTreeMap::new(),
			last_check_created_files: Instant::now(),
			latest_created_dir: None,
			last_check_rename: Instant::now(),
			old_paths_map: HashMap::new(),
			new_paths_map: HashMap::new(),
			paths_map_buffer: Vec::new(),
		}
	}

	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		trace!("Received MacOS event: {:#?}", event);

		let Event {
			kind, mut paths, ..
		} = event;

		match kind {
			EventKind::Create(CreateKind::Folder) => {
				if let Some(latest_created_dir) = self.latest_created_dir.take() {
					if paths[0] == latest_created_dir {
						// NOTE: This is a MacOS specific event that happens when a folder is created
						// trough Finder. It creates a folder but 2 events are triggered in
						// FSEvents. So we store and check the latest created folder to avoid
						// hiting a unique constraint in the database
						return Ok(());
					}
				}

				create_dir(
					self.location_id,
					&paths[0],
					&fs::metadata(&paths[0]).await?,
					self.library,
				)
				.await?;
				self.latest_created_dir = Some(paths.remove(0));
			}
			EventKind::Create(CreateKind::File) => {
				create_file(
					self.location_id,
					&paths[0],
					&fs::metadata(&paths[0]).await?,
					self.library,
				)
				.await?;
				self.recently_created_files
					.insert(paths.remove(0), Instant::now());
			}
			EventKind::Modify(ModifyKind::Data(DataChange::Content)) => {
				// NOTE: MacOS emits a Create File and then an Update Content event
				// when a file is created. So we need to check if the file was recently
				// created to avoid unecessary updates
				if !self.recently_created_files.contains_key(&paths[0]) {
					update_file(self.location_id, &paths[0], self.library).await?;
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
				self.handle_single_rename_event(paths.remove(0)).await?;
			}

			EventKind::Remove(_) => {
				remove(self.location_id, &paths[0], self.library).await?;
			}
			other_event_kind => {
				trace!("Other MacOS event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}

	async fn tick(&mut self) {
		// Cleaning out recently created files that are older than 1 second
		if self.last_check_created_files.elapsed() > ONE_SECOND {
			self.last_check_created_files = Instant::now();
			self.recently_created_files
				.retain(|_, created_at| created_at.elapsed() < ONE_SECOND);
		}

		if self.last_check_rename.elapsed() > HUNDRED_MILLIS {
			// Cleaning out recently renamed files that are older than 100 milliseconds
			self.handle_create_eviction().await;
			self.handle_remove_eviction().await;
		}
	}
}

impl MacOsEventHandler<'_> {
	async fn handle_create_eviction(&mut self) {
		// Just to make sure that our buffer is clean
		self.paths_map_buffer.clear();

		for (inode_and_device, (instant, path)) in self.new_paths_map.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if let Err(e) = create_dir_or_file(self.location_id, &path, self.library).await {
					error!("Failed to create file_path on MacOS : {e}");
				} else {
					trace!("Created file_path due timeout: {}", path.display());
					invalidate_query!(self.library, "locations.getExplorerData");
				}
			} else {
				self.paths_map_buffer
					.push((inode_and_device, (instant, path)));
			}
		}

		for (key, value) in self.paths_map_buffer.drain(..) {
			self.new_paths_map.insert(key, value);
		}
	}

	async fn handle_remove_eviction(&mut self) {
		// Just to make sure that our buffer is clean
		self.paths_map_buffer.clear();

		for (inode_and_device, (instant, path)) in self.old_paths_map.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if let Err(e) = remove(self.location_id, &path, self.library).await {
					error!("Failed to remove file_path: {e}");
				} else {
					trace!("Removed file_path due timeout: {}", path.display());
					invalidate_query!(self.library, "locations.getExplorerData");
				}
			} else {
				self.paths_map_buffer
					.push((inode_and_device, (instant, path)));
			}
		}

		for (key, value) in self.paths_map_buffer.drain(..) {
			self.old_paths_map.insert(key, value);
		}
	}

	async fn handle_single_rename_event(
		&mut self,
		path: PathBuf, // this is used internally only once, so we can use just PathBuf
	) -> Result<(), LocationManagerError> {
		match fs::metadata(&path).await {
			Ok(meta) => {
				// File or directory exists, so this can be a "new path" to an actual rename/move or a creation
				trace!("Path exists: {}", path.display());

				let inode_and_device = get_inode_and_device(&meta)?;
				let location_path = extract_location_path(self.location_id, self.library).await?;

				if !check_existing_file_path(
					&MaterializedPath::new(self.location_id, &location_path, &path, meta.is_dir())?,
					&self.library.db,
				)
				.await?
				{
					if let Some((_, old_path)) = self.old_paths_map.remove(&inode_and_device) {
						trace!(
							"Got a match new -> old: {} -> {}",
							path.display(),
							old_path.display()
						);

						// We found a new path for this old path, so we can rename it
						rename(self.location_id, &path, &old_path, self.library).await?;
					} else {
						trace!("No match for new path yet: {}", path.display());
						self.new_paths_map
							.insert(inode_and_device, (Instant::now(), path));
					}
				} else {
					warn!(
						"Received rename event for a file that already exists in the database: {}",
						path.display()
					);
				}
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				// File or directory does not exist in the filesystem, if it exists in the database,
				// then we try pairing it with the old path from our map

				trace!("Path doesn't exists: {}", path.display());

				let inode_and_device =
					extract_inode_and_device_from_path(self.location_id, &path, self.library)
						.await?;

				if let Some((_, new_path)) = self.new_paths_map.remove(&inode_and_device) {
					trace!(
						"Got a match old -> new: {} -> {}",
						path.display(),
						new_path.display()
					);

					// We found a new path for this old path, so we can rename it
					rename(self.location_id, &new_path, &path, self.library).await?;
				} else {
					trace!("No match for old path yet: {}", path.display());
					// We didn't find a new path for this old path, so we store ir for later
					self.old_paths_map
						.insert(inode_and_device, (Instant::now(), path));
				}
			}
			Err(e) => return Err(e.into()),
		}

		Ok(())
	}
}
