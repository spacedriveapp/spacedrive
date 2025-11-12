//! AI/ML Model management system
//!
//! Downloads and manages models for:
//! - Whisper (speech-to-text)
//! - Tesseract (OCR language data)
//! - Future: CLIP, Stable Diffusion, etc.

pub mod action;
pub mod download;
pub mod ensure;
pub mod query;
pub mod types;
pub mod whisper;

pub use action::{DeleteWhisperModelAction, DownloadWhisperModelAction};
pub use download::ModelDownloadJob;
pub use ensure::ensure_whisper_model;
pub use query::ListWhisperModelsQuery;
pub use types::{ModelInfo, ModelProvider, ModelType};
pub use whisper::{WhisperModel, WhisperModelManager};

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Get the models directory for a given data directory
pub fn get_models_dir(data_dir: &Path) -> PathBuf {
	data_dir.join("models")
}

/// Get the whisper models directory
pub fn get_whisper_models_dir(data_dir: &Path) -> PathBuf {
	get_models_dir(data_dir).join("whisper")
}

/// Get the tesseract data directory
pub fn get_tesseract_data_dir(data_dir: &Path) -> PathBuf {
	get_models_dir(data_dir).join("tesseract")
}

/// Ensure models directory exists
pub async fn ensure_models_dir(data_dir: &Path) -> Result<PathBuf> {
	let models_dir = get_models_dir(data_dir);
	tokio::fs::create_dir_all(&models_dir).await?;
	Ok(models_dir)
}
