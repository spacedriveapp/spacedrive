use crate::{api::CoreEvent, node::config::NodePreferences};

use sd_prisma::prisma::location;

use std::{collections::HashMap, ffi::OsString, path::PathBuf, pin::pin, sync::Arc};

use async_channel as chan;
use futures_concurrency::stream::Merge;
use tokio::{
	spawn,
	sync::{broadcast, oneshot, watch},
	time::{interval, interval_at, timeout, Instant, MissedTickBehavior},
};
use tokio_stream::{
	wrappers::{IntervalStream, WatchStream},
	StreamExt,
};
use tracing::{debug, error, trace};

use super::{
	clean_up::{process_ephemeral_clean_up, process_indexed_clean_up},
	old_actor::DatabaseMessage,
	preferences::ThumbnailerPreferences,
	process::{batch_processor, ProcessorControlChannels},
	state::{remove_by_cas_ids, OldThumbsProcessingSaveState, RegisterReporter},
	BatchToProcess, ThumbnailKind, HALF_HOUR, ONE_SEC, THIRTY_SECS,
};

#[derive(Debug, Clone)]
pub(super) struct WorkerChannels {
	pub(super) progress_management_rx: chan::Receiver<RegisterReporter>,
	pub(super) databases_rx: chan::Receiver<DatabaseMessage>,
	pub(super) cas_ids_to_delete_rx: chan::Receiver<(Vec<String>, ThumbnailKind)>,
	pub(super) thumbnails_to_generate_rx: chan::Receiver<(BatchToProcess, ThumbnailKind)>,
	pub(super) cancel_rx: chan::Receiver<oneshot::Sender<()>>,
}

pub(super) async fn old_worker(
	available_parallelism: usize,
	node_preferences_rx: watch::Receiver<NodePreferences>,
	reporter: broadcast::Sender<CoreEvent>,
	thumbnails_directory: Arc<PathBuf>,
	WorkerChannels {
		progress_management_rx,
		databases_rx,
		cas_ids_to_delete_rx,
		thumbnails_to_generate_rx,
		cancel_rx,
	}: WorkerChannels,
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
		ProgressManagement(RegisterReporter),
		BatchProgress((location::id::Type, u32)),
		Shutdown(oneshot::Sender<()>),
		UpdatedPreferences(ThumbnailerPreferences),
		IdleTick,
	}

	let OldThumbsProcessingSaveState {
		mut bookkeeper,
		mut ephemeral_file_names,
		mut queue,
		mut indexed_leftovers_queue,
		mut ephemeral_leftovers_queue,
	} = OldThumbsProcessingSaveState::load(thumbnails_directory.as_ref()).await;

	let (generated_ephemeral_thumbnails_tx, ephemeral_thumbnails_cas_ids_rx) = chan::bounded(32);
	let (leftovers_tx, leftovers_rx) = chan::bounded(8);
	let (batch_report_progress_tx, batch_report_progress_rx) = chan::bounded(8);
	let (stop_older_processing_tx, stop_older_processing_rx) = chan::bounded(1);

	let mut shutdown_leftovers_rx = pin!(leftovers_rx.clone());
	let mut shutdown_batch_report_progress_rx = pin!(batch_report_progress_rx.clone());

	let mut current_batch_processing_rx: Option<oneshot::Receiver<()>> = None;

	let mut msg_stream = pin!((
		IntervalStream::new(to_remove_interval).map(|_| StreamMessage::RemovalTick),
		cas_ids_to_delete_rx.map(StreamMessage::ToDelete),
		databases_rx.map(StreamMessage::Database),
		thumbnails_to_generate_rx.map(StreamMessage::NewBatch),
		leftovers_rx.map(StreamMessage::Leftovers),
		ephemeral_thumbnails_cas_ids_rx.map(StreamMessage::NewEphemeralThumbnailsFilenames),
		progress_management_rx.map(StreamMessage::ProgressManagement),
		batch_report_progress_rx.map(StreamMessage::BatchProgress),
		cancel_rx.map(StreamMessage::Shutdown),
		IntervalStream::new(idle_interval).map(|_| StreamMessage::IdleTick),
		WatchStream::new(node_preferences_rx).map(|node_preferences| {
			StreamMessage::UpdatedPreferences(node_preferences.thumbnailer)
		}),
	)
		.merge());

	let mut thumbnailer_preferences = ThumbnailerPreferences::default();

	while let Some(msg) = msg_stream.next().await {
		match msg {
			StreamMessage::IdleTick => {
				if let Some(done_rx) = current_batch_processing_rx.as_mut() {
					// Checking if the previous run finished or was aborted to clean state
					match done_rx.try_recv() {
						Ok(()) | Err(oneshot::error::TryRecvError::Closed) => {
							current_batch_processing_rx = None;
						}

						Err(oneshot::error::TryRecvError::Empty) => {
							// The previous run is still running
							continue;
						}
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
					} else if let Some((batch, library_id)) = indexed_leftovers_queue.pop_front() {
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
							batch_report_progress_tx: batch_report_progress_tx.clone(),
						},
						leftovers_tx.clone(),
						reporter.clone(),
						(available_parallelism, thumbnailer_preferences.clone()),
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
					if let Err(e) = remove_by_cas_ids(&thumbnails_directory, cas_ids, kind).await {
						error!("Got an error when trying to remove thumbnails: {e:#?}");
					}
				}
			}

			StreamMessage::NewBatch((batch, kind)) => {
				let in_background = batch.in_background;

				if let Some(location_id) = batch.location_id {
					bookkeeper
						.add_work(location_id, batch.batch.len() as u32)
						.await;
				}

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
				if !in_background {
					stop_batch(
						&current_batch_processing_rx,
						&stop_older_processing_tx,
						&stop_older_processing_rx,
					)
					.await;
				}
			}

			StreamMessage::Leftovers((batch, ThumbnailKind::Indexed(library_id))) => {
				indexed_leftovers_queue.push_back((batch, library_id))
			}

			StreamMessage::Leftovers((batch, ThumbnailKind::Ephemeral)) => {
				ephemeral_leftovers_queue.push_back(batch)
			}

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

			StreamMessage::BatchProgress((location_id, progressed)) => {
				bookkeeper.add_progress(location_id, progressed).await;
			}

			StreamMessage::Shutdown(cancel_tx) => {
				debug!("Thumbnail actor is shutting down...");
				let start = Instant::now();

				stop_batch(
					&current_batch_processing_rx,
					&stop_older_processing_tx,
					&stop_older_processing_rx,
				)
				.await;

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

				// Consuming the last progress reports to keep everything up to date
				shutdown_batch_report_progress_rx.close();
				while let Some((location_id, progressed)) =
					shutdown_batch_report_progress_rx.next().await
				{
					bookkeeper.add_progress(location_id, progressed).await;
				}

				// Saving state
				OldThumbsProcessingSaveState {
					bookkeeper,
					ephemeral_file_names,
					queue,
					indexed_leftovers_queue,
					ephemeral_leftovers_queue,
				}
				.store(thumbnails_directory.as_ref())
				.await;

				// Signaling that we're done shutting down
				cancel_tx.send(()).ok();

				debug!("Thumbnailer has been shutdown in {:?}", start.elapsed());
				return;
			}

			StreamMessage::ProgressManagement((location_id, progress_tx)) => {
				bookkeeper.register_reporter(location_id, progress_tx);
			}

			StreamMessage::UpdatedPreferences(preferences) => {
				thumbnailer_preferences = preferences;
				stop_batch(
					&current_batch_processing_rx,
					&stop_older_processing_tx,
					&stop_older_processing_rx,
				)
				.await;
			}
		}
	}
}

#[inline]
async fn stop_batch(
	current_batch_processing_rx: &Option<oneshot::Receiver<()>>,
	stop_older_processing_tx: &chan::Sender<oneshot::Sender<()>>,
	stop_older_processing_rx: &chan::Receiver<oneshot::Sender<()>>,
) {
	// First stopping the current batch processing
	if current_batch_processing_rx.is_some() {
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
				error!("Thumbnail actor died when trying to stop older processing");
			}
		}
	}
}
