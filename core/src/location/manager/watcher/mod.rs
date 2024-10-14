use crate::{library::Library, Node};

use sd_core_indexer_rules::{IndexerRule, IndexerRuler};
use sd_core_prisma_helpers::{location_ids_and_path, location_with_indexer_rules};

use sd_prisma::prisma::{location, PrismaClient};
use sd_utils::db::maybe_missing;

use std::{
	collections::HashSet,
	future::Future,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::stream::Merge;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::{
	spawn,
	task::JoinHandle,
	time::{interval_at, Instant, MissedTickBehavior},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, instrument, trace, warn, Instrument};
use uuid::Uuid;

use super::LocationManagerError;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "android")]
mod android;

mod utils;

use utils::reject_event;

#[cfg(target_os = "linux")]
type Handler = linux::EventHandler;

#[cfg(target_os = "macos")]
type Handler = macos::EventHandler;

#[cfg(target_os = "windows")]
type Handler = windows::EventHandler;

#[cfg(target_os = "android")]
type Handler = android::EventHandler;

#[cfg(target_os = "ios")]
type Handler = ios::EventHandler;

pub(super) type IgnorePath = (PathBuf, bool);

type INode = u64;
#[allow(dead_code)] // this is not dead code, it's used with the TS bindings
type InstantAndPath = (Instant, PathBuf);

const ONE_SECOND: Duration = Duration::from_secs(1);
const THIRTY_SECONDS: Duration = Duration::from_secs(30);
const HUNDRED_MILLIS: Duration = Duration::from_millis(100);

trait EventHandler: 'static {
	fn new(location_id: location::id::Type, library: Arc<Library>, node: Arc<Node>) -> Self
	where
		Self: Sized;

	/// Handle a file system event.
	fn handle_event(
		&mut self,
		event: Event,
	) -> impl Future<Output = Result<(), LocationManagerError>> + Send;

	/// As Event Handlers have some inner state, from time to time we need to call this tick method
	/// so the event handler can update its state.
	fn tick(&mut self) -> impl Future<Output = ()> + Send;
}

#[derive(Debug)]
pub(super) struct LocationWatcher {
	location_id: location::id::Type,
	location_path: PathBuf,
	watcher: RecommendedWatcher,
	ignore_path_tx: chan::Sender<IgnorePath>,
	handle: Option<JoinHandle<()>>,
	stop_tx: chan::Sender<()>,
}

impl LocationWatcher {
	#[instrument(
		name = "location_watcher",
		skip(pub_id, maybe_location_path, library, node),
		fields(
			library_id = %library.id,
			location_path = ?maybe_location_path,
		),
	)]
	pub(super) fn new(
		location_ids_and_path::Data {
			id: location_id,
			pub_id,
			path: maybe_location_path,
			..
		}: location_ids_and_path::Data,
		library: Arc<Library>,
		node: Arc<Node>,
	) -> Result<Self, LocationManagerError> {
		let location_pub_id = Uuid::from_slice(&pub_id)?;
		let location_path = maybe_missing(maybe_location_path, "location.path")?.into();

		let (events_tx, events_rx) = chan::unbounded();
		let (ignore_path_tx, ignore_path_rx) = chan::bounded(8);
		let (stop_tx, stop_rx) = chan::bounded(1);

		let watcher = RecommendedWatcher::new(
			move |result| {
				if !events_tx.is_closed() {
					// SAFETY: we are not blocking the thread as this is an unbounded channel
					if events_tx.send_blocking(result).is_err() {
						error!(%location_id, "Unable to send watcher event to location manager;");
					}
				} else {
					error!(%location_id, "Tried to send file system events to a closed channel;");
				}
			},
			Config::default(),
		)?;

		let handle = spawn({
			let events_rx = events_rx.clone();
			let ignore_path_rx = ignore_path_rx.clone();
			let stop_rx = stop_rx.clone();
			async move {
				while let Err(e) = spawn(
					Self::handle_watch_events(
						location_id,
						location_pub_id,
						Arc::clone(&node),
						Arc::clone(&library),
						events_rx.clone(),
						ignore_path_rx.clone(),
						stop_rx.clone(),
					)
					.in_current_span(),
				)
				.await
				{
					if e.is_panic() {
						error!(?e, "Location watcher panicked;");
					} else {
						trace!("Location watcher received shutdown signal and will exit...");
						break;
					}
					trace!("Restarting location watcher processing task...");
				}

				info!("Location watcher gracefully shutdown");
			}
			.in_current_span()
		});

		Ok(Self {
			location_id,
			location_path,
			watcher,
			ignore_path_tx,
			handle: Some(handle),
			stop_tx,
		})
	}

	async fn handle_watch_events(
		location_id: location::id::Type,
		location_pub_id: Uuid,
		node: Arc<Node>,
		library: Arc<Library>,
		events_rx: chan::Receiver<notify::Result<Event>>,
		ignore_path_rx: chan::Receiver<IgnorePath>,
		stop_rx: chan::Receiver<()>,
	) {
		enum StreamMessage {
			NewEvent(notify::Result<Event>),
			NewIgnorePath(IgnorePath),
			Tick,
			Stop,
		}

		let mut event_handler = Handler::new(location_id, Arc::clone(&library), Arc::clone(&node));

		let mut last_event_at = Instant::now();

		let mut cached_indexer_ruler = None;
		let mut cached_location_path = None;

		let mut paths_to_ignore = HashSet::new();

		let mut handler_tick_interval =
			interval_at(Instant::now() + HUNDRED_MILLIS, HUNDRED_MILLIS);
		// In case of doubt check: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html
		handler_tick_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

		let mut msg_stream = pin!((
			events_rx.map(StreamMessage::NewEvent),
			ignore_path_rx.map(StreamMessage::NewIgnorePath),
			IntervalStream::new(handler_tick_interval).map(|_| StreamMessage::Tick),
			stop_rx.map(|()| StreamMessage::Stop),
		)
			.merge());

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::NewEvent(Ok(event)) => {
					if let Err(e) = get_cached_indexer_ruler_and_location_path(
						location_id,
						&mut cached_indexer_ruler,
						&mut cached_location_path,
						&last_event_at,
						&library.db,
					)
					.await
					{
						error!(?e, "Failed to get indexer ruler;");
					}

					last_event_at = Instant::now();

					if let Err(e) = Self::handle_single_event(
						location_pub_id,
						cached_location_path.as_deref(),
						event,
						&mut event_handler,
						&node,
						&paths_to_ignore,
						cached_indexer_ruler.as_ref(),
					)
					.await
					{
						error!(?e, "Failed to handle location file system event;");
					}
				}

				StreamMessage::NewEvent(Err(e)) => error!(?e, "Watcher error;"),

				StreamMessage::NewIgnorePath((path, should_ignore)) => {
					if should_ignore {
						paths_to_ignore.insert(path);
					} else {
						paths_to_ignore.remove(&path);
					}
				}

				StreamMessage::Tick => event_handler.tick().await,

				StreamMessage::Stop => {
					debug!("Stopping Location Manager event handler for location");
					break;
				}
			}
		}
	}

	#[instrument(skip_all, fields(?event, ?ignore_paths, ?location_path))]
	async fn handle_single_event(
		location_pub_id: Uuid,
		location_path: Option<&Path>,
		event: Event,
		event_handler: &mut impl EventHandler,
		node: &Node,
		ignore_paths: &HashSet<PathBuf>,
		indexer_ruler: Option<&IndexerRuler>,
	) -> Result<(), LocationManagerError> {
		if reject_event(&event, ignore_paths, location_path, indexer_ruler).await {
			return Ok(());
		}

		if !node.locations.is_online(&location_pub_id).await {
			warn!("Tried to handle event for offline location");
			return Ok(());
		}

		event_handler.handle_event(event).await
	}

	#[instrument(
		skip(self, path),
		fields(
			location_id = %self.location_id,
			location_path = %self.location_path.display(),
			path = %path.display(),
		),
	)]
	pub(super) async fn ignore_path(&self, path: PathBuf, ignore: bool) {
		self.ignore_path_tx
			.send((path, ignore))
			.await
			.expect("Location watcher ignore path channel closed");
	}

	pub(super) fn check_path(&self, path: impl AsRef<Path>) -> bool {
		self.location_path == path.as_ref()
	}

	#[instrument(
		skip(self),
		fields(
			location_id = %self.location_id,
			location_path = %self.location_path.display(),
		),
	)]
	pub(super) fn watch(&mut self) {
		trace!("Start watching location");

		if let Err(e) = self
			.watcher
			.watch(self.location_path.as_path(), RecursiveMode::Recursive)
		{
			error!(?e, "Unable to watch location;");
		} else {
			trace!("Now watching location");
		}
	}

	#[instrument(
		skip(self),
		fields(
			location_id = %self.location_id,
			location_path = %self.location_path.display(),
		),
	)]
	pub(super) fn unwatch(&mut self) {
		if let Err(e) = self.watcher.unwatch(self.location_path.as_path()) {
			/**************************************** TODO: ****************************************
			 * According to an unit test, this error may occur when a subdirectory is removed	   *
			 * and we try to unwatch the parent directory then we have to check the implications   *
			 * of unwatch error for this case.   												   *
			 **************************************************************************************/
			error!(?e, "Unable to unwatch location;");
		} else {
			trace!("Stop watching location");
		}
	}
}

impl Drop for LocationWatcher {
	fn drop(&mut self) {
		// FIXME: change this Drop to async drop in the future
		if let Some(handle) = self.handle.take() {
			let stop_tx = self.stop_tx.clone();
			spawn(async move {
				stop_tx
					.send(())
					.await
					.expect("Location watcher stop channel closed");

				if let Err(e) = handle.await {
					error!(?e, "Failed to join watcher task;");
				}
			});
		}
	}
}

async fn get_cached_indexer_ruler_and_location_path(
	location_id: location::id::Type,
	cached_indexer_ruler: &mut Option<IndexerRuler>,
	location_path: &mut Option<PathBuf>,
	last_event_at: &Instant,
	db: &PrismaClient,
) -> Result<(), LocationManagerError> {
	if cached_indexer_ruler.is_none() || last_event_at.elapsed() > THIRTY_SECONDS {
		if let Some(location_with_indexer_rules::Data {
			path,
			indexer_rules,
			..
		}) = db
			.location()
			.find_unique(location::id::equals(location_id))
			.include(location_with_indexer_rules::include())
			.exec()
			.await?
		{
			*cached_indexer_ruler = Some(
				indexer_rules
					.iter()
					.map(|rule| IndexerRule::try_from(&rule.indexer_rule))
					.collect::<Result<Vec<_>, _>>()
					.map(IndexerRuler::new)?,
			);

			*location_path = path.map(Into::into);
		}
	}

	Ok(())
}

/***************************************************************************************************
* Some tests to validate our assumptions of events through different file systems				   *
****************************************************************************************************
*	Events dispatched on Linux:																	   *
*		Create File:																			   *
*			1) EventKind::Create(CreateKind::File)												   *
*			2) EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any))						   *
*				or EventKind::Modify(ModifyKind::Data(DataChange::Any))							   *
*			3) EventKind::Access(AccessKind::Close(AccessMode::Write)))							   *
*		Create Directory:																		   *
*			1) EventKind::Create(CreateKind::Folder)											   *
*		Update File:																			   *
*			1) EventKind::Modify(ModifyKind::Data(DataChange::Any))								   *
*			2) EventKind::Access(AccessKind::Close(AccessMode::Write)))							   *
*		Update File (rename):																	   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*			3) EventKind::Modify(ModifyKind::Name(RenameMode::Both))							   *
*		Update Directory (rename):																   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*			3) EventKind::Modify(ModifyKind::Name(RenameMode::Both))							   *
*		Delete File:																			   *
*			1) EventKind::Remove(RemoveKind::File)												   *
*		Delete Directory:																		   *
*			1) EventKind::Remove(RemoveKind::Folder)											   *
*																								   *
*	Events dispatched on MacOS:																	   *
*		Create File:																			   *
*			1) EventKind::Create(CreateKind::File)												   *
*			2) EventKind::Modify(ModifyKind::Data(DataChange::Content))							   *
*		Create Directory:																		   *
*			1) EventKind::Create(CreateKind::Folder)											   *
*		Update File:																			   *
*			1) EventKind::Modify(ModifyKind::Data(DataChange::Content))							   *
*		Update File (rename):																	   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::Any)) -- From						   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::Any))	-- To						   *
*		Update Directory (rename):																   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::Any)) -- From						   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::Any))	-- To						   *
*		Delete File:																			   *
*			1) EventKind::Remove(RemoveKind::File)												   *
*		Delete Directory:																		   *
*			1) EventKind::Remove(RemoveKind::Folder)											   *
*																								   *
*	Events dispatched on Windows:																   *
*		Create File:																			   *
*			1) EventKind::Create(CreateKind::Any)												   *
*			2) EventKind::Modify(ModifyKind::Any)												   *
*		Create Directory:																		   *
*			1) EventKind::Create(CreateKind::Any)												   *
*		Update File:																			   *
*			1) EventKind::Modify(ModifyKind::Any)												   *
*		Update File (rename):																	   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*		Update Directory (rename):																   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*		Delete File:																			   *
*			1) EventKind::Remove(RemoveKind::Any)												   *
*		Delete Directory:																		   *
*			1) EventKind::Remove(RemoveKind::Any)												   *
*																								   *
*	Events dispatched on Android:																   *
*	TODO																						   *
*																								   *
*	Events dispatched on iOS:																	   *
*	TODO																						   *
*																								   *
***************************************************************************************************/
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
	use std::{
		io::ErrorKind,
		path::{Path, PathBuf},
		time::Duration,
	};

	use notify::{
		event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
		Config, Event, EventKind, RecommendedWatcher, Watcher,
	};
	use tempfile::{tempdir, TempDir};
	use tokio::{fs, io::AsyncWriteExt, sync::mpsc, time::sleep};
	use tracing::{debug, error};
	// use tracing_test::traced_test;

	#[cfg(any(target_os = "macos", target_os = "ios"))]
	use notify::event::DataChange;

	#[cfg(target_os = "linux")]
	use notify::event::{AccessKind, AccessMode};

	async fn setup_watcher() -> (
		TempDir,
		RecommendedWatcher,
		mpsc::UnboundedReceiver<notify::Result<Event>>,
	) {
		let (events_tx, events_rx) = mpsc::unbounded_channel();

		let watcher = RecommendedWatcher::new(
			move |result| {
				events_tx
					.send(result)
					.expect("Unable to send watcher event");
			},
			Config::default(),
		)
		.expect("Failed to create watcher");

		(tempdir().unwrap(), watcher, events_rx)
	}

	async fn expect_event(
		mut events_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
		path: impl AsRef<Path>,
		expected_event: EventKind,
	) {
		let path = path.as_ref();
		debug!(?expected_event, path = %path.display());
		let mut tries = 0;
		loop {
			match events_rx.try_recv() {
				Ok(maybe_event) => {
					let event = maybe_event.expect("Failed to receive event");
					debug!(?event, "Received event;");
					// Using `ends_with` and removing root path here due to a weird edge case on CI tests at MacOS
					if event.paths[0].ends_with(path.iter().skip(1).collect::<PathBuf>())
						&& event.kind == expected_event
					{
						debug!("Received expected event");
						break;
					}
				}
				Err(e) => {
					debug!(?e, "No event yet;");
					tries += 1;
					sleep(Duration::from_millis(100)).await;
				}
			}

			if tries == 10 {
				panic!("No expected event received after 10 tries");
			}
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn create_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &file_path, EventKind::Modify(ModifyKind::Any)).await;

		#[cfg(any(target_os = "macos", target_os = "ios"))]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Data(DataChange::Content)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Access(AccessKind::Close(AccessMode::Write)),
		)
		.await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn create_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Any)).await;

		#[cfg(any(target_os = "macos", target_os = "ios"))]
		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Folder)).await;

		#[cfg(target_os = "linux")]
		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Folder)).await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn update_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		let mut file = fs::OpenOptions::new()
			.append(true)
			.open(&file_path)
			.await
			.expect("Failed to open file");

		// Writing then sync data before closing the file
		file.write_all(b"\nanother test")
			.await
			.expect("Failed to write to file");
		file.sync_all().await.expect("Failed to flush file");
		drop(file);

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &file_path, EventKind::Modify(ModifyKind::Any)).await;

		#[cfg(any(target_os = "macos", target_os = "ios"))]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Data(DataChange::Content)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Access(AccessKind::Close(AccessMode::Write)),
		)
		.await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn update_file_rename_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		let new_file_name = root_dir.path().join("test2.txt");

		fs::rename(&file_path, &new_file_name)
			.await
			.expect("Failed to rename file");

		#[cfg(target_os = "windows")]
		expect_event(
			events_rx,
			&new_file_name,
			EventKind::Modify(ModifyKind::Name(RenameMode::To)),
		)
		.await;

		#[cfg(any(target_os = "macos", target_os = "ios"))]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
		)
		.await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn update_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		let new_dir_name = root_dir.path().join("inner2");

		fs::rename(&dir_path, &new_dir_name)
			.await
			.expect("Failed to rename directory");

		#[cfg(target_os = "windows")]
		expect_event(
			events_rx,
			&new_dir_name,
			EventKind::Modify(ModifyKind::Name(RenameMode::To)),
		)
		.await;

		#[cfg(any(target_os = "macos", target_os = "ios"))]
		expect_event(
			events_rx,
			&dir_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&dir_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
		)
		.await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn delete_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		fs::remove_file(&file_path)
			.await
			.expect("Failed to remove file");

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::File)).await;

		#[cfg(target_os = "linux")]
		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::File)).await;

		#[cfg(target_os = "ios")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
		)
		.await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn delete_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		if let Err(e) = fs::metadata(&dir_path).await {
			if e.kind() == ErrorKind::NotFound {
				panic!("Directory not found");
			} else {
				panic!("{e}");
			}
		}

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!(root = %root_dir.path().display(), "Now watching;");

		debug!("First unwatching the inner directory before removing it");
		if let Err(e) = watcher.unwatch(&dir_path) {
			error!(?e, "Failed to unwatch inner directory;");
		}

		fs::remove_dir(&dir_path)
			.await
			.expect("Failed to remove directory");

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Folder)).await;

		#[cfg(target_os = "linux")]
		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Folder)).await;

		#[cfg(target_os = "ios")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
		)
		.await;

		debug!(root = %root_dir.path().display(), "Unwatching root directory;");
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!(?e, "Failed to unwatch root directory;");
		}
	}
}
