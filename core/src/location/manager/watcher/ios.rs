//! iOS file system watcher implementation.

use crate::{invalidate_query, library::Library, location::manager::LocationManagerError, Node};

use sd_prisma::prisma::location;
use sd_utils::error::FileIOError;

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, MetadataKind, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, time::Instant};
use tracing::{debug, error, trace};

use super::{
	utils::{create_dir, create_file, recalculate_directories_size, remove, rename, update_file},
	EventHandler, INode, InstantAndPath, HUNDRED_MILLIS, ONE_SECOND,
};

#[derive(Debug)]
pub(super) struct IosEventHandler<'lib> {
	location_id: location::id::Type,
	library: &'lib Arc<Library>,
	node: &'lib Arc<Node>,
	files_to_update: HashMap<PathBuf, Instant>,
	reincident_to_update_files: HashMap<PathBuf, Instant>,
	last_events_eviction_check: Instant,
	latest_created_dir: Option<PathBuf>,
	old_paths_map: HashMap<INode, InstantAndPath>,
	new_paths_map: HashMap<INode, InstantAndPath>,
	paths_map_buffer: Vec<(INode, InstantAndPath)>,
	to_recalculate_size: HashMap<PathBuf, Instant>,
	path_and_instant_buffer: Vec<(PathBuf, Instant)>,
	rename_event_queue: HashMap<PathBuf, Instant>,
}

#[async_trait]
impl<'lib> EventHandler<'lib> for IosEventHandler<'lib> {
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
			files_to_update: HashMap::new(),
			reincident_to_update_files: HashMap::new(),
			last_events_eviction_check: Instant::now(),
			latest_created_dir: None,
			old_paths_map: HashMap::new(),
			new_paths_map: HashMap::new(),
			rename_event_queue: HashMap::new(),
			paths_map_buffer: Vec::new(),
			to_recalculate_size: HashMap::new(),
			path_and_instant_buffer: Vec::new(),
		}
	}

	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		let Event {
			kind, mut paths, ..
		} = event;

		match kind {
			EventKind::Create(CreateKind::Folder) => {
				// If a folder creation event is received, handle it as usual
				let path = paths.remove(0);

				self.rename_event_queue.insert(path.clone(), Instant::now());

				create_dir(
					self.location_id,
					&path,
					&fs::metadata(&path).await.map_err(|e| {
						FileIOError::from((
							&path,
							e,
							"Failed to extract metadata of newly create directory in the watcher",
						))
					})?,
					self.node,
					self.library,
				)
				.await?;

				self.latest_created_dir = Some(path);
			}

			EventKind::Create(CreateKind::File)
			| EventKind::Modify(ModifyKind::Data(DataChange::Content))
			| EventKind::Modify(ModifyKind::Metadata(
				MetadataKind::WriteTime | MetadataKind::Extended,
			)) => {
				// When we receive a create, modify data or metadata events of the abore kinds
				// we just mark the file to be updated in a near future
				// each consecutive event of these kinds that we receive for the same file
				// we just store the path again in the map below, with a new instant
				// that effectively resets the timer for the file to be updated <- Copied from macos.rs
				let path = paths.remove(0);
				self.rename_event_queue.insert(path.clone(), Instant::now());
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

			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
				self.handle_single_rename_event(paths.remove(0)).await?;
			}

			// For some reason, iOS doesn't have a Delete Event, so the vent type comes up as this.
			// Delete Event
			EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)) => {
				debug!("File has been deleted: {:#?}", paths);
				let path = paths.remove(0);
				if let Some(parent) = path.parent() {
					if parent != Path::new("") {
						self.to_recalculate_size
							.insert(parent.to_path_buf(), Instant::now());
					}
				}
				remove(self.location_id, &path, self.library).await?; //FIXME: Find out why this freezes the watcher
			}
			other_event_kind => {
				trace!("Other iOS event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}

	async fn tick(&mut self) {
		if self.last_events_eviction_check.elapsed() > HUNDRED_MILLIS {
			if let Err(e) = self.handle_to_update_eviction().await {
				error!("Error while handling recently created or update files eviction: {e:#?}");
			}

			// Cleaning out recently renamed files that are older than 100 milliseconds
			if let Err(e) = self.handle_rename_create_eviction().await {
				error!("Failed to create file_path on iOS : {e:#?}");
			}

			if let Err(e) = self.handle_rename_remove_eviction().await {
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

impl IosEventHandler<'_> {
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

	async fn handle_rename_create_eviction(&mut self) -> Result<(), LocationManagerError> {
		// Just to make sure that our buffer is clean
		self.paths_map_buffer.clear();
		let mut should_invalidate = false;

		for (inode, (instant, path)) in self.new_paths_map.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if !self.files_to_update.contains_key(&path) {
					let metadata = fs::metadata(&path)
						.await
						.map_err(|e| FileIOError::from((&path, e)))?;

					if metadata.is_dir() {
						// Don't need to dispatch a recalculate directory event as `create_dir` dispatches
						// a `scan_location_sub_path` function, which recalculates the size already
						create_dir(self.location_id, &path, &metadata, self.node, self.library)
							.await?;
					} else {
						if let Some(parent) = path.parent() {
							if parent != Path::new("") {
								self.to_recalculate_size
									.insert(parent.to_path_buf(), Instant::now());
							}
						}
						create_file(self.location_id, &path, &metadata, self.node, self.library)
							.await?;
					}

					trace!("Created file_path due timeout: {}", path.display());
					should_invalidate = true;
				}
			} else {
				self.paths_map_buffer.push((inode, (instant, path)));
			}
		}

		if should_invalidate {
			invalidate_query!(self.library, "search.paths");
		}

		self.new_paths_map.extend(self.paths_map_buffer.drain(..));

		Ok(())
	}

	async fn handle_rename_remove_eviction(&mut self) -> Result<(), LocationManagerError> {
		// Just to make sure that our buffer is clean
		self.paths_map_buffer.clear();
		let mut should_invalidate = false;

		for (inode, (instant, path)) in self.old_paths_map.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if let Some(parent) = path.parent() {
					if parent != Path::new("") {
						self.to_recalculate_size
							.insert(parent.to_path_buf(), Instant::now());
					}
				}
				remove(self.location_id, &path, self.library).await?;
				trace!("Removed file_path due timeout: {}", path.display());
				should_invalidate = true;
			} else {
				self.paths_map_buffer.push((inode, (instant, path)));
			}
		}

		if should_invalidate {
			invalidate_query!(self.library, "search.paths");
		}

		self.old_paths_map.extend(self.paths_map_buffer.drain(..));

		Ok(())
	}

	// Thanks iOS for having fun event types that we have to handle in a special way
	async fn handle_single_rename_event(
		&mut self,
		path: PathBuf,
	) -> Result<(), LocationManagerError> {
		if let Some((key, _)) = self.rename_event_queue.iter().nth(0) {
			let new_path_name = Path::new(key).file_name().unwrap();
			let new_path_name_string = Some(new_path_name.to_str().unwrap().to_string());

			rename(
				self.location_id,
				&key,
				&path,
				fs::metadata(&key)
					.await
					.map_err(|e| FileIOError::from((&key, e)))?,
				self.library,
			)
			.await?;

			// Remove the path from the rename event queue
			self.rename_event_queue.remove(&key.clone());

			debug!("Updated location name: {:#?}", new_path_name_string.clone());
		} else {
			error!("HashMap is empty or index out of bounds");
		}

		Ok(())
	}
}
