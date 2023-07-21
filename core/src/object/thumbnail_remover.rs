use crate::{
	library::Library,
	prisma::{file_path, PrismaClient},
	util::error::{FileIOError, NonUtf8PathError},
};

use std::{
	collections::{HashMap, HashSet},
	ffi::OsStr,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use futures::{stream::FuturesUnordered, FutureExt};
use futures_concurrency::{future::TryJoin, stream::Merge};
use thiserror::Error;
use tokio::{
	fs,
	time::{interval_at, Instant, MissedTickBehavior},
};
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{debug, error, trace};
use uuid::Uuid;

const TEN_SECONDS: Duration = Duration::from_secs(10);
const FIVE_MINUTES: Duration = Duration::from_secs(5 * 60);

#[derive(Error, Debug)]
enum ThumbnailRemoverActorError {
	#[error("database error")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("missing file name: {}", .0.display())]
	MissingFileName(Box<Path>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
}

#[derive(Clone)]
pub struct ThumbnailRemoverActorProxy {
	invoker: chan::Sender<()>,
	non_indexed_thumbnails_cas_ids_tx: chan::Sender<String>,
}

impl ThumbnailRemoverActorProxy {
	pub async fn invoke(&self) {
		if self.invoker.send(()).await.is_err() {
			error!("Thumbnail remover actor is dead");
		}
	}

	pub async fn new_non_indexed_thumbnail(&self, cas_id: String) {
		if self
			.non_indexed_thumbnails_cas_ids_tx
			.send(cas_id)
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead");
		}
	}
}

enum DatabaseMessage {
	Add(Uuid, Arc<PrismaClient>),
	Remove(Uuid),
}

pub struct ThumbnailRemoverActor {
	invoke_tx: chan::Sender<()>,
	databases_tx: chan::Sender<DatabaseMessage>,
	non_indexed_thumbnails_cas_ids_tx: chan::Sender<String>,
	_cancel_loop: DropGuard,
}

impl ThumbnailRemoverActor {
	pub fn new(thumbnails_directory: impl AsRef<Path>) -> Self {
		let (invoke_tx, invoke_rx) = chan::bounded(4);
		let thumbnails_directory = thumbnails_directory.as_ref().to_path_buf();
		let (databases_tx, databases_rx) = chan::bounded(4);
		let (non_indexed_thumbnails_cas_ids_tx, non_indexed_thumbnails_cas_ids_rx) =
			chan::unbounded();
		let cancel_token = CancellationToken::new();

		let inner_cancel_token = cancel_token.child_token();
		tokio::spawn(async move {
			loop {
				if let Err(e) = tokio::spawn(Self::worker(
					thumbnails_directory.clone(),
					invoke_rx.clone(),
					databases_rx.clone(),
					non_indexed_thumbnails_cas_ids_rx.clone(),
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

		Self {
			invoke_tx,
			databases_tx,
			non_indexed_thumbnails_cas_ids_tx,
			_cancel_loop: cancel_token.drop_guard(),
		}
	}

	pub async fn new_library(&self, Library { id, db, .. }: &Library) {
		if self
			.databases_tx
			.send(DatabaseMessage::Add(*id, Arc::clone(db)))
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead")
		}
	}

	pub async fn remove_library(&self, library_id: Uuid) {
		if self
			.databases_tx
			.send(DatabaseMessage::Remove(library_id))
			.await
			.is_err()
		{
			error!("Thumbnail remover actor is dead")
		}
	}

	pub fn proxy(&self) -> ThumbnailRemoverActorProxy {
		ThumbnailRemoverActorProxy {
			invoker: self.invoke_tx.clone(),
			non_indexed_thumbnails_cas_ids_tx: self.non_indexed_thumbnails_cas_ids_tx.clone(),
		}
	}

	async fn worker(
		thumbnails_directory: PathBuf,
		invoke_rx: chan::Receiver<()>,
		databases_rx: chan::Receiver<DatabaseMessage>,
		non_indexed_thumbnails_cas_ids_rx: chan::Receiver<String>,
		cancel_token: CancellationToken,
	) {
		let mut last_checked = Instant::now();

		let mut check_interval = interval_at(Instant::now() + FIVE_MINUTES, FIVE_MINUTES);
		check_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut databases = HashMap::new();
		let mut non_indexed_thumbnails_cas_ids = HashSet::new();

		enum StreamMessage {
			Invoke,
			Database(DatabaseMessage),
			NonIndexedThumbnail(String),
			Stop,
		}

		let cancel = pin!(cancel_token.cancelled());

		let mut msg_stream = (
			invoke_rx.map(|()| StreamMessage::Invoke),
			databases_rx.map(StreamMessage::Database),
			non_indexed_thumbnails_cas_ids_rx.map(StreamMessage::NonIndexedThumbnail),
			IntervalStream::new(check_interval).map(|_| StreamMessage::Invoke),
			cancel.into_stream().map(|()| StreamMessage::Stop),
		)
			.merge();

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::Invoke => {
					// For any of them we process a clean up if a time since the last one already passed
					if last_checked.elapsed() > TEN_SECONDS && !databases.is_empty() {
						if let Err(e) = Self::process_clean_up(
							&thumbnails_directory,
							databases.values(),
							&non_indexed_thumbnails_cas_ids,
						)
						.await
						{
							error!("Got an error when trying to clean stale thumbnails: {e:#?}");
						}
						last_checked = Instant::now();
					}
				}
				StreamMessage::Database(DatabaseMessage::Add(id, db)) => {
					databases.insert(id, db);
				}
				StreamMessage::Database(DatabaseMessage::Remove(id)) => {
					databases.remove(&id);
				}
				StreamMessage::NonIndexedThumbnail(cas_id) => {
					non_indexed_thumbnails_cas_ids.insert(cas_id);
				}
				StreamMessage::Stop => {
					debug!("Thumbnail remover actor is stopping");
					break;
				}
			}
		}
	}

	async fn process_clean_up(
		thumbnails_directory: &Path,
		databases: impl Iterator<Item = &Arc<PrismaClient>>,
		non_indexed_thumbnails_cas_ids: &HashSet<String>,
	) -> Result<(), ThumbnailRemoverActorError> {
		let databases = databases.collect::<Vec<_>>();

		// Thumbnails directory have the following structure:
		// thumbnails/
		// ├── version.txt
		//└── <cas_id>[0..2]/ # sharding
		//    └── <cas_id>[2..].webp

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

			let entry_path_name = entry_path
				.file_name()
				.ok_or_else(|| {
					ThumbnailRemoverActorError::MissingFileName(entry.path().into_boxed_path())
				})?
				.to_str()
				.ok_or_else(|| NonUtf8PathError(entry.path().into_boxed_path()))?;

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
					.ok_or_else(|| {
						ThumbnailRemoverActorError::MissingFileName(entry.path().into_boxed_path())
					})?
					.to_str()
					.ok_or_else(|| NonUtf8PathError(entry.path().into_boxed_path()))?;

				thumbnails_paths_by_cas_id
					.insert(format!("{}{}", entry_path_name, thumbnail_name), thumb_path);
			}

			if thumbnails_paths_by_cas_id.is_empty() {
				fs::remove_dir(&entry_path)
					.await
					.map_err(|e| FileIOError::from((entry_path, e)))?;

				continue;
			}

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

			thumbnails_paths_by_cas_id
				.into_values()
				.map(|path| async move {
					trace!("Removing stale thumbnail: {}", path.display());
					fs::remove_file(&path)
						.await
						.map_err(|e| FileIOError::from((path, e)))
				})
				.collect::<Vec<_>>()
				.try_join()
				.await?;
		}

		Ok(())
	}
}
