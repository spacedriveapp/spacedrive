use crate::{
	location::file_path_helper::{file_path_for_media_processor, IsolatedFilePathData},
	util::{db::MissingFieldError, error::FileIOError},
};

use sd_prisma::prisma::{file_path, location};

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	pin::pin,
	thread,
};

use async_channel as chan;
use crossbeam::channel;
use futures_concurrency::{future::Join, stream::Merge};
use image::{imageops::FilterType, GenericImageView, ImageFormat};
use ndarray::{s, Array, Axis};
use ort::{inputs, SessionBuilder, SessionInputs, SessionOutputs};
use thiserror::Error;
use tokio::{
	fs,
	sync::broadcast,
	task::{block_in_place, JoinHandle},
};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tracing::{debug, error};

type BatchToken = u64;
type ModelInput = (BatchToken, file_path::id::Type, Vec<u8>, ImageFormat);
type ModelOutput = (BatchToken, file_path::id::Type, Vec<String>);

type BatchOutput = (file_path::id::Type, Result<Vec<String>, ImageLabellerError>);

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

#[derive(Debug, Error)]
pub enum ImageLabellerError {
	#[error("model executor failed: {0}")]
	ModelExecutorFailed(#[from] ort::Error),
	#[error("image load failed: {0}")]
	ImageLoadFailed(#[from] image::ImageError),
	#[error("failed to get isolated file path data: {0}")]
	IsolateFilePathData(#[from] MissingFieldError),
	#[error("file_path with unsupported extension: <id='{0}', extension='{1}'>")]
	UnsupportedExtension(file_path::id::Type, String),
	#[error("file_path too big: <id='{0}', size='{1}'>")]
	FileTooBig(file_path::id::Type, usize),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

struct Batch {
	location_id: location::id::Type,
	location_path: PathBuf,
	file_paths: Vec<file_path_for_media_processor::Data>,
	output: chan::Sender<BatchOutput>,
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Model {
	RecognizeAnything,
	YoloV8,
}

impl Model {
	pub fn prepare_input(
		&self,
		image: &[u8],
		format: ImageFormat,
	) -> Result<impl Into<SessionInputs<'_>>, ImageLabellerError> {
		match *self {
			Model::RecognizeAnything => todo!(),
			Model::YoloV8 => {
				let original_img = image::load_from_memory_with_format(image, format)?;
				let img = original_img.resize_exact(640, 640, FilterType::CatmullRom);
				let mut input = Array::zeros((1, 3, 640, 640));
				for pixel in img.pixels() {
					let x = pixel.0 as _;
					let y = pixel.1 as _;
					let [r, g, b, _] = pixel.2 .0;
					input[[0, 0, y, x]] = (r as f32) / 255.;
					input[[0, 1, y, x]] = (g as f32) / 255.;
					input[[0, 2, y, x]] = (b as f32) / 255.;
				}

				Ok(inputs!["images" => input.view()]?)
			}
		}
	}

	pub fn process_output(
		&self,
		output: SessionOutputs<'_>,
	) -> Result<Vec<String>, ImageLabellerError> {
		match *self {
			Model::RecognizeAnything => todo!(),
			Model::YoloV8 => {
				#[rustfmt::skip]
				const YOLOV8_CLASS_LABELS: [&str; 80] = [
					"person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck",
					"boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench",
					"bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra",
					"giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee",
					"skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove",
					"skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup",
					"fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange",
					"broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "couch",
					"potted plant", "bed", "dining table", "toilet", "tv", "laptop", "mouse",
					"remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
					"refrigerator", "book", "clock", "vase", "scissors", "teddy bear",
					"hair drier", "toothbrush"
				];

				let output0 = &output["output0"];

				let output_tensor = output0.extract_tensor::<f32>()?;

				let output_view = output_tensor.view();

				let output_tensor_transposed = output_view.t();

				let output = output_tensor_transposed.slice(s![.., .., 0]);

				Ok(output
					.axis_iter(Axis(0))
					.map(|row| {
						row.iter()
							// skip bounding box coordinates
							.skip(4)
							.enumerate()
							.map(|(class_id, probability)| (class_id, *probability))
							.reduce(|accum, row| if row.1 > accum.1 { row } else { accum })
							.expect("not empty output")
					})
					.filter(|(_, probability)| *probability > 0.8)
					.map(|(class_id, _)| YOLOV8_CLASS_LABELS[class_id])
					.collect::<HashSet<_>>()
					.into_iter()
					.map(ToString::to_string)
					.collect())
			}
		}
	}
}

pub struct ImageLabeller {
	batches_tx: chan::Sender<Batch>,
	handles: [JoinHandle<()>; 2],
}

impl ImageLabeller {
	pub fn new(model_path: PathBuf, model: Model) -> Self {
		let (images_tx, images_rx) = channel::unbounded();
		let (results_tx, results_rx) = chan::unbounded();

		let (batches_tx, batches_rx) = chan::unbounded();

		let (cancel_tx, mut cancel_rx) = broadcast::channel(1);

		let model_supervisor_handle = tokio::spawn(async move {
			loop {
				thread::scope(|s| {
					let handle = s.spawn(|| {
						model_executor(&model_path, model, images_rx.clone(), results_tx.clone())
					});

					if let Err(e) = block_in_place(|| handle.join()) {
						error!("Model executor failed: {e:#?}; restarting...");
					}
				});

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
		});

		let batch_supervisor_handle = tokio::spawn({
			let mut cancel_rx = cancel_tx.subscribe();
			async move {
				loop {
					let handle = tokio::spawn(process_batches(
						images_tx.clone(),
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

		Self {
			batches_tx,
			handles: [model_supervisor_handle, batch_supervisor_handle],
		}
	}

	pub async fn new_batch(
		&self,
		location_id: location::id::Type,
		location_path: PathBuf,
		file_paths: Vec<file_path_for_media_processor::Data>,
	) -> chan::Receiver<BatchOutput> {
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
}

impl Drop for ImageLabeller {
	fn drop(&mut self) {
		debug!("Shutting down image labeller");
		self.handles.iter().for_each(JoinHandle::abort);
		self.batches_tx.close();
	}
}

async fn process_batches(
	images_tx: channel::Sender<ModelInput>,
	batches_rx: chan::Receiver<Batch>,
	results_rx: chan::Receiver<ModelOutput>,
	cancel_rx: broadcast::Receiver<()>,
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
		BroadcastStream::new(cancel_rx).map(|_| StreamMessage::Shutdown)
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
								if output.send_blocking((file_path_id, Err(e.into()))).is_err() {
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
								.send_blocking((
									file_path_id,
									Err(ImageLabellerError::UnsupportedExtension(
										file_path_id,
										iso_file_path.extension().to_owned(),
									)),
								))
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
								ImageLabellerError::FileTooBig(
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
							if output.send_blocking((file_path_id, Err(e))).is_err() {
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
					.for_each(|(file_path_id, bytes, format)| {
						images_tx
							.send((current_batch_token, file_path_id, bytes, format))
							.expect("images_tx unexpectedly closed");
					});
			}

			StreamMessage::Results((current_batch_token, file_path_id, labels)) => {
				if let Some((pending, output)) = pending_batches.get_mut(&current_batch_token) {
					*pending -= 1;

					if output.send_blocking((file_path_id, Ok(labels))).is_err() {
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

fn model_executor(
	model_path: &Path,
	model: Model,
	images_rx: channel::Receiver<ModelInput>,
	results_tx: chan::Sender<ModelOutput>,
) -> Result<(), ImageLabellerError> {
	debug!("Starting model executor");
	let session_builder = SessionBuilder::new()?;
	// .with_parallel_execution(true)?
	// .with_memory_pattern(true)?
	debug!(
		"session builder creates: model path: {}",
		model_path.display()
	);
	let session = session_builder.with_model_from_file(model_path)?;
	debug!("Model executor started");

	while let Ok((batch_token, file_path_id, image, format)) = images_rx.recv() {
		let input = model.prepare_input(&image, format)?;
		let output = session.run(input)?;

		let labels = model.process_output(output)?;

		// This will never block as the channel is unbounded
		if results_tx
			.send_blocking((batch_token, file_path_id, labels))
			.is_err()
		{
			error!("Failed to send model output, <batch_token='{batch_token}', file_path_id='{file_path_id}'>");
			break;
		}
	}

	Ok(())
}
