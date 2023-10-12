use crate::{
	library::{Libraries, LibraryManagerEvent},
	object::media::thumbnail::ThumbnailerError,
	prisma::{file_path, PrismaClient},
	util::error::{FileIOError, NonUtf8PathError},
};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	ffi::OsStr,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::{Duration, SystemTime},
};

use async_channel as chan;
use futures::{future::try_join_all, stream::FuturesUnordered, FutureExt};
use futures_concurrency::{
	future::{Join, Race},
	stream::Merge,
};
use thiserror::Error;
use tokio::{
	fs, io, spawn,
	sync::oneshot,
	time::{interval, interval_at, timeout, Instant, MissedTickBehavior},
};
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{debug, error, trace};
use uuid::Uuid;

use super::{generate_thumbnail, GenerateThumbnailArgs, THUMBNAIL_CACHE_DIR_NAME};

const ONE_SEC: Duration = Duration::from_secs(1);
const THIRTY_SECS: Duration = Duration::from_secs(30);
const HALF_HOUR: Duration = Duration::from_secs(30 * 60);
const ONE_WEEK: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Error, Debug)]
enum Error {
	#[error("database error")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("missing file name: {}", .0.display())]
	MissingFileName(Box<Path>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
}

#[derive(Debug)]
enum DatabaseMessage {
	Add(Uuid, Arc<PrismaClient>),
	Remove(Uuid),
}

// Thumbnails directory have the following structure:
// thumbnails/
// ├── version.txt
//└── <cas_id>[0..2]/ # sharding
//    └── <cas_id>.webp
pub struct Thumbnailer {
	cas_ids_to_delete_tx: chan::Sender<Vec<String>>,
	ephemeral_thumbnails_to_generate_tx: chan::Sender<Vec<GenerateThumbnailArgs>>,
	_cancel_loop: DropGuard,
}

impl Thumbnailer {
	pub fn new(data_dir: PathBuf, lm: Arc<Libraries>) -> Self {
		let mut thumbnails_directory = data_dir;
		thumbnails_directory.push(THUMBNAIL_CACHE_DIR_NAME);

		let (databases_tx, databases_rx) = chan::bounded(4);
		let (ephemeral_thumbnails_to_generate_tx, ephemeral_thumbnails_to_generate_rx) =
			chan::unbounded();
		let (cas_ids_to_delete_tx, cas_ids_to_delete_rx) = chan::bounded(16);
		let cancel_token = CancellationToken::new();

		let inner_cancel_token = cancel_token.child_token();
		tokio::spawn(async move {
			loop {
				if let Err(e) = tokio::spawn(Self::worker(
					thumbnails_directory.clone(),
					databases_rx.clone(),
					cas_ids_to_delete_rx.clone(),
					ephemeral_thumbnails_to_generate_rx.clone(),
					inner_cancel_token.child_token(),
				))
				.await
				{
					error!(
						"Error on Thumbnail Remover Actor; \
						Error: {e}; \
						Restarting the worker loop...",
					);
				}
				if inner_cancel_token.is_cancelled() {
					break;
				}
			}
		});

		tokio::spawn({
			let rx = lm.rx.clone();
			async move {
				if let Err(err) = rx
					.subscribe(move |event| {
						let databases_tx = databases_tx.clone();

						async move {
							match event {
								LibraryManagerEvent::Load(library) => {
									if databases_tx
										.send(DatabaseMessage::Add(library.id, library.db.clone()))
										.await
										.is_err()
									{
										error!("Thumbnail remover actor is dead")
									}
								}
								LibraryManagerEvent::Edit(_) => {}
								LibraryManagerEvent::InstancesModified(_) => {}
								LibraryManagerEvent::Delete(library) => {
									if databases_tx
										.send(DatabaseMessage::Remove(library.id))
										.await
										.is_err()
									{
										error!("Thumbnail remover actor is dead")
									}
								}
							}
						}
					})
					.await
				{
					error!("Thumbnail remover actor has crashed with error: {err:?}")
				}
			}
		});

		Self {
			cas_ids_to_delete_tx,
			ephemeral_thumbnails_to_generate_tx,
			_cancel_loop: cancel_token.drop_guard(),
		}
	}

	async fn worker(
		thumbnails_directory: PathBuf,
		databases_rx: chan::Receiver<DatabaseMessage>,
		cas_ids_to_delete_rx: chan::Receiver<Vec<String>>,
		ephemeral_thumbnails_to_generate_rx: chan::Receiver<Vec<GenerateThumbnailArgs>>,
		cancel_token: CancellationToken,
	) {
		let mut to_remove_interval = interval_at(Instant::now() + THIRTY_SECS, HALF_HOUR);
		to_remove_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut idle_interval = interval(ONE_SEC);
		idle_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut databases = HashMap::new();
		let mut ephemeral_thumbnails_cas_ids = HashSet::new();

		#[derive(Debug)]
		enum StreamMessage {
			RemovalTick,
			ToDelete(Vec<String>),
			Database(DatabaseMessage),
			EphemeralThumbnailNewBatch(Vec<GenerateThumbnailArgs>),
			Leftovers(Vec<GenerateThumbnailArgs>),
			NewEphemeralThumbnailCasIds(Vec<String>),
			Stop,
			IdleTick,
		}

		let cancel = pin!(cancel_token.cancelled());

		// This is a LIFO queue, so we can process the most recent thumbnails first
		let mut ephemeral_thumbnails_queue = Vec::with_capacity(8);

		// This one is a FIFO queue, so we can process leftovers from the previous batch first
		let mut ephemeral_thumbnails_leftovers_queue = VecDeque::with_capacity(8);

		let (ephemeral_thumbnails_cas_ids_tx, ephemeral_thumbnails_cas_ids_rx) = chan::bounded(32);
		let (leftovers_tx, leftovers_rx) = chan::bounded(8);

		let (stop_older_processing_tx, stop_older_processing_rx) = chan::bounded(1);

		let mut current_batch_processing_rx: Option<oneshot::Receiver<()>> = None;

		let mut msg_stream = (
			databases_rx.map(StreamMessage::Database),
			cas_ids_to_delete_rx.map(StreamMessage::ToDelete),
			ephemeral_thumbnails_to_generate_rx.map(StreamMessage::EphemeralThumbnailNewBatch),
			leftovers_rx.map(StreamMessage::Leftovers),
			ephemeral_thumbnails_cas_ids_rx.map(StreamMessage::NewEphemeralThumbnailCasIds),
			IntervalStream::new(to_remove_interval).map(|_| StreamMessage::RemovalTick),
			IntervalStream::new(idle_interval).map(|_| StreamMessage::IdleTick),
			cancel.into_stream().map(|()| StreamMessage::Stop),
		)
			.merge();

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::IdleTick => {
					if let Some(done_rx) = current_batch_processing_rx.as_mut() {
						// Checking if the previous run finished or was aborted to clean state
						if matches!(
							done_rx.try_recv(),
							Ok(()) | Err(oneshot::error::TryRecvError::Closed)
						) {
							current_batch_processing_rx = None;
						}
					}

					if current_batch_processing_rx.is_none()
						&& (!ephemeral_thumbnails_queue.is_empty()
							|| !ephemeral_thumbnails_leftovers_queue.is_empty())
					{
						let (done_tx, done_rx) = oneshot::channel();
						current_batch_processing_rx = Some(done_rx);

						if let Some(batch) = ephemeral_thumbnails_queue.pop() {
							spawn(batch_processor(
								batch,
								ephemeral_thumbnails_cas_ids_tx.clone(),
								stop_older_processing_rx.clone(),
								done_tx,
								leftovers_tx.clone(),
								false,
							));
						} else if let Some(batch) = ephemeral_thumbnails_leftovers_queue.pop_front()
						{
							spawn(batch_processor(
								batch,
								ephemeral_thumbnails_cas_ids_tx.clone(),
								stop_older_processing_rx.clone(),
								done_tx,
								leftovers_tx.clone(),
								true,
							));
						}
					}
				}

				StreamMessage::RemovalTick => {
					// For any of them we process a clean up if a time since the last one already passed
					if !databases.is_empty() {
						if let Err(e) = Self::process_clean_up(
							&thumbnails_directory,
							databases.values(),
							&ephemeral_thumbnails_cas_ids,
						)
						.await
						{
							error!("Got an error when trying to clean stale thumbnails: {e:#?}");
						}
					}
				}
				StreamMessage::ToDelete(cas_ids) => {
					if !cas_ids.is_empty() {
						if let Err(e) =
							Self::remove_by_cas_ids(&thumbnails_directory, cas_ids).await
						{
							error!("Got an error when trying to remove thumbnails: {e:#?}");
						}
					}
				}

				StreamMessage::EphemeralThumbnailNewBatch(batch) => {
					ephemeral_thumbnails_queue.push(batch);
					if current_batch_processing_rx.is_some() // Only sends stop signal if there is a batch being processed
						&& stop_older_processing_tx.send(()).await.is_err()
					{
						error!("Thumbnail remover actor died when trying to stop older processing");
					}
				}

				StreamMessage::Leftovers(batch) => {
					ephemeral_thumbnails_leftovers_queue.push_back(batch);
				}

				StreamMessage::Database(DatabaseMessage::Add(id, db)) => {
					databases.insert(id, db);
				}
				StreamMessage::Database(DatabaseMessage::Remove(id)) => {
					databases.remove(&id);
				}
				StreamMessage::NewEphemeralThumbnailCasIds(cas_ids) => {
					ephemeral_thumbnails_cas_ids.extend(cas_ids);
				}
				StreamMessage::Stop => {
					debug!("Thumbnail remover actor is stopping");
					break;
				}
			}
		}
	}

	async fn remove_by_cas_ids(
		thumbnails_directory: &Path,
		cas_ids: Vec<String>,
	) -> Result<(), Error> {
		try_join_all(cas_ids.into_iter().map(|cas_id| async move {
			let thumbnail_path =
				thumbnails_directory.join(format!("{}/{cas_id}.webp", &cas_id[0..2]));

			trace!("Removing thumbnail: {}", thumbnail_path.display());

			match fs::remove_file(&thumbnail_path).await {
				Ok(()) => Ok(()),
				Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
				Err(e) => Err(FileIOError::from((thumbnail_path, e))),
			}
		}))
		.await?;

		Ok(())
	}

	async fn process_clean_up(
		thumbnails_directory: &Path,
		databases: impl Iterator<Item = &Arc<PrismaClient>>,
		non_indexed_thumbnails_cas_ids: &HashSet<String>,
	) -> Result<(), Error> {
		let databases = databases.collect::<Vec<_>>();

		fs::create_dir_all(&thumbnails_directory)
			.await
			.map_err(|e| FileIOError::from((thumbnails_directory, e)))?;
		let mut read_dir = fs::read_dir(thumbnails_directory)
			.await
			.map_err(|e| FileIOError::from((thumbnails_directory, e)))?;

		while let Some(entry) = read_dir
			.next_entry()
			.await
			.map_err(|e| FileIOError::from((thumbnails_directory, e)))?
		{
			let entry_path = entry.path();
			if !entry
				.metadata()
				.await
				.map_err(|e| FileIOError::from((thumbnails_directory, e)))?
				.is_dir()
			{
				continue;
			}

			let mut thumbnails_paths_by_cas_id = HashMap::new();

			let mut entry_read_dir = fs::read_dir(&entry_path)
				.await
				.map_err(|e| FileIOError::from((&entry_path, e)))?;

			while let Some(thumb_entry) = entry_read_dir
				.next_entry()
				.await
				.map_err(|e| FileIOError::from((&entry_path, e)))?
			{
				let thumb_path = thumb_entry.path();

				if thumb_path
					.extension()
					.and_then(OsStr::to_str)
					.map_or(true, |ext| ext != "webp")
				{
					continue;
				}

				let thumbnail_name = thumb_path
					.file_stem()
					.ok_or_else(|| Error::MissingFileName(entry.path().into_boxed_path()))?
					.to_str()
					.ok_or_else(|| NonUtf8PathError(entry.path().into_boxed_path()))?;

				thumbnails_paths_by_cas_id.insert(thumbnail_name.to_string(), thumb_path);
			}

			if thumbnails_paths_by_cas_id.is_empty() {
				trace!(
					"Removing empty thumbnails sharding directory: {}",
					entry_path.display()
				);
				fs::remove_dir(&entry_path)
					.await
					.map_err(|e| FileIOError::from((entry_path, e)))?;

				continue;
			}

			let thumbs_found = thumbnails_paths_by_cas_id.len();

			let mut thumbs_in_db_futs = databases
				.iter()
				.map(|db| {
					db.file_path()
						.find_many(vec![file_path::cas_id::in_vec(
							thumbnails_paths_by_cas_id.keys().cloned().collect(),
						)])
						.select(file_path::select!({ cas_id }))
						.exec()
				})
				.collect::<FuturesUnordered<_>>();

			while let Some(maybe_thumbs_in_db) = thumbs_in_db_futs.next().await {
				maybe_thumbs_in_db?
					.into_iter()
					.filter_map(|file_path| file_path.cas_id)
					.for_each(|cas_id| {
						thumbnails_paths_by_cas_id.remove(&cas_id);
					});
			}

			thumbnails_paths_by_cas_id
				.retain(|cas_id, _| !non_indexed_thumbnails_cas_ids.contains(cas_id));

			let now = SystemTime::now();

			let removed_count = try_join_all(thumbnails_paths_by_cas_id.into_values().map(
				|path| async move {
					if let Ok(metadata) = fs::metadata(&path).await {
						if metadata
							.accessed()
							.map(|when| {
								now.duration_since(when)
									.map(|duration| duration < ONE_WEEK)
									.unwrap_or(false)
							})
							.unwrap_or(false)
						{
							// If the thumbnail was accessed in the last week, we don't remove it yet
							// as the file is probably still in use
							return Ok(false);
						}
					}

					tracing::warn!("Removing stale thumbnail: {}", path.display());
					fs::remove_file(&path)
						.await
						.map(|()| true)
						.map_err(|e| FileIOError::from((path, e)))
				},
			))
			.await?
			.into_iter()
			.filter(|r| *r)
			.count();

			if thumbs_found == removed_count {
				// if we removed all the thumnails we found, it means that the directory is empty
				// and can be removed...
				trace!(
					"Removing empty thumbnails sharding directory: {}",
					entry_path.display()
				);
				fs::remove_dir(&entry_path)
					.await
					.map_err(|e| FileIOError::from((entry_path, e)))?;
			}
		}

		Ok(())
	}

	pub async fn new_non_indexed_thumbnails_batch(&self, batch: Vec<GenerateThumbnailArgs>) {
		if self
			.ephemeral_thumbnails_to_generate_tx
			.send(batch)
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead: Failed to send new batch");
		}
	}

	pub async fn remove_cas_ids(&self, cas_ids: Vec<String>) {
		if self.cas_ids_to_delete_tx.send(cas_ids).await.is_err() {
			error!("Thumbnail remover actor is dead: Failed to send cas ids to delete");
		}
	}
}

async fn batch_processor(
	batch: Vec<GenerateThumbnailArgs>,
	generated_cas_ids_tx: chan::Sender<Vec<String>>,
	stop_rx: chan::Receiver<()>,
	done_tx: oneshot::Sender<()>,
	leftovers_tx: chan::Sender<Vec<GenerateThumbnailArgs>>,
	in_background: bool,
) {
	let mut queue = VecDeque::from(batch);

	enum RaceOutputs {
		Processed,
		Stop,
	}

	// Need this borrow here to satisfy the async move below
	let generated_cas_ids_tx = &generated_cas_ids_tx;

	while !queue.is_empty() {
		let chunk = (0..4)
			.filter_map(|_| queue.pop_front())
			.map(
				|GenerateThumbnailArgs {
				     extension,
				     cas_id,
				     path,
				     node,
				 }| {
					spawn(async move {
						timeout(
							THIRTY_SECS,
							generate_thumbnail(&extension, cas_id, &path, node, in_background),
						)
						.await
						.unwrap_or_else(|_| Err(ThumbnailerError::TimedOut(path.into_boxed_path())))
					})
				},
			)
			.collect::<Vec<_>>();

		if let RaceOutputs::Stop = (
			async move {
				let cas_ids = chunk
					.join()
					.await
					.into_iter()
					.filter_map(|join_result| {
						join_result
							.map_err(|e| error!("Failed to join thumbnail generation task: {e:#?}"))
							.ok()
					})
					.filter_map(|result| {
						result
							.map_err(|e| {
								error!(
									"Failed to generate thumbnail for ephemeral location: {e:#?}"
								)
							})
							.ok()
					})
					.collect();

				if generated_cas_ids_tx.send(cas_ids).await.is_err() {
					error!("Thumbnail remover actor is dead: Failed to send generated cas ids")
				}

				trace!("Processed chunk of thumbnails");
				RaceOutputs::Processed
			},
			async {
				stop_rx
					.recv()
					.await
					.expect("Critical error on thumbnails actor");
				trace!("Received a stop signal");
				RaceOutputs::Stop
			},
		)
			.race()
			.await
		{
			// Our queue is always contiguous, so this `from`` is free
			let leftovers = Vec::from(queue);

			trace!(
				"Stopped with {} thumbnails left to process",
				leftovers.len()
			);
			if !leftovers.is_empty() && leftovers_tx.send(leftovers).await.is_err() {
				error!("Thumbnail remover actor is dead: Failed to send leftovers")
			}

			done_tx.send(()).ok();

			return;
		}
	}

	done_tx.send(()).ok();
}
