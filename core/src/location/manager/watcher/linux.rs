//! Linux has the best behaving file system events, with just some small caveats:
//! When we move files or directories, we receive 3 events: Rename From, Rename To and Rename Both.
//! But when we move a file or directory to the outside from the watched location, we just receive
//! the Rename From event, so we have to keep track of all rename events to match them against each
//! other. If we have dangling Rename From events, we have to remove them after some time.
//! Aside from that, when a directory is moved to our watched location from the outside, we receive
//! a Create Dir event, this one is actually ok at least.

use crate::{invalidate_query, library::Library, location::manager::LocationManagerError, Node};

use sd_prisma::prisma::location;
use sd_utils::error::FileIOError;

use std::{
	collections::{BTreeMap, HashMap},
	path::{Path, PathBuf},
	sync::Arc,
};

use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, time::Instant};
use tracing::{error, instrument, trace};

use super::{
	utils::{create_dir, recalculate_directories_size, remove, rename, update_file},
	HUNDRED_MILLIS, ONE_SECOND,
};

#[derive(Debug)]
pub(super) struct EventHandler {
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	library: Arc<Library>,
	node: Arc<Node>,
	last_events_eviction_check: Instant,
	rename_from: HashMap<PathBuf, Instant>,
	recently_renamed_from: BTreeMap<PathBuf, Instant>,
	files_to_update: HashMap<PathBuf, Instant>,
	reincident_to_update_files: HashMap<PathBuf, Instant>,
	to_recalculate_size: HashMap<PathBuf, Instant>,

	path_and_instant_buffer: Vec<(PathBuf, Instant)>,
}

impl super::EventHandler for EventHandler {
	fn new(
		location_id: location::id::Type,
		location_pub_id: location::pub_id::Type,
		library: Arc<Library>,
		node: Arc<Node>,
	) -> Self
	where
		Self: Sized,
	{
		Self {
			location_id,
			location_pub_id,
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

	#[instrument(
		skip_all,
		fields(
			location_id = %self.location_id,
			library_id = %self.library.id,
			waiting_rename_count = %self.recently_renamed_from.len(),
			waiting_update_count = %self.files_to_update.len(),
			reincident_to_update_files_count = %self.reincident_to_update_files.len(),
			waiting_size_count = %self.to_recalculate_size.len(),
		),
	)]
	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		trace!("Received Linux event");

		let Event {
			kind, mut paths, ..
		} = event;

		match kind {
			EventKind::Create(CreateKind::File)
			| EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
				// When we receive a create, modify data or metadata events of the above kinds
				// we just mark the file to be updated in a near future
				// each consecutive event of these kinds that we receive for the same file
				// we just store the path again in the map below, with a new instant
				// that effectively resets the timer for the file to be updated
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

			EventKind::Create(CreateKind::Folder) => {
				let path = paths.remove(0);

				// Don't need to dispatch a recalculate directory event as `create_dir` dispatches
				// a `scan_location_sub_path` function, which recalculates the size already

				create_dir(
					self.location_id,
					&path,
					&fs::metadata(&path)
						.await
						.map_err(|e| FileIOError::from((&path, e)))?,
					&self.node,
					&self.library,
				)
				.await?;
			}

			EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
				// Just in case we can't guarantee that we receive the Rename From event before the
				// Rename Both event. Just a safeguard
				if self.recently_renamed_from.remove(&paths[0]).is_none() {
					self.rename_from.insert(paths.remove(0), Instant::now());
				}
			}

			EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
				let to_path = paths.remove(1);
				let from_path = paths.remove(0);

				self.rename_from.remove(&from_path);
				rename(
					self.location_id,
					&to_path,
					&from_path,
					fs::metadata(&to_path)
						.await
						.map_err(|e| FileIOError::from((&to_path, e)))?,
					&self.library,
				)
				.await?;

				self.recently_renamed_from.insert(from_path, Instant::now());
			}

			EventKind::Remove(_) => {
				let path = paths.remove(0);
				if let Some(parent) = path.parent() {
					if parent != Path::new("") {
						self.to_recalculate_size
							.insert(parent.to_path_buf(), Instant::now());
					}
				}

				remove(self.location_id, &path, &self.library).await?;
			}

			_ => {
				trace!("Other Linux event that we don't handle for now");
			}
		}

		Ok(())
	}

	async fn tick(&mut self) {
		if self.last_events_eviction_check.elapsed() > HUNDRED_MILLIS {
			if let Err(e) = self.handle_to_update_eviction().await {
				error!(
					?e,
					"Error while handling recently created or update files eviction;"
				);
			}

			if let Err(e) = self.handle_rename_from_eviction().await {
				error!(?e, "Failed to remove file_path;");
			}

			self.recently_renamed_from
				.retain(|_, instant| instant.elapsed() < HUNDRED_MILLIS);

			if !self.to_recalculate_size.is_empty() {
				if let Err(e) = recalculate_directories_size(
					&mut self.to_recalculate_size,
					&mut self.path_and_instant_buffer,
					self.location_id,
					self.location_pub_id.clone(),
					&self.library,
				)
				.await
				{
					error!(?e, "Failed to recalculate directories size;");
				}
			}

			self.last_events_eviction_check = Instant::now();
		}
	}
}

impl EventHandler {
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

				update_file(self.location_id, &path, &self.node, &self.library).await?;

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

				update_file(self.location_id, &path, &self.node, &self.library).await?;

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

				remove(self.location_id, &path, &self.library).await?;

				should_invalidate = true;

				trace!(path = %path.display(), "Removed file_path due timeout;");
			} else {
				self.path_and_instant_buffer.push((path, instant));
			}
		}

		if should_invalidate {
			invalidate_query!(self.library, "search.paths");
		}

		self.rename_from
			.extend(self.path_and_instant_buffer.drain(..));

		Ok(())
	}
}
