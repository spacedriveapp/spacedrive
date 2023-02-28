use crate::{
	library::Library,
	location::{file_path_helper::get_existing_file_or_directory, location_with_indexer_rules},
};

use std::{
	collections::{hash_map::DefaultHasher, BTreeMap},
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	time::Duration,
};

use async_trait::async_trait;
use futures::{stream::FuturesUnordered, StreamExt};
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{
	fs,
	runtime::Handle,
	select,
	sync::{mpsc, oneshot},
	task::{block_in_place, JoinHandle},
	time::{sleep, Instant},
};
use tracing::{error, trace};

use super::{
	utils::{
		create_dir, create_dir_by_path, create_file, create_file_by_path, remove_by_file_path,
		remove_event, rename, update_file,
	},
	EventHandler, LocationManagerError,
};

const ONE_SECOND: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(super) struct MacOsEventHandler {
	recently_created_files: BTreeMap<PathBuf, Instant>,
	last_check: Instant,
	latest_created_dir: Option<Event>,
	rename_events_tx: mpsc::Sender<(location_with_indexer_rules::Data, PathBuf, Library)>,
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

async fn handle_rename_events_loop(
	mut rename_events_rx: mpsc::Receiver<(location_with_indexer_rules::Data, PathBuf, Library)>,
	mut stop_rx: oneshot::Receiver<()>,
) {
	// Organizing locations, paths and library contexts by path's hash, so we can easy share
	let mut paths_by_hash = BTreeMap::new();
	let mut last_path_hash = None;
	let mut timeouts = FuturesUnordered::new();

	loop {
		select! {
			_ = &mut stop_rx => {
				break;
			}
			Some((location, path, library_ctx)) = rename_events_rx.recv() => {
				trace!("Received rename event for path: {}", path.display());
				if let Some(path_hash) = last_path_hash.take() {
					// SAFETY: If we have a `path_hash` in the Option,
					// it's because we put it in the hashmap
					let (location, old_path, library_ctx) = paths_by_hash.remove(&path_hash).unwrap();

					// We received 2 rename events in an interval smaller then 100ms
					// this is actually a rename or move operation
					if let Err(e) = rename(path, old_path, &location, &library_ctx).await {
						error!("Failed to rename file: {e}");
					}

				} else {
					let mut hasher = DefaultHasher::new();
					path.hash(&mut hasher);
					let path_hash = hasher.finish();

					paths_by_hash.insert(path_hash, (location, path, library_ctx));
					timeouts.push(timeout(path_hash));
					last_path_hash = Some(path_hash);
				}
			}
			Some(path_hash) = timeouts.next() => {
				trace!("Timeout for path_hash: {path_hash}");
				// We need this if let here because the path can already be handled by
				// the other `select!` branch
				if let Some((location, path, library)) = paths_by_hash.remove(&path_hash) {
					last_path_hash = None;
					trace!("Path: {}", path.display());

					if let Err(e) = handle_create_or_delete(&location, path, &library).await {
						error!("Failed to handle create or delete event: {e:#?}");
					}
				}
			}
		}
	}
}

async fn timeout(path_hash: u64) -> u64 {
	sleep(Duration::from_millis(100)).await;
	path_hash
}

async fn handle_create_or_delete(
	location: &location_with_indexer_rules::Data,
	path: impl AsRef<Path>,
	library: &Library,
) -> Result<(), LocationManagerError> {
	let path = path.as_ref();
	if let Some(file_path) = get_existing_file_or_directory(location, path, &library.db).await? {
		remove_by_file_path(location, path, &file_path, library).await?;
	} else if fs::metadata(path).await?.is_dir() {
		create_dir_by_path(location, path, library).await?;
	} else {
		create_file_by_path(location, path, library).await?;
	}

	Ok(())
}
