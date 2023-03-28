use crate::{
	invalidate_query,
	library::Library,
	location::{
		file_path_helper::{check_existing_file_path, get_inode_and_device, MaterializedPath},
		location_with_indexer_rules,
	},
};

use std::{
	collections::{BTreeMap, HashMap},
	path::PathBuf,
	time::Duration,
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{
	fs, io,
	runtime::Handle,
	select,
	sync::{mpsc, oneshot},
	task::{block_in_place, JoinHandle},
	time::{interval_at, Instant, MissedTickBehavior},
};
use tracing::{error, trace, warn};

use super::{
	utils::{
		create_dir, create_dir_or_file_by_path, create_file, extract_inode_and_device_from_path,
		remove_by_path, remove_event, rename, update_file,
	},
	EventHandler, INodeAndDevice, InstantLocationPathAndLibrary, LocationManagerError,
	LocationPathAndLibrary,
};

const ONE_SECOND: Duration = Duration::from_secs(1);
const HUNDRED_MILLIS: Duration = Duration::from_millis(100);

enum CreateOrDelete {
	Create,
	Delete,
}

#[derive(Debug)]
pub(super) struct MacOsEventHandler {
	recently_created_files: BTreeMap<PathBuf, Instant>,
	last_check: Instant,
	latest_created_dir: Option<Event>,
	rename_events_tx: mpsc::Sender<LocationPathAndLibrary>,
	stop_tx: Option<oneshot::Sender<()>>,
	handle: Option<JoinHandle<()>>,
}

impl Drop for MacOsEventHandler {
	fn drop(&mut self) {
		if let Some(stop_tx) = self.stop_tx.take() {
			if stop_tx.send(()).is_err() {
				error!("Failed to send stop signal to MacOS rename event handler");
			}
			// FIXME: change this Drop to async drop in the future
			if let Some(handle) = self.handle.take() {
				if let Err(e) =
					block_in_place(move || Handle::current().block_on(async move { handle.await }))
				{
					error!("Failed to join watcher task: {e:#?}")
				}
			}
		}
	}
}

#[async_trait]
impl EventHandler for MacOsEventHandler {
	fn new() -> Self
	where
		Self: Sized,
	{
		let (stop_tx, stop_rx) = oneshot::channel();
		let (rename_events_tx, rename_events_rx) = mpsc::channel(16);

		Self {
			recently_created_files: BTreeMap::new(),
			last_check: Instant::now(),
			latest_created_dir: None,
			rename_events_tx,
			stop_tx: Some(stop_tx),
			handle: Some(tokio::spawn(handle_rename_events_loop(
				rename_events_rx,
				stop_rx,
			))),
		}
	}

	async fn handle_event(
		&mut self,
		location: location_with_indexer_rules::Data,
		library: &Library,
		event: Event,
	) -> Result<(), LocationManagerError> {
		trace!("Received MacOS event: {:#?}", event);

		match event.kind {
			EventKind::Create(CreateKind::Folder) => {
				if let Some(latest_created_dir) = self.latest_created_dir.take() {
					if event.paths[0] == latest_created_dir.paths[0] {
						// NOTE: This is a MacOS specific event that happens when a folder is created
						// trough Finder. It creates a folder but 2 events are triggered in
						// FSEvents. So we store and check the latest created folder to avoid
						// hiting a unique constraint in the database
						return Ok(());
					}
				}

				create_dir(&location, &event, library).await?;
				self.latest_created_dir = Some(event);
			}
			EventKind::Create(CreateKind::File) => {
				create_file(&location, &event, library).await?;
				let Event { mut paths, .. } = event;
				self.recently_created_files
					.insert(paths.remove(0), Instant::now());
			}
			EventKind::Modify(ModifyKind::Data(DataChange::Content)) => {
				// NOTE: MacOS emits a Create File and then an Update Content event
				// when a file is created. So we need to check if the file was recently
				// created to avoid unecessary updates
				if !self.recently_created_files.contains_key(&event.paths[0]) {
					update_file(&location, &event, library).await?;
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
				if self
					.rename_events_tx
					.send((location, event.paths[0].clone(), library.clone()))
					.await
					.is_err()
				{
					error!("Failed to send rename event to be handled: event {event:#?}");
				}
			}

			EventKind::Remove(remove_kind) => {
				remove_event(&location, &event, remove_kind, library).await?;
			}
			other_event_kind => {
				trace!("Other MacOS event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		// Cleaning out recently created files that are older than 1 second
		if self.last_check.elapsed() > ONE_SECOND {
			self.last_check = Instant::now();
			self.recently_created_files
				.retain(|_, created_at| created_at.elapsed() < ONE_SECOND);
		}

		Ok(())
	}
}

/// Rename events on MacOS using FSEvents backend are pretty complicated;
/// There are just (ModifyKind::Name(RenameMode::Any) events and nothing else.
/// This means that we have to link the old path with the new path to know which file was renamed.
/// But you can't forget that renames events aren't always the case that I file name was modified,
/// but its path was modified. So we have to check if the file was moved. When a file is moved
/// inside the same location, we received 2 events: one for the old path and one for the new path.
/// But when a file is moved to another location, we only receive the old path event... This
/// way we have to handle like a file deletion, and the same applies for when a file is moved to our
/// current location from anywhere else, we just receive the new path rename event, which means a
/// creation.
async fn handle_rename_events_loop(
	mut rename_events_rx: mpsc::Receiver<LocationPathAndLibrary>,
	mut stop_rx: oneshot::Receiver<()>,
) {
	let mut old_paths_map = HashMap::new();
	let mut new_paths_map = HashMap::new();

	// Using this buffer to not reallocate memory for every cleanup
	let mut maps_buffer = vec![];

	let mut cleaning_interval = interval_at(Instant::now() + HUNDRED_MILLIS, HUNDRED_MILLIS);
	// In case of doubt check: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html
	cleaning_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

	loop {
		select! {
			_ = &mut stop_rx => {
				break;
			}
			Some((location, path, library)) = rename_events_rx.recv() => {
				trace!("Received rename event for path: {}", path.display());
				if let Err(e) = handle_single_rename_event(
					location,
					path,
					library,
					&mut old_paths_map,
					&mut new_paths_map,
				).await {
					error!("Failed to handle rename event: {e}");
				}
			}
			_ = cleaning_interval.tick() => {
				// Cleaning out recently renamed files that are older than 2 seconds
				clear_paths_map(&mut old_paths_map, &mut maps_buffer, CreateOrDelete::Delete).await;
				clear_paths_map(&mut new_paths_map, &mut maps_buffer, CreateOrDelete::Create).await;
			}
		}
	}
}

async fn clear_paths_map(
	paths_map: &mut HashMap<INodeAndDevice, InstantLocationPathAndLibrary>,
	temp_buffer: &mut Vec<(INodeAndDevice, InstantLocationPathAndLibrary)>,
	create_or_delete: CreateOrDelete,
) {
	// Just to make sure that our buffer is clean
	temp_buffer.clear();

	for (created_at, (instant, (location, path, library))) in paths_map.drain() {
		if instant.elapsed() > HUNDRED_MILLIS {
			let mut flag = false;
			match create_or_delete {
				CreateOrDelete::Create => {
					if let Err(e) = create_dir_or_file_by_path(&location, &path, &library).await {
						error!("Failed to create file_path on MacOS : {e}");
					} else {
						trace!("Created file_path due timeout: {}", path.display());
						flag = true;
					}
				}
				CreateOrDelete::Delete => {
					if let Err(e) = remove_by_path(&location, &path, &library).await {
						error!("Failed to remove file_path: {e}");
					} else {
						trace!("Removed file_path due timeout: {}", path.display());
						flag = true;
					}
				}
			}

			if flag {
				invalidate_query!(&library, "locations.getExplorerData");
			}
		} else {
			temp_buffer.push((created_at, (instant, (location, path, library))));
		}
	}

	for (key, value) in temp_buffer.drain(..) {
		paths_map.insert(key, value);
	}
}

async fn handle_single_rename_event(
	location: location_with_indexer_rules::Data,
	path: PathBuf, // this is used internally only once, so we can use just PathBuf
	library: Library,
	old_paths_map: &mut HashMap<INodeAndDevice, InstantLocationPathAndLibrary>,
	new_paths_map: &mut HashMap<INodeAndDevice, InstantLocationPathAndLibrary>,
) -> Result<(), LocationManagerError> {
	match fs::metadata(&path).await {
		Ok(meta) => {
			// File or directory exists, so this can be a "new path" to an actual rename/move or a creation
			trace!("Path exists: {}", path.display());

			let inode_and_device = get_inode_and_device(&meta)?;

			if !check_existing_file_path(
				&MaterializedPath::new(location.id, &location.path, &path, meta.is_dir())?,
				&library.db,
			)
			.await?
			{
				if let Some((_, (old_path_location, old_path, old_path_library))) =
					old_paths_map.remove(&inode_and_device)
				{
					// Just to make sure we're not doing anything wrong
					assert_eq!(location.id, old_path_location.id);
					assert_eq!(library.id, old_path_library.id);

					trace!(
						"Got a match new -> old: {} -> {}",
						path.display(),
						old_path.display()
					);

					// We found a new path for this old path, so we can rename it
					rename(&path, &old_path, &location, &library).await?;
				} else {
					trace!("No match for new path yet: {}", path.display());
					new_paths_map.insert(
						inode_and_device,
						(Instant::now(), (location, path, library)),
					);
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
				extract_inode_and_device_from_path(&location, &path, &library).await?;

			if let Some((_, (new_path_location, new_path, new_path_library))) =
				new_paths_map.remove(&inode_and_device)
			{
				// Just to make sure we're not doing anything wrong
				assert_eq!(location.id, new_path_location.id);
				assert_eq!(library.id, new_path_library.id);

				trace!(
					"Got a match old -> new: {} -> {}",
					path.display(),
					new_path.display()
				);

				// We found a new path for this old path, so we can rename it
				rename(&new_path, &path, &location, &library).await?;
			} else {
				trace!("No match for old path yet: {}", path.display());
				// We didn't find a new path for this old path, so we store ir for later
				old_paths_map.insert(
					inode_and_device,
					(Instant::now(), (location, path, library)),
				);
			}
		}
		Err(e) => return Err(e.into()),
	}

	Ok(())
}
