use crate::{library::LibraryId, util::error::FileIOError};

use std::{
	collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
	ffi::OsString,
	path::Path,
};

use async_channel as chan;
use futures_concurrency::future::TryJoin;
use sd_prisma::prisma::location;
use serde::{Deserialize, Serialize};
use tokio::{fs, io};
use tracing::{error, info, trace};

use super::{
	actor::ActorError, get_shard_hex, BatchToProcess, ThumbnailKind, EPHEMERAL_DIR, SAVE_STATE_FILE,
};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ThumbsProcessingSaveState {
	pub(super) bookkeeper: BookKeeper,
	pub(super) ephemeral_file_names: HashSet<OsString>,
	// This queues doubles as LIFO and FIFO, assuming LIFO in case of users asking for a new batch
	// by entering a new directory in the explorer, otherwise processing as FIFO
	pub(super) queue: VecDeque<(BatchToProcess, ThumbnailKind)>,
	// These below are FIFO queues, so we can process leftovers from the previous batch first
	pub(super) indexed_leftovers_queue: VecDeque<(BatchToProcess, LibraryId)>,
	pub(super) ephemeral_leftovers_queue: VecDeque<BatchToProcess>,
}

impl Default for ThumbsProcessingSaveState {
	fn default() -> Self {
		Self {
			bookkeeper: BookKeeper::default(),
			ephemeral_file_names: HashSet::with_capacity(128),
			queue: VecDeque::with_capacity(32),
			indexed_leftovers_queue: VecDeque::with_capacity(8),
			ephemeral_leftovers_queue: VecDeque::with_capacity(8),
		}
	}
}

impl ThumbsProcessingSaveState {
	pub(super) async fn load(thumbnails_directory: impl AsRef<Path>) -> Self {
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

	pub(super) async fn store(self, thumbnails_directory: impl AsRef<Path>) {
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

pub(super) async fn remove_by_cas_ids(
	thumbnails_directory: &Path,
	cas_ids: Vec<String>,
	kind: ThumbnailKind,
) -> Result<(), ActorError> {
	let base_dir = match kind {
		ThumbnailKind::Ephemeral => thumbnails_directory.join(EPHEMERAL_DIR),
		ThumbnailKind::Indexed(library_id) => thumbnails_directory.join(library_id.to_string()),
	};

	cas_ids
		.into_iter()
		.map(|cas_id| {
			let thumbnail_path = base_dir.join(format!("{}/{cas_id}.webp", get_shard_hex(&cas_id)));

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

pub(super) type RegisterReporter = (location::id::Type, chan::Sender<(u32, u32)>);

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct BookKeeper {
	work_progress: HashMap<location::id::Type, (u32, u32)>, // (pending, total)

	// We can't save reporter function or a channel to disk, the job must ask again to be registered
	#[serde(skip, default)]
	reporter_by_location: HashMap<location::id::Type, chan::Sender<(u32, u32)>>,
}
impl Default for BookKeeper {
	fn default() -> Self {
		Self {
			work_progress: HashMap::with_capacity(8),
			reporter_by_location: HashMap::with_capacity(8),
		}
	}
}

impl BookKeeper {
	pub(super) async fn add_work(&mut self, location_id: location::id::Type, thumbs_count: u32) {
		let (in_progress, total) = match self.work_progress.entry(location_id) {
			Entry::Occupied(mut entry) => {
				let (in_progress, total) = entry.get_mut();

				*total += thumbs_count;

				(*in_progress, *total)
			}
			Entry::Vacant(entry) => {
				entry.insert((0, thumbs_count));

				(0, thumbs_count)
			}
		};

		if let Some(progress_tx) = self.reporter_by_location.get(&location_id) {
			if progress_tx.send((in_progress, total)).await.is_err() {
				error!(
					"Failed to send progress update to reporter on location <id='{location_id}'>"
				);
			}
		}
	}

	pub(super) fn register_reporter(
		&mut self,
		location_id: location::id::Type,
		reporter_tx: chan::Sender<(u32, u32)>,
	) {
		self.reporter_by_location.insert(location_id, reporter_tx);
	}

	pub(super) async fn add_progress(&mut self, location_id: location::id::Type, progress: u32) {
		if let Some((current_progress, total)) = self.work_progress.get_mut(&location_id) {
			*current_progress += progress;

			if *current_progress == *total {
				if let Some(progress_tx) = self.reporter_by_location.remove(&location_id) {
					if progress_tx.send((*current_progress, *total)).await.is_err() {
						error!(
							"Failed to send progress update to reporter on location <id='{location_id}'>"
						);
					}
				}

				self.work_progress.remove(&location_id);
			} else if let Some(progress_tx) = self.reporter_by_location.get(&location_id) {
				if progress_tx.send((*current_progress, *total)).await.is_err() {
					error!(
						"Failed to send progress update to reporter on location <id='{location_id}'>"
					);
				}
			}
		}
	}
}
