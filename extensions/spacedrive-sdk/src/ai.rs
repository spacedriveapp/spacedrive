//! AI operations
//!
//! OCR, text classification, and other AI-powered analysis.

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::Arc;

use crate::ffi::WireClient;
use crate::types::Result;

/// AI client for intelligent operations
pub struct AiClient {
	client: Arc<RefCell<WireClient>>,
}

impl AiClient {
	pub(crate) fn new(client: Arc<RefCell<WireClient>>) -> Self {
		Self { client }
	}

	/// Extract text from image or PDF using OCR
	pub fn ocr(&self, data: &[u8], options: OcrOptions) -> Result<OcrResult> {
		self.client.borrow().call(
			"query:ai.ocr.v1",
			&OcrInput {
				data: BASE64_STANDARD.encode(data),
				options,
			},
		)
	}

	/// Classify or extract information from text using AI
	pub fn classify_text(&self, text: &str, prompt: &str) -> Result<serde_json::Value> {
		self.client.borrow().call(
			"query:ai.classify_text.v1",
			&ClassifyTextInput {
				text: text.to_string(),
				prompt: prompt.to_string(),
				options: ClassifyOptions::default(),
			},
		)
	}

	/// Generate semantic embedding for text
	pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
		let result: EmbedOutput = self.client.borrow().call(
			"query:ai.embed.v1",
			&EmbedInput {
				text: text.to_string(),
			},
		)?;
		Ok(result.embedding)
	}
}

// === Input/Output Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrOptions {
	#[serde(default = "default_language")]
	pub language: String,

	#[serde(default)]
	pub engine: OcrEngine,

	#[serde(default = "default_true")]
	pub preprocessing: bool,
}

fn default_language() -> String {
	"eng".to_string()
}

fn default_true() -> bool {
	true
}

impl Default for OcrOptions {
	fn default() -> Self {
		Self {
			language: "eng".to_string(),
			engine: OcrEngine::Tesseract,
			preprocessing: true,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OcrEngine {
	Tesseract,
	EasyOcr,
}

impl Default for OcrEngine {
	fn default() -> Self {
		OcrEngine::Tesseract
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct OcrInput {
	data: String, // base64-encoded
	options: OcrOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
	pub text: String,
	pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyOptions {
	#[serde(default = "default_model")]
	pub model: String,

	#[serde(default = "default_temperature")]
	pub temperature: f32,

	#[serde(default = "default_max_tokens")]
	pub max_tokens: u32,
}

fn default_model() -> String {
	"user_default".to_string()
}

fn default_temperature() -> f32 {
	0.1
}

fn default_max_tokens() -> u32 {
	1000
}

impl Default for ClassifyOptions {
	fn default() -> Self {
		Self {
			model: "user_default".to_string(),
			temperature: 0.1,
			max_tokens: 1000,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct ClassifyTextInput {
	text: String,
	prompt: String,
	options: ClassifyOptions,
}

#[derive(Debug, Serialize, Deserialize)]
struct EmbedInput {
	text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EmbedOutput {
	embedding: Vec<f32>,
}
