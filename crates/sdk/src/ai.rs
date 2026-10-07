//! AI operations - machine learning models, inference, prompts
//!
//! This handles ML models (face_detection, llm, ocr, embeddings).
//! NOT to be confused with data models (Person, Album) - those are in models.rs
//!
//! Stubs for type-checking. Implementation will call WASM host functions.

use crate::types::*;
use serde::{de::DeserializeOwned, Serialize};

/// AI context for machine learning operations
pub struct AiContext;

impl AiContext {
	/// Load a registered AI model by ID (category:name)
	pub fn from_registered(&self, model_id: &str) -> ModelHandle {
		ModelHandle {
			model_id: model_id.to_string(),
		}
	}

	/// Use AI model with preference (local, API, etc.)
	pub fn with_model(&self, preference: &str) -> ModelHandle {
		ModelHandle {
			model_id: preference.to_string(),
		}
	}
}

/// AI Model registration context (for extension install)
pub struct AiModelRegistry;

impl AiModelRegistry {
	/// Register an AI model on extension install
	pub async fn register(
		&self,
		category: &str,
		name: &str,
		source: AiModelSource,
	) -> Result<AiModelId> {
		Err(Error::Unsupported("register".into()))
	}

	/// Check if AI model is registered
	pub fn is_registered(&self, model_id: &str) -> bool {
		panic!("WASM host call")
	}
}

/// AI Model source (bundled, download, or Ollama)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AiModelSource {
	Bundled(Vec<u8>),
	Download { url: String, sha256: String },
	Ollama(String),
}

/// AI Model identifier (category:name)
#[derive(Clone, Debug)]
pub struct AiModelId {
	pub category: String,
	pub name: String,
}

impl AiModelId {
	pub fn new(category: impl Into<String>, name: impl Into<String>) -> Self {
		Self {
			category: category.into(),
			name: name.into(),
		}
	}
}

impl std::fmt::Display for AiModelId {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}:{}", self.category, self.name)
	}
}

/// Handle to a loaded AI model
pub struct ModelHandle {
	model_id: String,
}

impl ModelHandle {
	/// Use Jinja template for prompting
	pub fn prompt_template(self, template_name: &str) -> PromptBuilder {
		PromptBuilder {
			model_id: self.model_id,
			template: template_name.to_string(),
		}
	}

	/// Detect faces in image
	pub async fn detect_faces(&self, image_data: &[u8]) -> Result<Vec<FaceDetection>> {
		Err(Error::Unsupported("detect_faces".into()))
	}

	/// Classify scene in image
	pub async fn classify(&self, image_data: &[u8]) -> Result<Vec<SceneTag>> {
		Err(Error::Unsupported("classify".into()))
	}

	/// OCR document
	pub async fn ocr_document(&self, record: &Record) -> Result<String> {
		Err(Error::Unsupported("ocr_document".into()))
	}

	/// Generate text embedding
	pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
		Err(Error::Unsupported("embed_text".into()))
	}
}

/// Prompt builder with Jinja templates
pub struct PromptBuilder {
	model_id: String,
	template: String,
}

impl PromptBuilder {
	/// Render template with context
	pub fn render_with<T: Serialize>(self, context: &T) -> Result<RenderedPrompt> {
		Err(Error::Unsupported("render_with".into()))
	}
}

/// Rendered prompt ready for inference
pub struct RenderedPrompt {
	model_id: String,
	prompt: String,
}

impl RenderedPrompt {
	/// Generate text from rendered prompt
	pub async fn generate_text(self) -> Result<String> {
		Err(Error::Unsupported("generate_text".into()))
	}

	/// Generate JSON from rendered prompt
	pub async fn generate_json<T: DeserializeOwned>(self) -> Result<T> {
		Err(Error::Unsupported("generate_json".into()))
	}
}

// Types for AI operations
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FaceDetection {
	pub bbox: BoundingBox,
	pub confidence: f32,
	pub embedding: Vec<f32>,
	pub identified_as: Option<uuid::Uuid>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BoundingBox {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SceneTag {
	pub label: String,
	pub confidence: f32,
}
