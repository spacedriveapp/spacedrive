use crate::{
	location::file_path_helper::{file_path_for_media_processor, IsolatedFilePathData},
	skynet::image_labeler::{ImageLabelerError, LabelerOutput},
	util::error::FileIOError,
};

use std::{cell::RefCell, collections::HashMap, path::PathBuf, pin::pin, sync::Arc, thread};

use async_channel as chan;
use crossbeam::channel;
use futures::stream::once;
use futures_concurrency::{future::Join, stream::Merge};
use image::ImageFormat;
use sd_prisma::prisma::location;
use tokio::{
	fs,
	io::ErrorKind,
	sync::{broadcast, oneshot},
	task::{block_in_place, JoinHandle},
};
use tokio_stream::StreamExt;
use tracing::{debug, error};

use super::model::{model_executor, Model, ModelExecutorInput, ModelOutput};

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

pub(super) type BatchToken = u64;

struct Batch {
	location_id: location::id::Type,
	location_path: PathBuf,
	file_paths: Vec<file_path_for_media_processor::Data>,
	output: chan::Sender<LabelerOutput>,
}

pub struct ImageLabeler {
	model_executor_input_tx: channel::Sender<ModelExecutorInput>,
	batches_tx: chan::Sender<Batch>,
	handles: [RefCell<Option<JoinHandle<()>>>; 2],
	cancel_tx: broadcast::Sender<()>,
}

impl ImageLabeler {
	pub async fn new(model: Arc<dyn Model>) -> Result<Self, ImageLabelerError> {
		let (model_executor_input_tx, model_executor_input_rx) = channel::unbounded();
		let (results_tx, results_rx) = chan::unbounded();

		let (batches_tx, batches_rx) = chan::unbounded();

		let (cancel_tx, mut cancel_rx) = broadcast::channel(1);

		let maybe_model = check_model_file(model).await?;

		let model_supervisor_handle = tokio::spawn({
			let model_executor_input_tx = model_executor_input_tx.clone();

			async move {
				loop {
					thread::scope(|s| {
						let handle = s.spawn(|| {
							model_executor(
								maybe_model.clone(),
								model_executor_input_rx.clone(),
								results_tx.clone(),
							);
						});

						if let Err(e) = block_in_place(|| handle.join()) {
							error!("Model executor panicked {e:#?}; restarting...");
						}
					});

					let cancel_res = cancel_rx.try_recv();

					if matches!(
						cancel_res,
						Ok(())
							| Err(broadcast::error::TryRecvError::Closed
								| broadcast::error::TryRecvError::Lagged(_))
					) {
						// If we sucessfully receive a cancellation signal or if the channel is closed or lagged,
						// we break the loop
						debug!("Model supervisor stopping");
						model_executor_input_tx.send(ModelExecutorInput::Stop).ok();

						break;
					}
				}
			}
		});

		let batch_supervisor_handle = tokio::spawn({
			let mut cancel_rx = cancel_tx.subscribe();
			let model_executor_input_tx = model_executor_input_tx.clone();
			async move {
				loop {
					let handle = tokio::spawn(process_batches(
						model_executor_input_tx.clone(),
						batches_rx.clone(),
						results_rx.clone(),
						cancel_rx.resubscribe(),
					));

					if let Err(e) = handle.await {
						error!("Batch supervisor failed: {e:#?}; restarting...");
					}

					if matches!(
						cancel_rx.try_recv(),
						Ok(())
							| Err(broadcast::error::TryRecvError::Closed
								| broadcast::error::TryRecvError::Lagged(_))
					) {
						// If we sucessfully receive a cancellation signal or if the channel is closed or lagged,
						// we break the loop
						break;
					}
				}
			}
		});

		Ok(Self {
			model_executor_input_tx,
			batches_tx,
			handles: [
				RefCell::new(Some(model_supervisor_handle)),
				RefCell::new(Some(batch_supervisor_handle)),
			],
			cancel_tx,
		})
	}

	pub async fn new_batch(
		&self,
		location_id: location::id::Type,
		location_path: PathBuf,
		file_paths: Vec<file_path_for_media_processor::Data>,
	) -> chan::Receiver<LabelerOutput> {
		let (tx, rx) = chan::bounded(file_paths.len());

		if self
			.batches_tx
			.send(Batch {
				location_id,
				location_path,
				file_paths,
				output: tx,
			})
			.await
			.is_err()
		{
			error!("Failed to send batch to image labeller");
		}

		rx
	}

	pub async fn change_model(&self, model: Arc<dyn Model>) -> Result<(), ImageLabelerError> {
		let model_path = model.path();

		match fs::metadata(model_path).await {
			Err(e) if e.kind() == ErrorKind::NotFound => {
				return Err(ImageLabelerError::ModelFileNotFound(model_path.into()));
			}
			Err(e) => {
				return Err(ImageLabelerError::FileIO(FileIOError::from((
					model_path,
					e,
					"Failed to get metadata for model file",
				))))
			}
			_ => {}
		}

		let (tx, rx) = oneshot::channel();

		if self
			.model_executor_input_tx
			.send(ModelExecutorInput::UpdateModel(model, tx))
			.is_err()
		{
			error!("Failed to send model update to image labeller");
		}

		rx.await
			.expect("model update result channel unexpectedly closed")
	}

	pub async fn shutdown(&self) {
		debug!("Shutting down image labeller");
		self.batches_tx.close();
		self.cancel_tx.send(()).ok();
		for handle in self
			.handles
			.iter()
			.filter_map(|ref_cell| ref_cell.try_borrow_mut().ok().and_then(|mut op| op.take()))
		{
			handle.abort();
			if let Err(e) = handle.await {
				error!("Failed to join image labeller supervisors: {e:#?}");
			}
		}
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl Sync for ImageLabeler {}

async fn process_batches(
	model_executor_input_tx: channel::Sender<ModelExecutorInput>,
	batches_rx: chan::Receiver<Batch>,
	results_rx: chan::Receiver<ModelOutput>,
	mut cancel_rx: broadcast::Receiver<()>,
) {
	let mut batch_token = 0u64;

	let mut pending_batches = HashMap::with_capacity(16);

	enum StreamMessage {
		Batch(Batch),
		Results(ModelOutput),
		Shutdown,
	}

	let mut msg_stream = pin!((
		batches_rx.map(StreamMessage::Batch),
		results_rx.map(StreamMessage::Results),
		once(cancel_rx.recv()).map(|_| StreamMessage::Shutdown)
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			StreamMessage::Batch(Batch {
				location_id,
				location_path,
				file_paths,
				output,
			}) => {
				let to_infere = file_paths
					.into_iter()
					.filter_map(|file_path| {
						let file_path_id = file_path.id;
						IsolatedFilePathData::try_from((location_id, file_path))
							.map(|iso_file_path| (file_path_id, iso_file_path))
							.map_err(|e| {
								if output
									.send_blocking(LabelerOutput {
										file_path_id,
										labels_result: Err(e.into()),
									})
									.is_err()
								{
									error!(
										"Failed to send batch output with iso_file_path error, \
									<file_path_id='{file_path_id}'>"
									);
								}
							})
							.ok()
					})
					.filter_map(|(file_path_id, iso_file_path)| {
						if let Some(format) = ImageFormat::from_extension(iso_file_path.extension())
						{
							Some((file_path_id, location_path.join(&iso_file_path), format))
						} else {
							if output
								.send_blocking(LabelerOutput {
									file_path_id,
									labels_result: Err(ImageLabelerError::UnsupportedExtension(
										file_path_id,
										iso_file_path.extension().to_owned(),
									)),
								})
								.is_err()
							{
								error!("Failed to send batch output with unsupported extension error, \
								<file_path_id='{file_path_id}'>");
							}

							None
						}
					})
					.map(|(file_path_id, path, format)| async move {
						let metadata = fs::metadata(&path).await.map_err(|e| {
							(
								file_path_id,
								FileIOError::from((
									&path,
									e,
									"Failed to get metadata for file to get labels",
								))
								.into(),
							)
						})?;

						if metadata.len() > MAX_FILE_SIZE {
							return Err((
								file_path_id,
								ImageLabelerError::FileTooBig(
									file_path_id,
									metadata.len() as usize,
								),
							));
						}

						let bytes = fs::read(&path).await.map_err(|e| {
							(
								file_path_id,
								FileIOError::from((&path, e, "Failed to read file to get labels"))
									.into(),
							)
						})?;

						Ok((file_path_id, bytes, format))
					})
					.collect::<Vec<_>>()
					.join()
					.await
					.into_iter()
					.filter_map(|res| match res {
						Ok(ok) => Some(ok),
						Err((file_path_id, e)) => {
							if output
								.send_blocking(LabelerOutput {
									file_path_id,
									labels_result: Err(e),
								})
								.is_err()
							{
								error!("Failed to send batch output with I/O errors, <file_path_id='{file_path_id}'>");
							}

							None
						}
					})
					.collect::<Vec<_>>();

				let current_batch_token = batch_token;
				batch_token = batch_token.wrapping_add(1);
				pending_batches.insert(current_batch_token, (to_infere.len(), output));

				to_infere
					.into_iter()
					.for_each(|(file_path_id, image, format)| {
						model_executor_input_tx
							.send(ModelExecutorInput::ToProcess {
								batch_token: current_batch_token,
								file_path_id,
								image,
								format,
							})
							.expect("images_tx unexpectedly closed");
					});
			}

			StreamMessage::Results((current_batch_token, file_path_id, labels_result)) => {
				if let Some((pending, output)) = pending_batches.get_mut(&current_batch_token) {
					*pending -= 1;

					if output
						.send(LabelerOutput {
							file_path_id,
							labels_result,
						})
						.await
						.is_err()
					{
						error!("Failed to send batch output with labels, <file_path_id='{file_path_id}'>");
					}

					if *pending == 0 {
						pending_batches.remove(&current_batch_token);
					}
				}
			}

			StreamMessage::Shutdown => {
				debug!("Shutting down image labeller batch processor");
				break;
			}
		}
	}
}

async fn check_model_file(
	model: Arc<dyn Model>,
) -> Result<Option<Arc<dyn Model>>, ImageLabelerError> {
	let model_path = model.path();

	match fs::metadata(model_path).await {
		Ok(_) => Ok(Some(model)),
		Err(e) if e.kind() == ErrorKind::NotFound => {
			error!(
				"Model file not found: '{}'. Image labeler will be disabled!",
				model_path.display()
			);
			Ok(None)
		}
		Err(e) => Err(ImageLabelerError::FileIO(FileIOError::from((
			model_path,
			e,
			"Failed to get metadata for model file",
		)))),
	}
}
