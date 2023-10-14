use crate::{
	api::CoreEvent,
	library::{Libraries, LibraryManagerEvent},
	object::media::thumbnail::{
		can_generate_thumbnail_for_document, can_generate_thumbnail_for_image,
		generate_image_thumbnail, get_shard_hex, get_thumb_key, ThumbnailerError,
	},
	util::error::{FileIOError, NonUtf8PathError},
};

use sd_file_ext::extensions::{DocumentExtension, ImageExtension};
use sd_prisma::prisma::{file_path, PrismaClient};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	ffi::OsStr,
	path::{Path, PathBuf},
	pin::pin,
	str::FromStr,
	sync::Arc,
	time::{Duration, SystemTime},
};

use async_channel as chan;
use futures::{stream::FuturesUnordered, FutureExt};
use futures_concurrency::{
	future::{Join, Race, TryJoin},
	stream::Merge,
};
use once_cell::sync::OnceCell;
use thiserror::Error;
use tokio::{
	fs, io, spawn,
	sync::{broadcast, oneshot, Mutex},
	time::{interval, interval_at, sleep, timeout, Instant, MissedTickBehavior},
};
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

use super::{init_thumbnail_dir, THUMBNAIL_CACHE_DIR_NAME};

const ONE_SEC: Duration = Duration::from_secs(1);
const THIRTY_SECS: Duration = Duration::from_secs(30);
const HALF_HOUR: Duration = Duration::from_secs(30 * 60);
const ONE_WEEK: Duration = Duration::from_secs(7 * 24 * 60 * 60);

static BATCH_SIZE: OnceCell<usize> = OnceCell::new();

#[derive(Debug)]
pub struct GenerateThumbnailArgs {
	pub extension: String,
	pub cas_id: String,
	pub path: PathBuf,
}

impl GenerateThumbnailArgs {
	pub fn new(extension: String, cas_id: String, path: PathBuf) -> Self {
		Self {
			extension,
			cas_id,
			path,
		}
	}
}

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
	Update(Uuid, Arc<PrismaClient>),
	Remove(Uuid),
}

#[derive(Debug)]
pub struct BatchToProcess {
	pub batch: Vec<GenerateThumbnailArgs>,
	pub should_regenerate: bool,
	pub in_background: bool,
}

#[derive(Debug)]
enum ProcessingKind {
	Indexed,
	Ephemeral,
}

// Thumbnails directory have the following structure:
// thumbnails/
// ├── version.txt
// └── <cas_id>[0..2]/ # sharding
//    └── <cas_id>.webp
pub struct Thumbnailer {
	thumbnails_directory: PathBuf,
	cas_ids_to_delete_tx: chan::Sender<Vec<String>>,
	thumbnails_to_generate_tx: chan::Sender<(BatchToProcess, ProcessingKind)>,
	last_single_thumb_generated: Mutex<Instant>,
	reporter: broadcast::Sender<CoreEvent>,
	_cancel_loop: DropGuard,
}

impl Thumbnailer {
	pub async fn new(
		data_dir: PathBuf,
		lm: Arc<Libraries>,
		reporter: broadcast::Sender<CoreEvent>,
	) -> Self {
		let thumbnails_directory = init_thumbnail_dir(&data_dir).await.unwrap_or_else(|e| {
			error!("Failed to initialize thumbnail directory: {e:#?}");
			let mut thumbnails_directory = data_dir;
			thumbnails_directory.push(THUMBNAIL_CACHE_DIR_NAME);
			thumbnails_directory
		});

		let (databases_tx, databases_rx) = chan::bounded(4);
		let (thumbnails_to_generate_tx, ephemeral_thumbnails_to_generate_rx) = chan::unbounded();
		let (cas_ids_to_delete_tx, cas_ids_to_delete_rx) = chan::bounded(16);
		let cancel_token = CancellationToken::new();

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

		let inner_cancel_token = cancel_token.child_token();
		let inner_thumbnails_directory = thumbnails_directory.clone();
		let inner_reporter = reporter.clone();
		tokio::spawn(async move {
			loop {
				if let Err(e) = tokio::spawn(Self::worker(
					inner_reporter.clone(),
					inner_thumbnails_directory.clone(),
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
			_cancel_loop: cancel_token.drop_guard(),
		}
	}

	async fn worker(
		reporter: broadcast::Sender<CoreEvent>,
		thumbnails_directory: PathBuf,
		databases_rx: chan::Receiver<DatabaseMessage>,
		cas_ids_to_delete_rx: chan::Receiver<Vec<String>>,
		thumbnails_to_generate_rx: chan::Receiver<(BatchToProcess, ProcessingKind)>,
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
			NewBatch((BatchToProcess, ProcessingKind)),
			Leftovers((BatchToProcess, ProcessingKind)),
			NewEphemeralThumbnailCasIds(Vec<String>),
			Stop,
			IdleTick,
		}

		let cancel = pin!(cancel_token.cancelled());

		// These are FIFO queues, so we can process leftovers from the previous batch first
		let mut queue = VecDeque::with_capacity(32);

		let mut ephemeral_leftovers_queue = VecDeque::with_capacity(8);
		let mut indexed_leftovers_queue = VecDeque::with_capacity(8);

		let (ephemeral_thumbnails_cas_ids_tx, ephemeral_thumbnails_cas_ids_rx) = chan::bounded(32);
		let (leftovers_tx, leftovers_rx) = chan::bounded(8);

		let (stop_older_processing_tx, stop_older_processing_rx) = chan::bounded(1);

		let mut current_batch_processing_rx: Option<oneshot::Receiver<()>> = None;

		let mut msg_stream = (
			databases_rx.map(StreamMessage::Database),
			cas_ids_to_delete_rx.map(StreamMessage::ToDelete),
			thumbnails_to_generate_rx.map(StreamMessage::NewBatch),
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
						&& (!queue.is_empty()
							|| !indexed_leftovers_queue.is_empty()
							|| !ephemeral_leftovers_queue.is_empty())
					{
						let (done_tx, done_rx) = oneshot::channel();
						current_batch_processing_rx = Some(done_rx);

						let batch_and_kind = if let Some(batch_and_kind) = queue.pop_front() {
							batch_and_kind
						} else if let Some(batch) = indexed_leftovers_queue.pop_front() {
							// indexed leftovers have bigger priority
							(batch, ProcessingKind::Indexed)
						} else if let Some(batch) = ephemeral_leftovers_queue.pop_front() {
							(batch, ProcessingKind::Ephemeral)
						} else {
							continue;
						};

						spawn(batch_processor(
							thumbnails_directory.clone(),
							batch_and_kind,
							ephemeral_thumbnails_cas_ids_tx.clone(),
							ProcessorControlChannels {
								stop_rx: stop_older_processing_rx.clone(),
								done_tx,
							},
							leftovers_tx.clone(),
							reporter.clone(),
						));
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

				StreamMessage::NewBatch((batch, kind)) => {
					let in_background = batch.in_background;

					tracing::debug!(
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
						tracing::debug!("Sending stop signal to older processing");
						let (tx, rx) = oneshot::channel();
						if stop_older_processing_tx.send(tx).await.is_err() {
							error!(
								"Thumbnail remover actor died when trying to stop older processing"
							);
						}
						rx.await.ok();
					}
				}

				StreamMessage::Leftovers((batch, kind)) => match kind {
					ProcessingKind::Indexed => indexed_leftovers_queue.push_back(batch),
					ProcessingKind::Ephemeral => ephemeral_leftovers_queue.push_back(batch),
				},

				StreamMessage::Database(DatabaseMessage::Add(id, db))
				| StreamMessage::Database(DatabaseMessage::Update(id, db)) => {
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
		cas_ids
			.into_iter()
			.map(|cas_id| async move {
				let thumbnail_path =
					thumbnails_directory.join(format!("{}/{cas_id}.webp", &cas_id[0..2]));

				trace!("Removing thumbnail: {}", thumbnail_path.display());

				match fs::remove_file(&thumbnail_path).await {
					Ok(()) => Ok(()),
					Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
					Err(e) => Err(FileIOError::from((thumbnail_path, e))),
				}
			})
			.collect::<Vec<_>>()
			.try_join()
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

			let removed_count = thumbnails_paths_by_cas_id
				.into_values()
				.map(|path| async move {
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

					trace!("Removing stale thumbnail: {}", path.display());
					fs::remove_file(&path)
						.await
						.map(|()| true)
						.map_err(|e| FileIOError::from((path, e)))
				})
				.collect::<Vec<_>>()
				.try_join()
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

	#[inline]
	async fn new_batch(&self, batch: BatchToProcess, kind: ProcessingKind) {
		if self
			.thumbnails_to_generate_tx
			.send((batch, kind))
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead: Failed to send new batch");
		}
	}

	pub async fn new_ephemeral_thumbnails_batch(&self, batch: BatchToProcess) {
		self.new_batch(batch, ProcessingKind::Ephemeral).await;
	}

	pub async fn new_indexed_thumbnails_batch(&self, batch: BatchToProcess) {
		self.new_batch(batch, ProcessingKind::Indexed).await;
	}

	pub async fn remove_cas_ids(&self, cas_ids: Vec<String>) {
		if self.cas_ids_to_delete_tx.send(cas_ids).await.is_err() {
			error!("Thumbnail remover actor is dead: Failed to send cas ids to delete");
		}
	}

	/// WARNING!!!! DON'T USE THIS METHOD IN A LOOP!!!!!!!!!!!!! It will be pretty slow on purpose!
	pub async fn generate_single_thumbnail(
		&self,
		extension: &str,
		cas_id: String,
		path: impl AsRef<Path>,
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
			extension,
			cas_id,
			path,
			false,
			false,
			self.reporter.clone(),
		)
		.await
		.map(|_| ());

		*last_single_thumb_generated_guard = Instant::now();

		res
	}
}

struct ProcessorControlChannels {
	stop_rx: chan::Receiver<oneshot::Sender<()>>,
	done_tx: oneshot::Sender<()>,
}

async fn batch_processor(
	thumbnails_directory: PathBuf,
	(
		BatchToProcess {
			batch,
			should_regenerate,
			in_background,
		},
		kind,
	): (BatchToProcess, ProcessingKind),
	generated_cas_ids_tx: chan::Sender<Vec<String>>,
	ProcessorControlChannels { stop_rx, done_tx }: ProcessorControlChannels,
	leftovers_tx: chan::Sender<(BatchToProcess, ProcessingKind)>,
	reporter: broadcast::Sender<CoreEvent>,
) {
	tracing::debug!(
		"Processing thumbnails batch of kind {kind:?} with size {} in {}",
		batch.len(),
		if in_background {
			"background"
		} else {
			"foreground"
		},
	);

	// Tranforming to `VecDeque` so we don't need to move anything as we consume from the beginning
	// This from is guaranteed to be O(1)
	let mut queue = VecDeque::from(batch);

	enum RaceOutputs {
		Processed,
		Stop(oneshot::Sender<()>),
	}

	// Need this borrow here to satisfy the async move below
	let generated_cas_ids_tx = &generated_cas_ids_tx;

	while !queue.is_empty() {
		let chunk = (0..*BATCH_SIZE
			.get()
			.expect("BATCH_SIZE is set at thumbnailer new method"))
			.filter_map(|_| queue.pop_front())
			.map(
				|GenerateThumbnailArgs {
				     extension,
				     cas_id,
				     path,
				 }| {
					let reporter = reporter.clone();
					let thumbnails_directory = thumbnails_directory.clone();
					spawn(async move {
						timeout(
							THIRTY_SECS,
							generate_thumbnail(
								thumbnails_directory,
								&extension,
								cas_id,
								&path,
								in_background,
								should_regenerate,
								reporter,
							),
						)
						.await
						.unwrap_or_else(|_| Err(ThumbnailerError::TimedOut(path.into_boxed_path())))
					})
				},
			)
			.collect::<Vec<_>>();

		if let RaceOutputs::Stop(tx) = (
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

				tracing::debug!("Processed chunk of thumbnails");
				RaceOutputs::Processed
			},
			async {
				let tx = stop_rx
					.recv()
					.await
					.expect("Critical error on thumbnails actor");
				tracing::debug!("Received a stop signal");
				RaceOutputs::Stop(tx)
			},
		)
			.race()
			.await
		{
			// Our queue is always contiguous, so this `from` is free
			let leftovers = Vec::from(queue);

			tracing::debug!(
				"Stopped with {} thumbnails left to process",
				leftovers.len()
			);
			if !leftovers.is_empty()
				&& leftovers_tx
					.send((
						BatchToProcess {
							batch: leftovers,
							should_regenerate,
							in_background: true, // Leftovers should always be in background
						},
						kind,
					))
					.await
					.is_err()
			{
				error!("Thumbnail remover actor is dead: Failed to send leftovers")
			}

			done_tx.send(()).ok();
			tx.send(()).ok();

			return;
		}
	}

	tracing::debug!("Finished batch!");

	done_tx.send(()).ok();
}

async fn generate_thumbnail(
	thumbnails_directory: PathBuf,
	extension: &str,
	cas_id: String,
	path: impl AsRef<Path>,
	in_background: bool,
	should_regenerate: bool,
	reporter: broadcast::Sender<CoreEvent>,
) -> Result<String, ThumbnailerError> {
	let path = path.as_ref();
	trace!("Generating thumbnail for {}", path.display());

	let mut output_path = thumbnails_directory;
	output_path.push(get_shard_hex(&cas_id));
	output_path.push(&cas_id);
	output_path.set_extension("webp");

	if let Err(e) = fs::metadata(&output_path).await {
		if e.kind() != io::ErrorKind::NotFound {
			error!(
				"Failed to check if thumbnail exists, but we will try to generate it anyway: {e:#?}"
			);
		}
	// Otherwise we good, thumbnail doesn't exist so we can generate it
	} else if !should_regenerate {
		trace!(
			"Skipping thumbnail generation for {} because it already exists",
			path.display()
		);
		return Ok(cas_id);
	}

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(&extension) {
			generate_image_thumbnail(&path, &output_path).await?;
		}
	} else if let Ok(extension) = DocumentExtension::from_str(extension) {
		if can_generate_thumbnail_for_document(&extension) {
			generate_image_thumbnail(&path, &output_path).await?;
		}
	}

	#[cfg(feature = "ffmpeg")]
	{
		use crate::object::media::thumbnail::{
			can_generate_thumbnail_for_video, generate_video_thumbnail,
		};
		use sd_file_ext::extensions::VideoExtension;

		if let Ok(extension) = VideoExtension::from_str(extension) {
			if can_generate_thumbnail_for_video(&extension) {
				generate_video_thumbnail(&path, &output_path).await?;
			}
		}
	}

	if !in_background {
		trace!("Emitting new thumbnail event");
		if reporter
			.send(CoreEvent::NewThumbnail {
				thumb_key: get_thumb_key(&cas_id),
			})
			.is_err()
		{
			warn!("Error sending event to Node's event bus");
		}
	}

	trace!("Generated thumbnail for {}", path.display());

	Ok(cas_id)
}
