//! Whisper model management

use super::types::{ModelInfo, ModelProvider, ModelType};
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperModel {
	Tiny,
	Base,
	Small,
	Medium,
	Large,
}

impl WhisperModel {
	pub fn filename(&self) -> &'static str {
		match self {
			Self::Tiny => "ggml-tiny.bin",
			Self::Base => "ggml-base.bin",
			Self::Small => "ggml-small.bin",
			Self::Medium => "ggml-medium.bin",
			Self::Large => "ggml-large-v3.bin",
		}
	}

	pub fn id(&self) -> &'static str {
		match self {
			Self::Tiny => "whisper-tiny",
			Self::Base => "whisper-base",
			Self::Small => "whisper-small",
			Self::Medium => "whisper-medium",
			Self::Large => "whisper-large",
		}
	}

	pub fn display_name(&self) -> &'static str {
		match self {
			Self::Tiny => "Whisper Tiny",
			Self::Base => "Whisper Base",
			Self::Small => "Whisper Small",
			Self::Medium => "Whisper Medium",
			Self::Large => "Whisper Large",
		}
	}

	pub fn download_url(&self) -> &'static str {
		match self {
			Self::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
			Self::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
			Self::Small => {
				"https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
			}
			Self::Medium => {
				"https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
			}
			Self::Large => {
				"https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
			}
		}
	}

	pub fn size_bytes(&self) -> u64 {
		match self {
			Self::Tiny => 75 * 1024 * 1024,     // 75 MB
			Self::Base => 142 * 1024 * 1024,    // 142 MB
			Self::Small => 466 * 1024 * 1024,   // 466 MB
			Self::Medium => 1500 * 1024 * 1024, // 1.5 GB
			Self::Large => 3100 * 1024 * 1024,  // 3.1 GB
		}
	}

	pub fn description(&self) -> &'static str {
		match self {
			Self::Tiny => "Fastest, lowest accuracy (75 MB)",
			Self::Base => "Good balance of speed and accuracy (142 MB)",
			Self::Small => "Better accuracy, slower (466 MB)",
			Self::Medium => "High accuracy, much slower (1.5 GB)",
			Self::Large => "Best accuracy, very slow (3.1 GB)",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s.to_lowercase().as_str() {
			"tiny" => Some(Self::Tiny),
			"base" => Some(Self::Base),
			"small" => Some(Self::Small),
			"medium" => Some(Self::Medium),
			"large" => Some(Self::Large),
			_ => None,
		}
	}

	pub fn all() -> Vec<Self> {
		vec![
			Self::Tiny,
			Self::Base,
			Self::Small,
			Self::Medium,
			Self::Large,
		]
	}
}

pub struct WhisperModelManager {
	models_dir: PathBuf,
}

impl WhisperModelManager {
	pub fn new(data_dir: &Path) -> Self {
		Self {
			models_dir: super::get_whisper_models_dir(data_dir),
		}
	}

	/// Get path for a model
	pub fn get_model_path(&self, model: &WhisperModel) -> PathBuf {
		self.models_dir.join(model.filename())
	}

	/// Check if a model is downloaded
	pub async fn is_downloaded(&self, model: &WhisperModel) -> bool {
		let path = self.get_model_path(model);
		if !path.exists() {
			return false;
		}

		// Verify size is reasonable (within 10% of expected)
		if let Ok(metadata) = tokio::fs::metadata(&path).await {
			let actual_size = metadata.len();
			let expected_size = model.size_bytes();
			let size_diff = if actual_size > expected_size {
				actual_size - expected_size
			} else {
				expected_size - actual_size
			};
			let tolerance = expected_size / 10; // 10% tolerance

			size_diff < tolerance
		} else {
			false
		}
	}

	/// List all available models with download status
	pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
		let mut models = Vec::new();

		for model in WhisperModel::all() {
			let downloaded = self.is_downloaded(&model).await;

			models.push(ModelInfo {
				id: model.id().to_string(),
				name: model.display_name().to_string(),
				model_type: ModelType::Whisper,
				size_bytes: model.size_bytes(),
				provider: ModelProvider::HuggingFace {
					repo: "ggerganov/whisper.cpp".to_string(),
				},
				filename: model.filename().to_string(),
				downloaded,
				description: Some(model.description().to_string()),
			});
		}

		Ok(models)
	}

	/// Delete a model
	pub async fn delete_model(&self, model: &WhisperModel) -> Result<()> {
		let path = self.get_model_path(model);
		if path.exists() {
			tokio::fs::remove_file(&path).await?;
		}
		Ok(())
	}

	/// Get total size of all downloaded models
	pub async fn total_downloaded_size(&self) -> u64 {
		let mut total = 0u64;

		for model in WhisperModel::all() {
			if self.is_downloaded(&model).await {
				total += model.size_bytes();
			}
		}

		total
	}
}
