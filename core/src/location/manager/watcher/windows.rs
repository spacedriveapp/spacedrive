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
	location::{
		file_path_helper::{get_inode_from_path, FilePathError},
		manager::LocationManagerError,
	},
	prisma::location,
	util::error::FileIOError,
	Node,
};

use std::{
	collections::{BTreeMap, HashMap},
	path::{Path, PathBuf},
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
	utils::{
		create_dir, extract_inode_from_path, recalculate_directories_size, remove, rename,
		update_file,
	},
	EventHandler, INode, InstantAndPath, HUNDRED_MILLIS, ONE_SECOND,
};

/// Windows file system event handler
#[derive(Debug)]
pub(super) struct WindowsEventHandler<'lib> {
	location_id: location::id::Type,
	library: &'lib Arc<Library>,
	node: &'lib Arc<Node>,
	last_events_eviction_check: Instant,
	rename_from_map: BTreeMap<INode, InstantAndPath>,
	rename_to_map: BTreeMap<INode, InstantAndPath>,
	files_to_remove: HashMap<INode, InstantAndPath>,
	files_to_remove_buffer: Vec<(INode, InstantAndPath)>,
	files_to_update: HashMap<PathBuf, Instant>,
	reincident_to_update_files: HashMap<PathBuf, Instant>,
	to_recalculate_size: HashMap<PathBuf, Instant>,
	path_and_instant_buffer: Vec<(PathBuf, Instant)>,
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
			last_events_eviction_check: Instant::now(),
			rename_from_map: BTreeMap::new(),
			rename_to_map: BTreeMap::new(),
			files_to_remove: HashMap::new(),
			files_to_remove_buffer: Vec::new(),
			files_to_update: HashMap::new(),
			reincident_to_update_files: HashMap::new(),
			to_recalculate_size: HashMap::new(),
			path_and_instant_buffer: Vec::new(),
		}
	}

	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		trace!("Received Windows event: {:#?}", event);
		let Event {
			kind, mut paths, ..
		} = event;

		match kind {
			EventKind::Create(CreateKind::Any) => {
				let inode = match get_inode_from_path(&paths[0]).await {
					Ok(inode) => inode,
					Err(FilePathError::FileIO(FileIOError { source, .. }))
						if source.raw_os_error() == Some(32) =>
					{
						// This is still being manipulated by another process, so we can just ignore it for now
						// as we will probably receive update events later
						self.files_to_update.insert(paths.remove(0), Instant::now());

						return Ok(());
					}
					Err(e) => {
						return Err(e.into());
					}
				};

				if let Some((_, old_path)) = self.files_to_remove.remove(&inode) {
					// if previously we added a file to be removed with the same inode
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
					let path = paths.remove(0);
					let metadata = fs::metadata(&path)
						.await
						.map_err(|e| FileIOError::from((&path, e)))?;

					if metadata.is_dir() {
						// Don't need to dispatch a recalculate directory event as `create_dir` dispatches
						// a `scan_location_sub_path` function, which recalculates the size already
						create_dir(self.location_id, path, &metadata, self.node, self.library)
							.await?;
					} else if self.files_to_update.contains_key(&path) {
						if let Some(old_instant) =
							self.files_to_update.insert(path.clone(), Instant::now())
						{
							self.reincident_to_update_files
								.entry(path)
								.or_insert(old_instant);
						}
					} else {
						self.files_to_update.insert(path, Instant::now());
					}
				}
			}
			EventKind::Modify(ModifyKind::Any) => {
				let path = paths.remove(0);
				if self.files_to_update.contains_key(&path) {
					if let Some(old_instant) =
						self.files_to_update.insert(path.clone(), Instant::now())
					{
						self.reincident_to_update_files
							.entry(path)
							.or_insert(old_instant);
					}
				} else {
					self.files_to_update.insert(path, Instant::now());
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
				let path = paths.remove(0);

				let inode = extract_inode_from_path(self.location_id, &path, self.library).await?;

				if let Some((_, new_path)) = self.rename_to_map.remove(&inode) {
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
					self.rename_from_map.insert(inode, (Instant::now(), path));
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
				let path = paths.remove(0);

				let inode = get_inode_from_path(&path).await?;

				if let Some((_, old_path)) = self.rename_from_map.remove(&inode) {
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
					self.rename_to_map.insert(inode, (Instant::now(), path));
				}
			}
			EventKind::Remove(_) => {
				let path = paths.remove(0);
				self.files_to_remove.insert(
					extract_inode_from_path(self.location_id, &path, self.library).await?,
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
		if self.last_events_eviction_check.elapsed() > HUNDRED_MILLIS {
			if let Err(e) = self.handle_to_update_eviction().await {
				error!("Error while handling recently created or update files eviction: {e:#?}");
			}

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

			if let Err(e) = self.handle_removes_eviction().await {
				error!("Failed to remove file_path: {e:#?}");
			}

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

impl WindowsEventHandler<'_> {
	async fn handle_to_update_eviction(&mut self) -> Result<(), LocationManagerError> {
		self.path_and_instant_buffer.clear();
		let mut should_invalidate = false;

		for (path, created_at) in self.files_to_update.drain() {
			if created_at.elapsed() < HUNDRED_MILLIS * 5 {
				self.path_and_instant_buffer.push((path, created_at));
			} else {
				self.reincident_to_update_files.remove(&path);
				handle_update(
					self.location_id,
					&path,
					self.node,
					&mut self.to_recalculate_size,
					self.library,
				)
				.await?;
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
				self.files_to_update.remove(&path);
				handle_update(
					self.location_id,
					&path,
					self.node,
					&mut self.to_recalculate_size,
					self.library,
				)
				.await?;
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

	async fn handle_removes_eviction(&mut self) -> Result<(), LocationManagerError> {
		self.files_to_remove_buffer.clear();
		let mut should_invalidate = false;

		for (inode, (instant, path)) in self.files_to_remove.drain() {
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
				self.files_to_remove_buffer.push((inode, (instant, path)));
			}
		}
		if should_invalidate {
			invalidate_query!(self.library, "search.paths");
		}

		for (key, value) in self.files_to_remove_buffer.drain(..) {
			self.files_to_remove.insert(key, value);
		}

		Ok(())
	}
}

async fn handle_update<'lib>(
	location_id: location::id::Type,
	path: &PathBuf,
	node: &'lib Arc<Node>,
	to_recalculate_size: &mut HashMap<PathBuf, Instant>,
	library: &'lib Arc<Library>,
) -> Result<(), LocationManagerError> {
	let metadata = fs::metadata(&path)
		.await
		.map_err(|e| FileIOError::from((&path, e)))?;
	if metadata.is_file() {
		if let Some(parent) = path.parent() {
			if parent != Path::new("") {
				to_recalculate_size.insert(parent.to_path_buf(), Instant::now());
			}
		}
		update_file(location_id, path, node, library).await?;
	}

	Ok(())
}
