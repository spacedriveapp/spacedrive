//! AI operations - machine learning models, inference, prompts
//!
//! This handles ML models (face_detection, llm, ocr, embeddings).
//! NOT to be confused with data models (Person, Album) - those are in models.rs
//!
//! Inference goes to the host as `ai.infer`, which checks the manifest's
//! `use_models` grant and hands the input to the provider the core has for
//! the model's category. A core built without inference providers, or with
//! no model of that category downloaded, answers [`Error::NotAvailable`],
//! which a job handles rather than fails on.
//!
//! The `_batch` methods send many inputs in one host call, such as a
//! folder of photos or a shot's keyframes, and answer each input on its own,
//! so one unreadable file fails only its own entry.

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

	fn infer<O: DeserializeOwned>(&self, task: &str, input: &[u8]) -> Result<O> {
		crate::ffi::op_framed(
			"ai.infer",
			&serde_json::json!({ "model": self.model_id, "task": task }),
			input,
		)
	}

	fn infer_batch<O: DeserializeOwned>(
		&self,
		task: &str,
		inputs: &[&[u8]],
	) -> Result<Vec<Result<O>>> {
		let lengths: Vec<usize> = inputs.iter().map(|input| input.len()).collect();
		let items: Vec<BatchItem<O>> = crate::ffi::op_framed(
			"ai.infer",
			&serde_json::json!({ "model": self.model_id, "task": task, "items": lengths }),
			&inputs.concat(),
		)?;
		Ok(items
			.into_iter()
			.map(|item| match item {
				BatchItem::Ok(output) => Ok(output),
				BatchItem::Error(error) => Err(error.into_error("ai.infer")),
			})
			.collect())
	}

	/// Detect faces in image. Boxes are fractions of the upright image.
	/// Each face carries an embedding when the manifest also grants
	/// `face_embedding` and the core has a face embedding model.
	pub async fn detect_faces(&self, image_data: &[u8]) -> Result<Vec<FaceDetection>> {
		self.infer("detect_faces", image_data)
	}

	/// [`Self::detect_faces`] over many images in one host call.
	pub async fn detect_faces_batch(
		&self,
		images: &[&[u8]],
	) -> Result<Vec<Result<Vec<FaceDetection>>>> {
		self.infer_batch("detect_faces", images)
	}

	/// Embed a face crop, through a `face_embedding` handle.
	pub async fn embed_face(&self, face_crop: &[u8]) -> Result<Vec<f32>> {
		self.infer("embed_face", face_crop)
	}

	/// Classify scene in image
	pub async fn classify(&self, image_data: &[u8]) -> Result<Vec<SceneTag>> {
		self.infer("classify", image_data)
	}

	/// [`Self::classify`] over many images in one host call.
	pub async fn classify_batch(&self, images: &[&[u8]]) -> Result<Vec<Result<Vec<SceneTag>>>> {
		self.infer_batch("classify", images)
	}

	/// Describe an image in a sentence or two, through an `image_caption`
	/// handle.
	pub async fn caption(&self, image_data: &[u8]) -> Result<String> {
		self.infer("caption", image_data)
	}

	/// [`Self::caption`] over many images in one host call.
	pub async fn caption_batch(&self, images: &[&[u8]]) -> Result<Vec<Result<String>>> {
		self.infer_batch("caption", images)
	}

	/// OCR document
	pub async fn ocr_document(&self, record: &Record) -> Result<String> {
		Err(Error::Unsupported("ocr_document".into()))
	}

	/// Generate text embedding
	pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
		self.infer("embed_text", text.as_bytes())
	}

	/// [`Self::embed_text`] over many texts in one host call.
	pub async fn embed_text_batch(&self, texts: &[&str]) -> Result<Vec<Result<Vec<f32>>>> {
		let inputs: Vec<&[u8]> = texts.iter().map(|text| text.as_bytes()).collect();
		self.infer_batch("embed_text", &inputs)
	}

	/// Generate text from a prompt, through a language model of this
	/// handle's category.
	pub async fn generate_text(&self, prompt: &str) -> Result<String> {
		self.infer("generate_text", prompt.as_bytes())
	}
}

/// One input's answer in a batch.
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum BatchItem<O> {
	Ok(O),
	Error(crate::ffi::HostError),
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
