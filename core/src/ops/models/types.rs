//! Model type definitions

use serde::{Deserialize, Serialize};
use specta::Type;

/// Type of model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ModelType {
	/// Whisper speech-to-text model
	Whisper,
	/// Tesseract OCR language data
	Tesseract,
}

/// Model provider
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum ModelProvider {
	/// Hugging Face
	HuggingFace { repo: String },
	/// GitHub Release
	GitHub { owner: String, repo: String },
	/// Direct URL
	Direct { url: String },
}

/// Information about a model
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelInfo {
	/// Unique model identifier
	pub id: String,
	/// Human-readable name
	pub name: String,
	/// Model type
	pub model_type: ModelType,
	/// File size in bytes
	pub size_bytes: u64,
	/// Where to download from
	pub provider: ModelProvider,
	/// Filename on disk
	pub filename: String,
	/// Whether this model is currently downloaded
	pub downloaded: bool,
	/// Optional description
	pub description: Option<String>,
}
