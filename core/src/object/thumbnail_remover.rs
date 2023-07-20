use crate::{
	prisma::{file_path, PrismaClient},
	util::error::{FileIOError, NonUtf8PathError},
};

use std::{collections::HashSet, ffi::OsStr, path::Path, sync::Arc, time::Duration};

use futures::future::try_join_all;
use thiserror::Error;
use tokio::{
	fs, select,
	sync::mpsc,
	time::{interval_at, Instant, MissedTickBehavior},
};
use tracing::error;

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
pub struct ThumbnailRemoverActor {
	tx: mpsc::Sender<()>,
}

impl ThumbnailRemoverActor {
	pub fn spawn(db: Arc<PrismaClient>, thumbnails_directory: impl AsRef<Path>) -> Self {
		let (tx, mut rx) = mpsc::channel(4);
		let thumbnails_directory = thumbnails_directory.as_ref().to_path_buf();

		tokio::spawn(async move {
			let mut last_checked = Instant::now();

			let mut check_interval = interval_at(Instant::now() + FIVE_MINUTES, FIVE_MINUTES);
			check_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

			loop {
				// Here we wait for a signal or for the tick interval to be reached
				select! {
					_ =  check_interval.tick() => {}
					signal = rx.recv() => {
						if signal.is_none() {
							break;
						}
					}
				}

				// For any of them we process a clean up if a time since the last one already passed
				if last_checked.elapsed() > TEN_SECONDS {
					if let Err(e) = Self::process_clean_up(&db, &thumbnails_directory).await {
						error!("Got an error when trying to clean stale thumbnails: {e:#?}");
					}
					last_checked = Instant::now();
				}
			}
		});

		Self { tx }
	}

	pub async fn invoke(&self) {
		self.tx.send(()).await.ok();
	}

	async fn process_clean_up(
		db: &PrismaClient,
		thumbnails_directory: &Path,
	) -> Result<(), ThumbnailRemoverActorError> {
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

			let mut thumbnails_paths_by_cas_id = Vec::new();

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
					.push((format!("{}{}", entry_path_name, thumbnail_name), thumb_path));
			}

			if thumbnails_paths_by_cas_id.is_empty() {
				fs::remove_dir(&entry_path)
					.await
					.map_err(|e| FileIOError::from((entry_path, e)))?;

				continue;
			}

			let thumbs_in_db = db
				.file_path()
				.find_many(vec![file_path::cas_id::in_vec(
					thumbnails_paths_by_cas_id
						.iter()
						.map(|(cas_id, _)| cas_id)
						.cloned()
						.collect(),
				)])
				.select(file_path::select!({ cas_id }))
				.exec()
				.await?
				.into_iter()
				.map(|file_path| {
					file_path
						.cas_id
						.expect("only file paths with a cas_id were queried")
				})
				.collect::<HashSet<_>>();

			try_join_all(
				thumbnails_paths_by_cas_id
					.into_iter()
					.filter_map(|(cas_id, path)| {
						(!thumbs_in_db.contains(&cas_id)).then_some(async move {
							fs::remove_file(&path)
								.await
								.map_err(|e| FileIOError::from((path, e)))
						})
					}),
			)
			.await?;
		}

		Ok(())
	}
}
