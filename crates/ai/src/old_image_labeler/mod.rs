use sd_prisma::prisma::file_path;
use sd_utils::{db::MissingFieldError, error::FileIOError};

use std::path::Path;

use thiserror::Error;
use tracing::error;
use uuid::Uuid;

mod model;
mod old_actor;
mod process;

pub use model::{DownloadModelError, Model, YoloV8, DEFAULT_MODEL_VERSION};
pub use old_actor::OldImageLabeler;

pub type BatchToken = Uuid;

#[derive(Debug)]
pub struct LabelerOutput {
	pub file_path_id: file_path::id::Type,
	pub has_new_labels: bool,
	pub result: Result<(), ImageLabelerError>,
}

#[derive(Debug, Error)]
pub enum ImageLabelerError {
	#[error("model executor failed: {0}")]
	ModelExecutorFailed(#[from] ort::Error),
	#[error("image load failed <path='{}'>: {0}", .1.display())]
	ImageLoadFailed(image::ImageError, Box<Path>),
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
	#[error("failed to decode pending batches: {0}")]
	Decode(#[from] rmp_serde::decode::Error),
	#[error("failed to encode pending batches: {0}")]
	Encode(#[from] rmp_serde::encode::Error),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("resume token not found: {0}")]
	TokenNotFound(BatchToken),
	#[error(transparent)]
	DownloadModel(#[from] DownloadModelError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),
}
