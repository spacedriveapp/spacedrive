//! Linux has the best behaving file system events, with just some small caveats:
//! When we move files or directories, we receive 3 events: Rename From, Rename To and Rename Both.
//! But when we move a file or directory to the outside from the watched location, we just receive
//! the Rename From event, so we have to keep track of all rename events to match them against each
//! other. If we have dangling Rename From events, we have to remove them after some time.
//! Aside from that, when a directory is moved to our watched location from the outside, we receive
//! a Create Dir event, this one is actually ok at least.

use crate::{
	invalidate_query, library::LoadedLibrary, location::manager::LocationManagerError,
	prisma::location, util::error::FileIOError, Node,
};

use std::{
	collections::{BTreeMap, HashMap},
	path::PathBuf,
	sync::Arc,
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, time::Instant};
use tracing::{error, trace};

use super::{
	utils::{create_dir, create_file, remove, rename, update_file},
	EventHandler, HUNDRED_MILLIS, ONE_SECOND,
};

#[derive(Debug)]
pub(super) struct LinuxEventHandler<'lib> {
	location_id: location::id::Type,
	library: &'lib Arc<LoadedLibrary>,
	node: &'lib Arc<Node>,
	last_check_rename: Instant,
	rename_from: HashMap<PathBuf, Instant>,
	rename_from_buffer: Vec<(PathBuf, Instant)>,
	recently_renamed_from: BTreeMap<PathBuf, Instant>,
	recently_created_files: BTreeMap<PathBuf, Instant>,
}

#[async_trait]
impl<'lib> EventHandler<'lib> for LinuxEventHandler<'lib> {
	fn new(
		location_id: location::id::Type,
		library: &'lib Arc<LoadedLibrary>,
		node: &'lib Arc<Node>,
	) -> Self {
		Self {
			location_id,
			library,
			node,
			last_check_rename: Instant::now(),
			rename_from: HashMap::new(),
			rename_from_buffer: Vec::new(),
			recently_renamed_from: BTreeMap::new(),
			recently_created_files: BTreeMap::new(),
		}
	}

	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		tracing::debug!("Received Linux event: {:#?}", event);

		let Event {
			kind, mut paths, ..
		} = event;

		match kind {
			EventKind::Create(CreateKind::File) => {
				let path = &paths[0];
				create_file(
					self.location_id,
					path,
					&fs::metadata(path)
						.await
						.map_err(|e| FileIOError::from((path, e)))?,
					self.node,
					self.library,
				)
				.await?;

				self.recently_created_files
					.insert(paths.remove(0), Instant::now());
			}
			EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
				// If a file was closed with write mode, then it was updated or created
				if !self.recently_created_files.contains_key(&paths[0]) {
					update_file(self.location_id, &paths[0], self.node, self.library).await?;
				}
			}
			EventKind::Create(CreateKind::Folder) => {
				let path = &paths[0];

				create_dir(
					self.location_id,
					path,
					&fs::metadata(path)
						.await
						.map_err(|e| FileIOError::from((path, e)))?,
					self.node,
					self.library,
				)
				.await?;
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
				// Just in case we can't garantee that we receive the Rename From event before the
				// Rename Both event. Just a safeguard
				if self.recently_renamed_from.remove(&paths[0]).is_none() {
					self.rename_from.insert(paths.remove(0), Instant::now());
				}
			}

			EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
				let from_path = &paths[0];
				let to_path = &paths[1];

				self.rename_from.remove(from_path);
				rename(
					self.location_id,
					to_path,
					from_path,
					fs::metadata(to_path)
						.await
						.map_err(|e| FileIOError::from((to_path, e)))?,
					self.library,
				)
				.await?;
				self.recently_renamed_from
					.insert(paths.swap_remove(0), Instant::now());
			}
			EventKind::Remove(_) => {
				remove(self.location_id, &paths[0], self.library).await?;
			}
			other_event_kind => {
				trace!("Other Linux event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}

	async fn tick(&mut self) {
		if self.last_check_rename.elapsed() > HUNDRED_MILLIS {
			self.last_check_rename = Instant::now();
			self.handle_rename_from_eviction().await;

			self.recently_renamed_from
				.retain(|_, instant| instant.elapsed() < HUNDRED_MILLIS);
			self.recently_created_files
				.retain(|_, created_at| created_at.elapsed() < ONE_SECOND);
		}
	}
}

impl LinuxEventHandler<'_> {
	async fn handle_rename_from_eviction(&mut self) {
		self.rename_from_buffer.clear();

		for (path, instant) in self.rename_from.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if let Err(e) = remove(self.location_id, &path, self.library).await {
					error!("Failed to remove file_path: {e}");
				} else {
					trace!("Removed file_path due timeout: {}", path.display());
					invalidate_query!(self.library, "search.paths");
				}
			} else {
				self.rename_from_buffer.push((path, instant));
			}
		}

		for (path, instant) in self.rename_from_buffer.drain(..) {
			self.rename_from.insert(path, instant);
		}
	}
}
