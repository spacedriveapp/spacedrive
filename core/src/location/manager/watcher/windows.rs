//! Windows file system event handler implementation has some caveats die to how
//! file system events are emitted on Windows.
//!
//! For example: When a file is moved to another
//! directory, we receive a remove event and then a create event, so to avoid having to actually
//! remove and create the `file_path` in the database, we have to wait some time after receiving
//! a remove event to see if a create event is emitted. If it is, we just update the `file_path`
//! in the database. If not, we remove the file from the database.

use crate::{
	invalidate_query,
	library::Library,
	location::{file_path_helper::get_inode_and_device_from_path, manager::LocationManagerError},
	prisma::location,
	util::error::FileIOError,
	Node,
};

use std::{
	collections::{BTreeMap, HashMap},
	path::PathBuf,
	sync::Arc,
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, time::Instant};
use tracing::{error, trace};

use super::{
	utils::{create_dir_or_file, extract_inode_and_device_from_path, remove, rename, update_file},
	EventHandler, INodeAndDevice, InstantAndPath, HUNDRED_MILLIS, ONE_SECOND,
};

/// Windows file system event handler
#[derive(Debug)]
pub(super) struct WindowsEventHandler<'lib> {
	location_id: location::id::Type,
	library: &'lib Arc<Library>,
	node: &'lib Arc<Node>,
	last_check_recently_files: Instant,
	recently_created_files: BTreeMap<PathBuf, Instant>,
	last_check_rename_and_remove: Instant,
	rename_from_map: BTreeMap<INodeAndDevice, InstantAndPath>,
	rename_to_map: BTreeMap<INodeAndDevice, InstantAndPath>,
	to_remove_files: HashMap<INodeAndDevice, InstantAndPath>,
	removal_buffer: Vec<(INodeAndDevice, InstantAndPath)>,
}

#[async_trait]
impl<'lib> EventHandler<'lib> for WindowsEventHandler<'lib> {
	fn new(
		location_id: location::id::Type,
		library: &'lib Arc<Library>,
		node: &'lib Arc<Node>,
	) -> Self
	where
		Self: Sized,
	{
		Self {
			location_id,
			library,
			node,
			last_check_recently_files: Instant::now(),
			recently_created_files: BTreeMap::new(),
			last_check_rename_and_remove: Instant::now(),
			rename_from_map: BTreeMap::new(),
			rename_to_map: BTreeMap::new(),
			to_remove_files: HashMap::new(),
			removal_buffer: Vec::new(),
		}
	}

	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		trace!("Received Windows event: {:#?}", event);
		let Event {
			kind, mut paths, ..
		} = event;

		match kind {
			EventKind::Create(CreateKind::Any) => {
				let inode_and_device = get_inode_and_device_from_path(&paths[0]).await?;

				if let Some((_, old_path)) = self.to_remove_files.remove(&inode_and_device) {
					// if previously we added a file to be removed with the same inode and device
					// of this "newly created" created file, it means that the file was just moved to another location
					// so we can treat if just as a file rename, like in other OSes

					trace!(
						"Got a rename instead of remove/create: {} -> {}",
						old_path.display(),
						paths[0].display(),
					);

					// We found a new path for this old path, so we can rename it instead of removing and creating it
					rename(
						self.location_id,
						&paths[0],
						&old_path,
						fs::metadata(&paths[0])
							.await
							.map_err(|e| FileIOError::from((&paths[0], e)))?,
						self.library,
					)
					.await?;
				} else {
					let metadata =
						create_dir_or_file(self.location_id, &paths[0], self.node, self.library)
							.await?;

					if metadata.is_file() {
						self.recently_created_files
							.insert(paths.remove(0), Instant::now());
					}
				}
			}
			EventKind::Modify(ModifyKind::Any) => {
				let path = &paths[0];
				// Windows emite events of update right after create events
				if !self.recently_created_files.contains_key(path) {
					let metadata = fs::metadata(path)
						.await
						.map_err(|e| FileIOError::from((path, e)))?;
					if metadata.is_file() {
						update_file(self.location_id, path, self.node, self.library).await?;
					}
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
				let path = paths.remove(0);

				let inode_and_device =
					extract_inode_and_device_from_path(self.location_id, &path, self.library)
						.await?;

				if let Some((_, new_path)) = self.rename_to_map.remove(&inode_and_device) {
					// We found a new path for this old path, so we can rename it
					rename(
						self.location_id,
						&new_path,
						&path,
						fs::metadata(&new_path)
							.await
							.map_err(|e| FileIOError::from((&new_path, e)))?,
						self.library,
					)
					.await?;
				} else {
					self.rename_from_map
						.insert(inode_and_device, (Instant::now(), path));
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
				let path = paths.remove(0);

				let inode_and_device =
					extract_inode_and_device_from_path(self.location_id, &path, self.library)
						.await?;

				if let Some((_, old_path)) = self.rename_to_map.remove(&inode_and_device) {
					// We found a old path for this new path, so we can rename it
					rename(
						self.location_id,
						&path,
						&old_path,
						fs::metadata(&path)
							.await
							.map_err(|e| FileIOError::from((&path, e)))?,
						self.library,
					)
					.await?;
				} else {
					self.rename_from_map
						.insert(inode_and_device, (Instant::now(), path));
				}
			}
			EventKind::Remove(_) => {
				let path = paths.remove(0);
				self.to_remove_files.insert(
					extract_inode_and_device_from_path(self.location_id, &path, self.library)
						.await?,
					(Instant::now(), path),
				);
			}

			other_event_kind => {
				trace!("Other Windows event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}

	async fn tick(&mut self) {
		// Cleaning out recently created files that are older than 1 second
		if self.last_check_recently_files.elapsed() > ONE_SECOND {
			self.last_check_recently_files = Instant::now();
			self.recently_created_files
				.retain(|_, created_at| created_at.elapsed() < ONE_SECOND);
		}

		if self.last_check_rename_and_remove.elapsed() > HUNDRED_MILLIS {
			self.last_check_rename_and_remove = Instant::now();
			self.rename_from_map.retain(|_, (created_at, path)| {
				let to_retain = created_at.elapsed() < HUNDRED_MILLIS;
				if !to_retain {
					trace!("Removing from rename from map: {:#?}", path.display())
				}
				to_retain
			});
			self.rename_to_map.retain(|_, (created_at, path)| {
				let to_retain = created_at.elapsed() < HUNDRED_MILLIS;
				if !to_retain {
					trace!("Removing from rename to map: {:#?}", path.display())
				}
				to_retain
			});
			self.handle_removes_eviction().await;
		}
	}
}

impl WindowsEventHandler<'_> {
	async fn handle_removes_eviction(&mut self) {
		self.removal_buffer.clear();

		for (inode_and_device, (instant, path)) in self.to_remove_files.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if let Err(e) = remove(self.location_id, &path, self.library).await {
					error!("Failed to remove file_path: {e}");
				} else {
					trace!("Removed file_path due timeout: {}", path.display());
					invalidate_query!(self.library, "search.paths");
				}
			} else {
				self.removal_buffer
					.push((inode_and_device, (instant, path)));
			}
		}

		for (key, value) in self.removal_buffer.drain(..) {
			self.to_remove_files.insert(key, value);
		}
	}
}
