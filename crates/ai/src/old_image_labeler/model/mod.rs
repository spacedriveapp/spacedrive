use sd_utils::error::FileIOError;

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};

use futures::prelude::stream::StreamExt;
use image::ImageFormat;
use ort::{Session, SessionBuilder, SessionInputs, SessionOutputs};
use thiserror::Error;
use tokio::{
	fs,
	io::{self, AsyncWriteExt},
};
use tracing::{error, info, trace};
use url::Url;

use super::ImageLabelerError;

mod yolov8;

pub use yolov8::YoloV8;
pub use yolov8::DEFAULT_MODEL_VERSION;

pub enum ModelSource {
	Url(Url),
	Path(PathBuf),
}

pub trait Model: Send + Sync + 'static {
	fn name(&self) -> &'static str;

	fn origin(&self) -> &ModelSource;

	fn version(&self) -> &str;

	fn versions() -> Vec<&'static str>
	where
		Self: Sized;

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
	model_data_dir: PathBuf,
}

impl ModelAndSession {
	pub async fn new(
		model: Box<dyn Model>,
		data_dir: impl AsRef<Path>,
	) -> Result<Self, DownloadModelError> {
		let data_dir = data_dir.as_ref().join(model.name());
		let model_path = download_model(model.origin(), &data_dir).await?;

		info!(
			"Loading mode: {} from {}",
			model.name(),
			model_path.display()
		);

		let maybe_session = check_model_file(&model_path)
			.await
			.map_err(|e| error!("Failed to check model file before passing to Ort: {e:#?}"))
			.ok()
			.and_then(|()| {
				load_model(&model_path)
					.map(|session| {
						info!("Loaded model: {}", model.name());
						trace!("{session:#?}");
						session
					})
					.map_err(|e| error!("Failed to load model: {e:#?}"))
					.ok()
			});

		Ok(Self {
			maybe_model: maybe_session.is_some().then_some(model),
			maybe_session,
			model_data_dir: data_dir,
		})
	}

	pub fn can_process(&self) -> bool {
		self.maybe_session.is_some() && self.maybe_model.is_some()
	}

	pub async fn update_model(
		&mut self,
		new_model: Box<dyn Model>,
	) -> Result<(), ImageLabelerError> {
		info!("Attempting to change image labeler models...");

		let model_path = download_model(new_model.origin(), &self.model_data_dir).await?;

		info!(
			"Change mode: {} to {}",
			new_model.name(),
			model_path.display()
		);

		check_model_file(&model_path).await.and_then(|()| {
			load_model(&model_path)
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
				.inspect_err(|e| {
					error!("Failed to load new model: {e:#?}");
					self.maybe_model = None;
					self.maybe_session = None;
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

#[derive(Error, Debug)]
pub enum DownloadModelError {
	#[error("Failed to download due to request error: {0}")]
	RequestError(#[from] reqwest::Error),
	#[error("Failed to download due to status code: {0}")]
	HttpStatusError(reqwest::StatusCode),
	#[error("Invalid file name for url: {0}")]
	InvalidUrlFileName(Url),
	#[error("Unknown model version to download: {0}")]
	UnknownModelVersion(String),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

fn load_model(model_path: impl AsRef<Path>) -> Result<Session, ImageLabelerError> {
	SessionBuilder::new()?
		.with_parallel_execution(true)?
		.with_memory_pattern(true)?
		.with_model_from_file(model_path)
		.map_err(Into::into)
}

async fn download_model(
	model_origin: &ModelSource,
	data_dir: impl AsRef<Path>,
) -> Result<PathBuf, DownloadModelError> {
	let data_dir = data_dir.as_ref();

	match model_origin {
		ModelSource::Url(url) => {
			let Some(file_name) = url.path_segments().and_then(|segments| segments.last()) else {
				return Err(DownloadModelError::InvalidUrlFileName(url.to_owned()));
			};

			fs::create_dir_all(data_dir)
				.await
				.map_err(|e| FileIOError::from((data_dir, e, "Failed to create data directory")))?;

			let file_path = data_dir.join(file_name);
			match fs::metadata(&file_path).await {
				Ok(_) => return Ok(file_path),
				Err(e) if e.kind() != io::ErrorKind::NotFound => {
					return Err(DownloadModelError::FileIO(FileIOError::from((
						file_path,
						e,
						"Failed to get metadata for model file",
					))))
				}
				_ => {
					info!("Downloading model from: {} to {}", url, file_path.display());
					let response = reqwest::get(url.as_str()).await?;
					// Ensure the request was successful (status code 2xx)
					if !response.status().is_success() {
						return Err(DownloadModelError::HttpStatusError(response.status()));
					}

					// Create or open a file at the specified path
					let mut file = fs::File::create(&file_path).await.map_err(|e| {
						FileIOError::from((
							&file_path,
							e,
							"Failed to create the model file on disk",
						))
					})?;
					// Stream the response body to the file
					let mut body = response.bytes_stream();
					while let Some(chunk) = body.next().await {
						let chunk = chunk?;
						file.write_all(&chunk).await.map_err(|e| {
							FileIOError::from((
								&file_path,
								e,
								"Failed to write chunk of data to the model file on disk",
							))
						})?;
					}
				}
			}

			Ok(file_path)
		}
		ModelSource::Path(file_path) => Ok(file_path.to_owned()),
	}
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
