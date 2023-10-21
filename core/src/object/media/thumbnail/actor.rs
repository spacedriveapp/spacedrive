use crate::{
	api::CoreEvent,
	library::{Libraries, LibraryId, LibraryManagerEvent},
	object::media::thumbnail::get_shard_hex,
	util::error::{FileIOError, NonUtf8PathError},
};

use sd_prisma::prisma::PrismaClient;

use std::{
	collections::{HashMap, HashSet, VecDeque},
	ffi::OsString,
	path::{Path, PathBuf},
	sync::Arc,
};

use async_channel as chan;
use futures_concurrency::{future::TryJoin, stream::Merge};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
	fs, io, spawn,
	sync::{broadcast, oneshot, Mutex},
	time::{interval, interval_at, sleep, timeout, Instant, MissedTickBehavior},
};
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use super::{
	clean_up::{process_ephemeral_clean_up, process_indexed_clean_up},
	directory::init_thumbnail_dir,
	process::{batch_processor, generate_thumbnail, ProcessorControlChannels, ThumbData},
	BatchToProcess, ThumbnailKind, ThumbnailerError, EPHEMERAL_DIR, HALF_HOUR, ONE_SEC,
	SAVE_STATE_FILE, THIRTY_SECS, THUMBNAIL_CACHE_DIR_NAME,
};

static BATCH_SIZE: OnceCell<usize> = OnceCell::new();

#[derive(Error, Debug)]
enum Error {
	#[error("database error")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
}

#[derive(Debug)]
enum DatabaseMessage {
	Add(Uuid, Arc<PrismaClient>),
	Update(Uuid, Arc<PrismaClient>),
	Remove(Uuid),
}

// Thumbnails directory have the following structure:
// thumbnails/
// ├── version.txt
// ├── thumbs_to_process.bin # processing save state
// ├── ephemeral/ # ephemeral ones have it's own directory
// │  └── <cas_id>[0..3]/ # sharding
// │     └── <cas_id>.webp
// └── <library_id>/ # we segregate thumbnails by library
//    └── <cas_id>[0..3]/ # sharding
//       └── <cas_id>.webp
pub struct Thumbnailer {
	thumbnails_directory: PathBuf,
	cas_ids_to_delete_tx: chan::Sender<(Vec<String>, ThumbnailKind)>,
	thumbnails_to_generate_tx: chan::Sender<(BatchToProcess, ThumbnailKind)>,
	last_single_thumb_generated: Mutex<Instant>,
	reporter: broadcast::Sender<CoreEvent>,
	cancel_tx: chan::Sender<oneshot::Sender<()>>,
}

impl Thumbnailer {
	pub async fn new(
		data_dir: PathBuf,
		libraries_manager: Arc<Libraries>,
		reporter: broadcast::Sender<CoreEvent>,
	) -> Self {
		let thumbnails_directory = init_thumbnail_dir(&data_dir, Arc::clone(&libraries_manager))
			.await
			.unwrap_or_else(|e| {
				error!("Failed to initialize thumbnail directory: {e:#?}");
				let mut thumbnails_directory = data_dir;
				thumbnails_directory.push(THUMBNAIL_CACHE_DIR_NAME);
				thumbnails_directory
			});

		let (databases_tx, databases_rx) = chan::bounded(4);
		let (thumbnails_to_generate_tx, ephemeral_thumbnails_to_generate_rx) = chan::unbounded();
		let (cas_ids_to_delete_tx, cas_ids_to_delete_rx) = chan::bounded(16);
		let (cancel_tx, cancel_rx) = chan::bounded(1);

		BATCH_SIZE
			.set(std::thread::available_parallelism().map_or_else(
				|e| {
					error!("Failed to get available parallelism: {e:#?}");
					4
				},
				|non_zero| {
					let count = non_zero.get();
					debug!("Thumbnailer will process batches of {count} thumbnails in parallel.");
					count
				},
			))
			.ok();

		spawn({
			let cancel_rx = cancel_rx.clone();
			let thumbnails_directory = thumbnails_directory.clone();
			let reporter = reporter.clone();
			async move {
				while let Err(e) = spawn(Self::worker(
					reporter.clone(),
					thumbnails_directory.clone(),
					databases_rx.clone(),
					cas_ids_to_delete_rx.clone(),
					ephemeral_thumbnails_to_generate_rx.clone(),
					cancel_rx.clone(),
				))
				.await
				{
					error!(
						"Error on Thumbnail Remover Actor; \
						Error: {e}; \
						Restarting the worker loop...",
					);
				}
			}
		});

		spawn({
			let rx = libraries_manager.rx.clone();
			let thumbnails_directory = thumbnails_directory.clone();

			async move {
				if let Err(err) = rx
					.subscribe(|event| {
						let databases_tx = databases_tx.clone();

						let thumbnails_directory = &thumbnails_directory;

						async move {
							match event {
								LibraryManagerEvent::Load(library) => {
									let library_dir =
										thumbnails_directory.join(library.id.to_string());
									fs::create_dir_all(&library_dir)
										.await
										.map_err(|e| {
											error!("Failed to create library dir for thumbnails: {:#?}", FileIOError::from((library_dir, e)))
										})
										.ok();
									if databases_tx
										.send(DatabaseMessage::Add(
											library.id,
											Arc::clone(&library.db),
										))
										.await
										.is_err()
									{
										error!("Thumbnail remover actor is dead")
									}
								}
								LibraryManagerEvent::Edit(library)
								| LibraryManagerEvent::InstancesModified(library) => {
									if databases_tx
										.send(DatabaseMessage::Update(
											library.id,
											Arc::clone(&library.db),
										))
										.await
										.is_err()
									{
										error!("Thumbnail remover actor is dead")
									}
								}
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
			thumbnails_directory,
			cas_ids_to_delete_tx,
			thumbnails_to_generate_tx,
			last_single_thumb_generated: Mutex::new(Instant::now()),
			reporter,
			cancel_tx,
		}
	}

	async fn worker(
		reporter: broadcast::Sender<CoreEvent>,
		thumbnails_directory: PathBuf,
		databases_rx: chan::Receiver<DatabaseMessage>,
		cas_ids_to_delete_rx: chan::Receiver<(Vec<String>, ThumbnailKind)>,
		thumbnails_to_generate_rx: chan::Receiver<(BatchToProcess, ThumbnailKind)>,
		cancel_rx: chan::Receiver<oneshot::Sender<()>>,
	) {
		let mut to_remove_interval = interval_at(Instant::now() + THIRTY_SECS, HALF_HOUR);
		to_remove_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut idle_interval = interval(ONE_SEC);
		idle_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut databases = HashMap::new();

		#[derive(Debug)]
		enum StreamMessage {
			RemovalTick,
			ToDelete((Vec<String>, ThumbnailKind)),
			Database(DatabaseMessage),
			NewBatch((BatchToProcess, ThumbnailKind)),
			Leftovers((BatchToProcess, ThumbnailKind)),
			NewEphemeralThumbnailsFilenames(Vec<OsString>),
			Shutdown(oneshot::Sender<()>),
			IdleTick,
		}

		let ThumbsProcessingSaveState {
			mut ephemeral_file_names,
			mut queue,
			mut indexed_leftovers_queue,
			mut ephemeral_leftovers_queue,
		} = ThumbsProcessingSaveState::load(&thumbnails_directory).await;

		let (generated_ephemeral_thumbnails_tx, ephemeral_thumbnails_cas_ids_rx) =
			chan::bounded(32);
		let (leftovers_tx, leftovers_rx) = chan::bounded(8);

		let mut shutdown_leftovers_rx = leftovers_rx.clone();

		let (stop_older_processing_tx, stop_older_processing_rx) = chan::bounded(1);

		let mut current_batch_processing_rx: Option<oneshot::Receiver<()>> = None;

		let mut msg_stream = (
			databases_rx.map(StreamMessage::Database),
			cas_ids_to_delete_rx.map(StreamMessage::ToDelete),
			thumbnails_to_generate_rx.map(StreamMessage::NewBatch),
			leftovers_rx.map(StreamMessage::Leftovers),
			ephemeral_thumbnails_cas_ids_rx.map(StreamMessage::NewEphemeralThumbnailsFilenames),
			IntervalStream::new(to_remove_interval).map(|_| StreamMessage::RemovalTick),
			IntervalStream::new(idle_interval).map(|_| StreamMessage::IdleTick),
			cancel_rx.map(StreamMessage::Shutdown),
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
						&& (!queue.is_empty()
							|| !indexed_leftovers_queue.is_empty()
							|| !ephemeral_leftovers_queue.is_empty())
					{
						let (done_tx, done_rx) = oneshot::channel();
						current_batch_processing_rx = Some(done_rx);

						let batch_and_kind = if let Some(batch_and_kind) = queue.pop_front() {
							batch_and_kind
						} else if let Some((batch, library_id)) =
							indexed_leftovers_queue.pop_front()
						{
							// indexed leftovers have bigger priority
							(batch, ThumbnailKind::Indexed(library_id))
						} else if let Some(batch) = ephemeral_leftovers_queue.pop_front() {
							(batch, ThumbnailKind::Ephemeral)
						} else {
							continue;
						};

						spawn(batch_processor(
							thumbnails_directory.clone(),
							batch_and_kind,
							generated_ephemeral_thumbnails_tx.clone(),
							ProcessorControlChannels {
								stop_rx: stop_older_processing_rx.clone(),
								done_tx,
							},
							leftovers_tx.clone(),
							reporter.clone(),
							*BATCH_SIZE
								.get()
								.expect("BATCH_SIZE is set at thumbnailer new method"),
						));
					}
				}

				StreamMessage::RemovalTick => {
					// For any of them we process a clean up if a time since the last one already passed
					if !databases.is_empty() {
						spawn(process_indexed_clean_up(
							thumbnails_directory.clone(),
							databases
								.iter()
								.map(|(id, db)| (*id, Arc::clone(db)))
								.collect::<Vec<_>>(),
						));
					}

					if !ephemeral_file_names.is_empty() {
						spawn(process_ephemeral_clean_up(
							thumbnails_directory.clone(),
							ephemeral_file_names.clone(),
						));
					}
				}
				StreamMessage::ToDelete((cas_ids, kind)) => {
					if !cas_ids.is_empty() {
						if let Err(e) =
							Self::remove_by_cas_ids(&thumbnails_directory, cas_ids, kind).await
						{
							error!("Got an error when trying to remove thumbnails: {e:#?}");
						}
					}
				}

				StreamMessage::NewBatch((batch, kind)) => {
					let in_background = batch.in_background;

					trace!(
						"New {kind:?} batch to process in {}, size: {}",
						if in_background {
							"background"
						} else {
							"foreground"
						},
						batch.batch.len()
					);

					if in_background {
						queue.push_back((batch, kind));
					} else {
						// If a processing must be in foreground, then it takes maximum priority
						queue.push_front((batch, kind));
					}

					// Only sends stop signal if there is a batch being processed
					if !in_background && current_batch_processing_rx.is_some() {
						trace!("Sending stop signal to older processing");
						let (tx, rx) = oneshot::channel();

						match stop_older_processing_tx.try_send(tx) {
							Ok(()) => {
								// We put a timeout here to avoid a deadlock in case the older processing already
								// finished its batch
								if timeout(ONE_SEC, rx).await.is_err() {
									stop_older_processing_rx.recv().await.ok();
								}
							}
							Err(e) if e.is_full() => {
								// The last signal we sent happened after a batch was already processed
								// So we clean the channel and we're good to go.
								stop_older_processing_rx.recv().await.ok();
							}
							Err(_) => {
								error!("Thumbnail remover actor died when trying to stop older processing");
							}
						}
					}
				}

				StreamMessage::Leftovers((batch, kind)) => match kind {
					ThumbnailKind::Indexed(library_id) => {
						indexed_leftovers_queue.push_back((batch, library_id))
					}
					ThumbnailKind::Ephemeral => ephemeral_leftovers_queue.push_back(batch),
				},

				StreamMessage::Database(DatabaseMessage::Add(id, db))
				| StreamMessage::Database(DatabaseMessage::Update(id, db)) => {
					databases.insert(id, db);
				}
				StreamMessage::Database(DatabaseMessage::Remove(id)) => {
					databases.remove(&id);
				}
				StreamMessage::NewEphemeralThumbnailsFilenames(new_ephemeral_thumbs) => {
					trace!("New ephemeral thumbnails: {}", new_ephemeral_thumbs.len());
					ephemeral_file_names.extend(new_ephemeral_thumbs);
				}
				StreamMessage::Shutdown(cancel_tx) => {
					debug!("Thumbnail actor is shutting down...");

					// First stopping the current batch processing
					let (tx, rx) = oneshot::channel();
					match stop_older_processing_tx.try_send(tx) {
						Ok(()) => {
							// We put a timeout here to avoid a deadlock in case the older processing already
							// finished its batch
							if timeout(ONE_SEC, rx).await.is_err() {
								stop_older_processing_rx.recv().await.ok();
							}
						}
						Err(e) if e.is_full() => {
							// The last signal we sent happened after a batch was already processed
							// So we clean the channel and we're good to go.
							stop_older_processing_rx.recv().await.ok();
						}
						Err(_) => {
							error!("Thumbnail actor died when trying to stop older processing");
						}
					}

					// Closing the leftovers channel to stop the batch processor as we already sent
					// an stop signal
					leftovers_tx.close();
					while let Some((batch, kind)) = shutdown_leftovers_rx.next().await {
						match kind {
							ThumbnailKind::Indexed(library_id) => {
								indexed_leftovers_queue.push_back((batch, library_id))
							}
							ThumbnailKind::Ephemeral => ephemeral_leftovers_queue.push_back(batch),
						}
					}

					// Saving state
					ThumbsProcessingSaveState {
						ephemeral_file_names,
						queue,
						indexed_leftovers_queue,
						ephemeral_leftovers_queue,
					}
					.store(thumbnails_directory)
					.await;

					// Signaling that we're done shutting down
					cancel_tx.send(()).ok();
					return;
				}
			}
		}
	}

	async fn remove_by_cas_ids(
		thumbnails_directory: &Path,
		cas_ids: Vec<String>,
		kind: ThumbnailKind,
	) -> Result<(), Error> {
		let base_dir = match kind {
			ThumbnailKind::Ephemeral => thumbnails_directory.join(EPHEMERAL_DIR),
			ThumbnailKind::Indexed(library_id) => thumbnails_directory.join(library_id.to_string()),
		};

		cas_ids
			.into_iter()
			.map(|cas_id| {
				let thumbnail_path =
					base_dir.join(format!("{}/{cas_id}.webp", get_shard_hex(&cas_id)));

				trace!("Removing thumbnail: {}", thumbnail_path.display());

				async move {
					match fs::remove_file(&thumbnail_path).await {
						Ok(()) => Ok(()),
						Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
						Err(e) => Err(FileIOError::from((thumbnail_path, e))),
					}
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		Ok(())
	}

	#[inline]
	async fn new_batch(&self, batch: BatchToProcess, kind: ThumbnailKind) {
		if self
			.thumbnails_to_generate_tx
			.send((batch, kind))
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead: Failed to send new batch");
		}
	}

	#[inline]
	pub async fn new_ephemeral_thumbnails_batch(&self, batch: BatchToProcess) {
		self.new_batch(batch, ThumbnailKind::Ephemeral).await
	}

	#[inline]
	pub async fn new_indexed_thumbnails_batch(&self, batch: BatchToProcess, library_id: LibraryId) {
		self.new_batch(batch, ThumbnailKind::Indexed(library_id))
			.await
	}

	#[inline]
	async fn remove_cas_ids(&self, cas_ids: Vec<String>, kind: ThumbnailKind) {
		if self
			.cas_ids_to_delete_tx
			.send((cas_ids, kind))
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead: Failed to send cas ids to delete");
		}
	}

	#[inline]
	pub async fn remove_ephemeral_cas_ids(&self, cas_ids: Vec<String>) {
		self.remove_cas_ids(cas_ids, ThumbnailKind::Ephemeral).await
	}

	#[inline]
	pub async fn remove_indexed_cas_ids(&self, cas_ids: Vec<String>, library_id: LibraryId) {
		self.remove_cas_ids(cas_ids, ThumbnailKind::Indexed(library_id))
			.await
	}

	pub async fn shutdown(&self) {
		let (tx, rx) = oneshot::channel();
		if self.cancel_tx.send(tx).await.is_err() {
			error!("Thumbnail remover actor is dead: Failed to send shutdown signal");
		} else {
			rx.await.ok();
		}
	}

	/// WARNING!!!! DON'T USE THIS METHOD IN A LOOP!!!!!!!!!!!!! It will be pretty slow on purpose!
	pub async fn generate_single_indexed_thumbnail(
		&self,
		extension: &str,
		cas_id: String,
		path: impl AsRef<Path>,
		library_id: LibraryId,
	) -> Result<(), ThumbnailerError> {
		self.generate_single_thumbnail(extension, cas_id, path, ThumbnailKind::Indexed(library_id))
			.await
	}

	async fn generate_single_thumbnail(
		&self,
		extension: &str,
		cas_id: String,
		path: impl AsRef<Path>,
		kind: ThumbnailKind,
	) -> Result<(), ThumbnailerError> {
		let mut last_single_thumb_generated_guard = self.last_single_thumb_generated.lock().await;

		let elapsed = Instant::now() - *last_single_thumb_generated_guard;
		if elapsed < ONE_SEC {
			// This will choke up in case someone try to use this method in a loop, otherwise
			// it will consume all the machine resources like a gluton monster from hell
			sleep(ONE_SEC - elapsed).await;
		}

		let res = generate_thumbnail(
			self.thumbnails_directory.clone(),
			ThumbData {
				extension,
				cas_id,
				path,
				in_background: false,
				should_regenerate: false,
				kind,
			},
			self.reporter.clone(),
		)
		.await
		.map(|_| ());

		*last_single_thumb_generated_guard = Instant::now();

		res
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct ThumbsProcessingSaveState {
	ephemeral_file_names: HashSet<OsString>,
	// This queues doubles as LIFO and FIFO, assuming LIFO in case of users asking for a new batch
	// by entering a new directory in the explorer, otherwise processing as FIFO
	queue: VecDeque<(BatchToProcess, ThumbnailKind)>,
	// These below are FIFO queues, so we can process leftovers from the previous batch first
	indexed_leftovers_queue: VecDeque<(BatchToProcess, LibraryId)>,
	ephemeral_leftovers_queue: VecDeque<BatchToProcess>,
}

impl Default for ThumbsProcessingSaveState {
	fn default() -> Self {
		Self {
			ephemeral_file_names: HashSet::with_capacity(128),
			queue: VecDeque::with_capacity(32),
			indexed_leftovers_queue: VecDeque::with_capacity(8),
			ephemeral_leftovers_queue: VecDeque::with_capacity(8),
		}
	}
}

impl ThumbsProcessingSaveState {
	async fn load(thumbnails_directory: impl AsRef<Path>) -> Self {
		let resume_file = thumbnails_directory.as_ref().join(SAVE_STATE_FILE);

		match fs::read(&resume_file).await {
			Ok(bytes) => {
				let this = rmp_serde::from_slice::<Self>(&bytes).unwrap_or_else(|e| {
					error!("Failed to deserialize save state at thumbnailer actor: {e:#?}");
					Self::default()
				});

				if let Err(e) = fs::remove_file(&resume_file).await {
					error!(
						"Failed to remove save state file at thumbnailer actor: {:#?}",
						FileIOError::from((resume_file, e))
					);
				}

				info!(
					"Resuming thumbnailer actor state: Existing ephemeral thumbs: {}; \
					Queued batches waiting processing: {}",
					this.ephemeral_file_names.len(),
					this.queue.len()
						+ this.indexed_leftovers_queue.len()
						+ this.ephemeral_leftovers_queue.len()
				);

				this
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				trace!("No save state found at thumbnailer actor");
				Self::default()
			}
			Err(e) => {
				error!(
					"Failed to read save state at thumbnailer actor: {:#?}",
					FileIOError::from((resume_file, e))
				);
				Self::default()
			}
		}
	}

	async fn store(self, thumbnails_directory: impl AsRef<Path>) {
		let resume_file = thumbnails_directory.as_ref().join(SAVE_STATE_FILE);

		info!(
			"Saving thumbnailer actor state: Existing ephemeral thumbs: {}; \
			Queued batches waiting processing: {}",
			self.ephemeral_file_names.len(),
			self.queue.len()
				+ self.indexed_leftovers_queue.len()
				+ self.ephemeral_leftovers_queue.len()
		);

		let Ok(bytes) = rmp_serde::to_vec_named(&self).map_err(|e| {
			error!("Failed to serialize save state at thumbnailer actor: {e:#?}");
		}) else {
			return;
		};

		if let Err(e) = fs::write(&resume_file, bytes).await {
			error!(
				"Failed to write save state at thumbnailer actor: {:#?}",
				FileIOError::from((resume_file, e))
			);
		}
	}
}
