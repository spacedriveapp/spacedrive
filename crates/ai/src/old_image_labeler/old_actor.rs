use sd_core_prisma_helpers::file_path_for_media_processor;
use sd_core_sync::SyncManager;

use sd_prisma::prisma::{device, location, PrismaClient};
use sd_utils::error::FileIOError;

use std::{
	cell::RefCell,
	collections::{HashMap, VecDeque},
	ops::Deref,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use futures::stream::StreamExt;
use futures_concurrency::stream::Merge;
use serde::{Deserialize, Serialize};
use tokio::{
	fs, io, spawn,
	sync::{oneshot, RwLock},
	task::JoinHandle,
	time::timeout,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::{
	model::{Model, ModelAndSession},
	process::{spawned_processing, FinishStatus},
	BatchToken, ImageLabelerError, LabelerOutput,
};

const ONE_SEC: Duration = Duration::from_secs(1);
const PENDING_BATCHES_FILE: &str = "pending_image_labeler_batches.bin";

type ResumeBatchRequest = (
	BatchToken,
	Arc<PrismaClient>,
	SyncManager,
	oneshot::Sender<Result<chan::Receiver<LabelerOutput>, ImageLabelerError>>,
);

type UpdateModelRequest = (
	Box<dyn Model>,
	oneshot::Sender<Result<(), ImageLabelerError>>,
);

pub(super) struct Batch {
	pub(super) token: BatchToken,
	pub(super) location_id: location::id::Type,
	pub(super) location_path: PathBuf,
	pub(super) device_id: device::id::Type,
	pub(super) file_paths: Vec<file_path_for_media_processor::Data>,
	pub(super) output_tx: chan::Sender<LabelerOutput>,
	pub(super) is_resumable: bool,
	pub(super) db: Arc<PrismaClient>,
	pub(super) sync: SyncManager,
}

#[derive(Serialize, Deserialize, Debug)]
struct ResumableBatch {
	location_id: location::id::Type,
	location_path: PathBuf,
	device_id: device::id::Type,
	file_paths: Vec<file_path_for_media_processor::Data>,
}

pub struct OldImageLabeler {
	to_resume_batches_file_path: PathBuf,
	new_batches_tx: chan::Sender<Batch>,
	resume_batch_tx: chan::Sender<ResumeBatchRequest>,
	update_model_tx: chan::Sender<UpdateModelRequest>,
	shutdown_tx: chan::Sender<oneshot::Sender<()>>,
	to_resume_batches: Arc<RwLock<HashMap<BatchToken, ResumableBatch>>>,
	handle: RefCell<Option<JoinHandle<()>>>,
}

impl OldImageLabeler {
	pub async fn new(
		model: Box<dyn Model>,
		data_directory: impl AsRef<Path>,
	) -> Result<Self, ImageLabelerError> {
		let to_resume_batches_file_path = data_directory.as_ref().join(PENDING_BATCHES_FILE);

		let model_and_session = Arc::new(RwLock::new(
			ModelAndSession::new(model, data_directory.as_ref().join("models")).await?,
		));

		let to_resume_batches = Arc::new(RwLock::new(
			match fs::read(&to_resume_batches_file_path).await {
				Ok(bytes) => {
					let pending_batches =
						rmp_serde::from_slice::<HashMap<BatchToken, ResumableBatch>>(&bytes)?;
					info!(
						"Image labeler had {} pending batches to be resumed",
						pending_batches.len()
					);

					if let Err(e) = fs::remove_file(&to_resume_batches_file_path).await {
						error!(
							"{:#?}",
							ImageLabelerError::from(FileIOError::from((
								&to_resume_batches_file_path,
								e,
								"Failed to remove to resume batches file",
							)))
						);
					}

					pending_batches
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					// If the file doesn't exist, we just start with an empty list
					HashMap::new()
				}
				Err(e) => {
					return Err(ImageLabelerError::FileIO(FileIOError::from((
						&to_resume_batches_file_path,
						e,
						"Failed to read to resume batches file",
					))))
				}
			},
		));

		let (new_batches_tx, new_batches_rx) = chan::unbounded();
		let (resume_batch_tx, resume_batch_rx) = chan::bounded(4);
		let (update_model_tx, update_model_rx) = chan::bounded(1);
		let (shutdown_tx, shutdown_rx) = chan::bounded(1);

		let batch_supervisor_handle = tokio::spawn({
			let to_resume_batches = Arc::clone(&to_resume_batches);
			async move {
				loop {
					let handle = tokio::spawn(actor_loop(
						Arc::clone(&model_and_session),
						new_batches_rx.clone(),
						resume_batch_rx.clone(),
						update_model_rx.clone(),
						shutdown_rx.clone(),
						Arc::clone(&to_resume_batches),
					));

					if let Err(e) = handle.await {
						error!("Batch processor panicked: {e:#?}; restarting...");
					} else {
						// process_batches exited normally, so we can exit as well
						break;
					}
				}
			}
		});

		Ok(Self {
			to_resume_batches_file_path,
			new_batches_tx,
			resume_batch_tx,
			update_model_tx,
			shutdown_tx,
			to_resume_batches,
			handle: RefCell::new(Some(batch_supervisor_handle)),
		})
	}

	#[allow(clippy::too_many_arguments)]
	async fn new_batch_inner(
		&self,
		location_id: location::id::Type,
		location_path: PathBuf,
		device_id: device::id::Type,
		file_paths: Vec<file_path_for_media_processor::Data>,
		db: Arc<PrismaClient>,
		sync: SyncManager,
		is_resumable: bool,
	) -> (BatchToken, chan::Receiver<LabelerOutput>) {
		let (tx, rx) = chan::bounded(usize::max(file_paths.len(), 1));
		let token = Uuid::new_v4();
		if !file_paths.is_empty() {
			if self
				.new_batches_tx
				.send(Batch {
					token,
					location_id,
					location_path,
					device_id,
					file_paths,
					output_tx: tx,
					is_resumable,
					db,
					sync,
				})
				.await
				.is_err()
			{
				error!("Failed to send batch to image labeller");
			}
		} else {
			// If there are no files to process, we close the channel immediately so the receiver
			// side will never wait for a message
			tx.close();
		}

		(token, rx)
	}

	pub async fn new_batch(
		&self,
		location_id: location::id::Type,
		device_id: device::id::Type,
		location_path: PathBuf,
		file_paths: Vec<file_path_for_media_processor::Data>,
		db: Arc<PrismaClient>,
		sync: SyncManager,
	) -> chan::Receiver<LabelerOutput> {
		self.new_batch_inner(
			location_id,
			location_path,
			device_id,
			file_paths,
			db,
			sync,
			false,
		)
		.await
		.1
	}

	/// Resumable batches have lower priority than normal batches
	pub async fn new_resumable_batch(
		&self,
		location_id: location::id::Type,
		location_path: PathBuf,
		device_id: device::id::Type,
		file_paths: Vec<file_path_for_media_processor::Data>,
		db: Arc<PrismaClient>,
		sync: SyncManager,
	) -> (BatchToken, chan::Receiver<LabelerOutput>) {
		self.new_batch_inner(
			location_id,
			location_path,
			device_id,
			file_paths,
			db,
			sync,
			true,
		)
		.await
	}

	pub async fn change_model(&self, model: Box<dyn Model>) -> Result<(), ImageLabelerError> {
		let (tx, rx) = oneshot::channel();

		if self.update_model_tx.send((model, tx)).await.is_err() {
			error!("Failed to send model update to image labeller");
		}

		rx.await
			.expect("model update result channel unexpectedly closed")
	}

	pub async fn shutdown(&self) {
		debug!("Shutting down image labeller");

		let (tx, rx) = oneshot::channel();

		self.new_batches_tx.close();
		self.resume_batch_tx.close();
		self.update_model_tx.close();

		if self.shutdown_tx.send(tx).await.is_err() {
			error!("Failed to send stop signal to image labeller model executor");
		}

		self.shutdown_tx.close();

		rx.await
			.expect("critical error: image labeller shutdown result channel unexpectedly closed");

		if let Some(handle) = self
			.handle
			.try_borrow_mut()
			.ok()
			.and_then(|mut maybe_handle| maybe_handle.take())
		{
			if let Err(e) = handle.await {
				error!("Failed to join image labeller supervisors: {e:#?}");
			}
		}

		let to_resume_batches = self.to_resume_batches.read().await;

		if !to_resume_batches.is_empty() {
			if let Ok(pending_batches) = rmp_serde::to_vec_named(to_resume_batches.deref())
				.map_err(|e| error!("{:#?}", ImageLabelerError::from(e)))
			{
				if let Err(e) = fs::write(&self.to_resume_batches_file_path, &pending_batches).await
				{
					error!(
						"{:#?}",
						ImageLabelerError::from(FileIOError::from((
							&self.to_resume_batches_file_path,
							e,
							"Failed to write to resume batches file",
						)))
					);
				}
			}
		}
	}

	pub async fn resume_batch(
		&self,
		token: BatchToken,
		db: Arc<PrismaClient>,
		sync: SyncManager,
	) -> Result<chan::Receiver<LabelerOutput>, ImageLabelerError> {
		let (tx, rx) = oneshot::channel();

		self.resume_batch_tx
			.send((token, db, sync, tx))
			.await
			.expect("critical error: image labeler communication channel unexpectedly closed");

		rx.await
			.expect("critical error: image labeler resume batch result channel unexpectedly closed")
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl Sync for OldImageLabeler {}

async fn actor_loop(
	model_and_session: Arc<RwLock<ModelAndSession>>,
	new_batches_rx: chan::Receiver<Batch>,
	resume_batch_rx: chan::Receiver<ResumeBatchRequest>,
	update_model_rx: chan::Receiver<UpdateModelRequest>,
	shutdown_rx: chan::Receiver<oneshot::Sender<()>>,
	to_resume_batches: Arc<RwLock<HashMap<BatchToken, ResumableBatch>>>,
) {
	let (done_tx, done_rx) = chan::bounded(1);
	let (stop_tx, stop_rx) = chan::bounded(1);

	let new_batches_rx_for_shutdown = new_batches_rx.clone();

	// // TODO: Make this configurable!
	// let available_parallelism = std::thread::available_parallelism().map_or_else(
	// 	|e| {
	// 		error!("Failed to get available parallelism: {e:#?}");
	// 		1
	// 	},
	// 	// Using 25% of available parallelism
	// 	|non_zero| usize::max(non_zero.get() / 4, 1),
	// );

	let available_parallelism = 1;

	info!(
		"Image labeler available parallelism: {} cores",
		available_parallelism
	);

	enum StreamMessage {
		NewBatch(Batch),
		ResumeBatch(
			BatchToken,
			Arc<PrismaClient>,
			SyncManager,
			oneshot::Sender<Result<chan::Receiver<LabelerOutput>, ImageLabelerError>>,
		),
		UpdateModel(
			Box<dyn Model>,
			oneshot::Sender<Result<(), ImageLabelerError>>,
		),
		BatchDone(FinishStatus),
		Shutdown(oneshot::Sender<()>),
	}

	let mut queue = VecDeque::with_capacity(16);

	let mut currently_processing = None;

	let mut msg_stream = pin!((
		new_batches_rx.map(StreamMessage::NewBatch),
		resume_batch_rx
			.map(|(token, db, sync, done_tx)| StreamMessage::ResumeBatch(token, db, sync, done_tx)),
		update_model_rx.map(|(model, done_tx)| StreamMessage::UpdateModel(model, done_tx)),
		done_rx.clone().map(StreamMessage::BatchDone),
		shutdown_rx.map(StreamMessage::Shutdown)
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			StreamMessage::NewBatch(batch @ Batch { is_resumable, .. }) => {
				if currently_processing.is_none() {
					currently_processing = Some(spawn(spawned_processing(
						Arc::clone(&model_and_session),
						batch,
						available_parallelism,
						stop_rx.clone(),
						done_tx.clone(),
					)));
				} else if !is_resumable {
					// TODO: Maybe we should cancel the current batch and start this one instead?
					queue.push_front(batch)
				} else {
					queue.push_back(batch)
				}
			}

			StreamMessage::ResumeBatch(token, db, sync, resume_done_tx) => {
				let resume_result = if let Some((batch, output_rx)) =
					to_resume_batches.write().await.remove(&token).map(
						|ResumableBatch {
						     location_id,
						     device_id,
						     location_path,
						     file_paths,
						 }| {
							let (output_tx, output_rx) =
								chan::bounded(usize::max(file_paths.len(), 1));
							(
								Batch {
									token,
									db,
									sync,
									device_id,
									output_tx,
									location_id,
									location_path,
									file_paths,
									is_resumable: true,
								},
								output_rx,
							)
						},
					) {
					if currently_processing.is_none() {
						currently_processing = Some(spawn(spawned_processing(
							Arc::clone(&model_and_session),
							batch,
							available_parallelism,
							stop_rx.clone(),
							done_tx.clone(),
						)));
					} else {
						queue.push_back(batch)
					}

					Ok(output_rx)
				} else {
					Err(ImageLabelerError::TokenNotFound(token))
				};

				if resume_done_tx.send(resume_result).is_err() {
					error!("Failed to send batch resume result from image labeller");
				}
			}

			StreamMessage::UpdateModel(new_model, update_done_tx) => {
				if currently_processing.is_some() {
					let (tx, rx) = oneshot::channel();

					stop_tx.send(tx).await.expect("stop_tx unexpectedly closed");

					if timeout(ONE_SEC, rx).await.is_err() {
						error!("Failed to stop image labeller batch processor");
						if stop_rx.is_full() {
							stop_rx.recv().await.ok();
						}
					}
				}

				if update_done_tx
					.send(
						model_and_session
							.write()
							.await
							.update_model(new_model)
							.await,
					)
					.is_err()
				{
					error!("Failed to send model update result from image labeller");
				}
			}

			StreamMessage::BatchDone(FinishStatus::Interrupted(batch)) => {
				if currently_processing.is_none() {
					currently_processing = Some(spawn(spawned_processing(
						Arc::clone(&model_and_session),
						batch,
						1,
						stop_rx.clone(),
						done_tx.clone(),
					)));
				} else {
					queue.push_front(batch);
				}
			}

			StreamMessage::BatchDone(FinishStatus::Done(token, output_tx)) => {
				debug!("Batch <token='{token}'> done");

				if let Some(handle) = currently_processing.take() {
					if let Err(e) = handle.await {
						error!("Failed to join image labeller batch processor: {e:#?}");
					}
				}

				output_tx.close(); // So our listener can exit

				if let Some(next_batch) = queue.pop_front() {
					currently_processing = Some(spawn(spawned_processing(
						Arc::clone(&model_and_session),
						next_batch,
						4,
						stop_rx.clone(),
						done_tx.clone(),
					)));
				}
			}

			StreamMessage::Shutdown(shutdown_done_tx) => {
				debug!("Shutting down image labeller batch processor");

				if let Some(handle) = currently_processing.take() {
					let (tx, rx) = oneshot::channel();

					stop_tx.send(tx).await.expect("stop_tx unexpectedly closed");

					if timeout(ONE_SEC * 5, rx).await.is_err() {
						error!("Failed to stop image labeller batch processor");
						if stop_rx.is_full() {
							stop_rx.recv().await.ok();
						}
					}

					if let Err(e) = handle.await {
						error!("Failed to join image labeller batch processor: {e:#?}");
					}

					if let Ok(FinishStatus::Interrupted(batch)) = done_rx.recv().await {
						queue.push_front(batch);
					}
				}

				let pending_batches = new_batches_rx_for_shutdown
					.filter_map(
						|Batch {
						     token,
						     location_id,
						     location_path,
						     device_id,
						     file_paths,
						     is_resumable,
						     ..
						 }| async move {
							is_resumable.then_some((
								token,
								ResumableBatch {
									location_id,
									location_path,
									device_id,
									file_paths,
								},
							))
						},
					)
					.collect::<Vec<_>>()
					.await;

				to_resume_batches.write().await.extend(
					queue
						.into_iter()
						.filter_map(
							|Batch {
							     token,
							     location_id,
							     location_path,
							     device_id,
							     file_paths,
							     is_resumable,
							     ..
							 }| {
								is_resumable.then_some((
									token,
									ResumableBatch {
										location_id,
										location_path,
										device_id,
										file_paths,
									},
								))
							},
						)
						.chain(pending_batches.into_iter()),
				);

				shutdown_done_tx
					.send(())
					.expect("shutdown_done_tx unexpectedly closed");

				break;
			}
		}
	}
}
