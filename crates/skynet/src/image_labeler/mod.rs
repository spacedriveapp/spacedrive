use sd_prisma::prisma::file_path;
use sd_utils::{db::MissingFieldError, error::FileIOError};

use std::{collections::HashSet, path::Path};

use thiserror::Error;
use tracing::error;

mod actor;
mod model;

pub use actor::ImageLabeler;
pub use model::{Model, YoloV8};

#[derive(Debug)]
pub struct LabelerOutput {
	pub file_path_id: file_path::id::Type,
	pub labels_result: Result<HashSet<String>, ImageLabelerError>,
}

#[derive(Debug, Error)]
pub enum ImageLabelerError {
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
	#[error("model file not found: {}", .0.display())]
	ModelFileNotFound(Box<Path>),
	#[error("no model available for inference")]
	NoModelAvailable,

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}
