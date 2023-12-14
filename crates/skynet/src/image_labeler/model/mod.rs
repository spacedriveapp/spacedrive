use sd_utils::error::FileIOError;

use std::{collections::HashSet, path::Path};

use image::ImageFormat;
use ort::{Session, SessionBuilder, SessionInputs, SessionOutputs};
use tokio::{fs, io};
use tracing::{error, info, trace};

use super::ImageLabelerError;

mod yolov8;

pub use yolov8::YoloV8;

pub trait Model: Send + Sync + 'static {
	fn name(&self) -> &'static str {
		std::any::type_name::<Self>()
	}

	fn path(&self) -> &Path;

	fn prepare_input<'image>(
		&self,
		image_path: &Path,
		image: &'image [u8],
		format: ImageFormat,
	) -> Result<SessionInputs<'image>, ImageLabelerError>;

	fn process_output(
		&self,
		output: SessionOutputs<'_>,
	) -> Result<HashSet<String>, ImageLabelerError>;
}

pub(super) struct ModelAndSession {
	maybe_model: Option<Box<dyn Model>>,
	maybe_session: Option<Session>,
}

impl ModelAndSession {
	pub async fn new(model: Box<dyn Model>) -> Self {
		let maybe_session = check_model_file(model.path())
			.await
			.map_err(|e| error!("Failed to check model file before passing to Ort: {e:#?}"))
			.ok()
			.and_then(|()| {
				load_model(model.as_ref())
					.map(|session| {
						info!("Loaded model: {}", model.name());
						trace!("{session:#?}");
						session
					})
					.map_err(|e| error!("Failed to load model: {e:#?}"))
					.ok()
			});

		Self {
			maybe_model: maybe_session.is_some().then_some(model),
			maybe_session,
		}
	}

	pub fn can_process(&self) -> bool {
		self.maybe_session.is_some() && self.maybe_model.is_some()
	}

	pub async fn update_model(
		&mut self,
		new_model: Box<dyn Model>,
	) -> Result<(), ImageLabelerError> {
		info!("Attempting to change image labeler models...");

		check_model_file(new_model.path()).await.and_then(|()| {
			load_model(new_model.as_ref())
				.map(|session| {
					info!(
						"Changing models: {} -> {}",
						self.maybe_model
							.as_ref()
							.map(|old_model| old_model.name())
							.unwrap_or("None"),
						new_model.name()
					);

					self.maybe_model = Some(new_model);
					self.maybe_session = Some(session);
				})
				.map_err(|e| {
					self.maybe_model = None;
					self.maybe_session = None;

					e
				})
		})
	}

	pub fn process_single_image(
		&self,
		image_path: &Path,
		image: Vec<u8>,
		format: ImageFormat,
	) -> Result<HashSet<String>, ImageLabelerError> {
		if let (Some(session), Some(model)) = (&self.maybe_session, self.maybe_model.as_deref()) {
			let inputs = model.prepare_input(image_path, &image, format)?;
			let outputs = session.run(inputs)?;
			model.process_output(outputs)
		} else {
			error!("Tried to process image without a loaded model");
			Err(ImageLabelerError::NoModelAvailable)
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

async fn check_model_file(model_path: impl AsRef<Path>) -> Result<(), ImageLabelerError> {
	let model_path = model_path.as_ref();

	match fs::metadata(model_path).await {
		Ok(_) => Ok(()),
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			error!(
				"Model file not found: '{}'. Image labeler will be disabled!",
				model_path.display()
			);
			Ok(())
		}
		Err(e) => Err(ImageLabelerError::FileIO(FileIOError::from((
			model_path,
			e,
			"Failed to get metadata for model file",
		)))),
	}
}
