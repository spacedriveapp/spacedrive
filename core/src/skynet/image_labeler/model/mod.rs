use std::sync::Arc;
use std::{collections::HashSet, path::Path};

use async_channel as chan;
use crossbeam::channel;
use image::ImageFormat;

use ort::{Session, SessionBuilder, SessionInputs, SessionOutputs};
use sd_prisma::prisma::file_path;
use tokio::sync::oneshot;
use tracing::{debug, error, info};

use super::{actor::BatchToken, ImageLabelerError};

mod yolov8;

pub use yolov8::YoloV8;

pub(super) enum ModelExecutorInput {
	ToProcess {
		batch_token: BatchToken,
		file_path_id: file_path::id::Type,
		image: Vec<u8>,
		format: ImageFormat,
	},
	UpdateModel(
		Arc<dyn Model>,
		oneshot::Sender<Result<(), ImageLabelerError>>,
	),
	Stop,
}
pub(super) type ModelOutput = (
	BatchToken,
	file_path::id::Type,
	Result<HashSet<String>, ImageLabelerError>,
);

pub trait Model: Send + Sync + 'static {
	fn name(&self) -> &'static str {
		std::any::type_name::<Self>()
	}

	fn path(&self) -> &Path;

	fn prepare_input<'image>(
		&self,
		image: &'image [u8],
		format: ImageFormat,
	) -> Result<SessionInputs<'image>, ImageLabelerError>;
	fn process_output(
		&self,
		output: SessionOutputs<'_>,
	) -> Result<HashSet<String>, ImageLabelerError>;
}

pub(super) fn model_executor(
	mut maybe_model: Option<Arc<dyn Model>>,
	input_rx: channel::Receiver<ModelExecutorInput>,
	results_tx: chan::Sender<ModelOutput>,
) {
	info!("Starting image labeler model executor");
	let mut maybe_session = if let Some(model) = maybe_model.as_deref() {
		load_model(model)
			.map(|session| {
				info!("Loaded model: {}", model.name());
				session
			})
			.map_err(|e| error!("Failed to load model: {e:#?}"))
			.ok()
	} else {
		None
	};

	while let Ok(msg) = input_rx.recv() {
		match msg {
			ModelExecutorInput::ToProcess {
				batch_token,
				file_path_id,
				image,
				format,
			} => {
				// This will never block as the channel is unbounded
				if results_tx
					.send_blocking((
						batch_token,
						file_path_id,
						if let (Some(session), Some(model)) =
							(&maybe_session, maybe_model.as_deref())
						{
							process_single_image(image, format, session, model)
						} else {
							error!("Tried to process image without a loaded model");
							Err(ImageLabelerError::NoModelAvailable)
						},
					))
					.is_err()
				{
					error!("Failed to send model output, <batch_token='{batch_token}', file_path_id='{file_path_id}'>");
					break;
				}
			}
			ModelExecutorInput::UpdateModel(new_model, res_tx) => {
				match load_model(new_model.as_ref()) {
					Ok(session) => {
						info!(
							"Changing models: {} -> {}",
							maybe_model.map(|model| model.name()).unwrap_or("None"),
							new_model.name()
						);
						if res_tx.send(Ok(())).is_err() {
							error!("Failed to send model update ok result");
						}
						maybe_model = Some(new_model);
						maybe_session = Some(session);
					}
					Err(e) => {
						if res_tx.send(Err(e)).is_err() {
							error!("Failed to send model update error result");
						}
						maybe_model = None;
						maybe_session = None;
					}
				}
				info!("Image labeler model updated");
			}
			ModelExecutorInput::Stop => {
				debug!("Stopping image labeler model executor");
				break;
			}
		}
	}
}

fn load_model(model: &dyn Model) -> Result<Session, ImageLabelerError> {
	SessionBuilder::new()?
		.with_parallel_execution(true)?
		.with_memory_pattern(true)?
		.with_model_from_file(model.path())
		.map_err(Into::into)
}

fn process_single_image(
	image: Vec<u8>,
	format: ImageFormat,
	session: &Session,
	model: &dyn Model,
) -> Result<HashSet<String>, ImageLabelerError> {
	let input = model.prepare_input(&image, format)?;
	let output = session.run(input)?;

	model.process_output(output)
}
