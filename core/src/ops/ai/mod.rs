//! # Inference providers
//!
//! `core::ops::ai` answers the inference an extension asks for through
//! `ai.infer`. Extensions are WASM and cannot run a model themselves, so the
//! core owns every model: it downloads them, loads them on first use and runs
//! them on this device.
//!
//! Each request names a category (`face_detection`, `face_embedding`,
//! `scene_classification`, `image_caption`, `text_embedding`). The registry
//! answers a category with the first model of that category in the catalog
//! whose files are on disk. A model is never downloaded implicitly: the user
//! downloads it through `ai.models.download`, which shows its size and license
//! first and refuses until the license is accepted.
//!
//! Inference is local unless a library opts in. Image captioning may use a
//! cloud service with the user's own key, but only when the library's
//! `ai.cloud_caption.enabled` setting is on and the extension's manifest
//! preference allows an API. A manifest that says `local` never reaches the
//! cloud, whatever the library allows.
//!
//! Requests are batched: one call carries any number of inputs and answers
//! each with its own result, so a bad file fails its own item and not the
//! batch.

pub mod catalog;
pub mod cloud;
pub mod image;
pub mod local;
pub mod ops;
pub mod registry;

use serde::{Deserialize, Serialize};
use specta::Type;

pub use registry::{Provider, ProviderRegistry};

/// What a provider does. The string form is the prefix an extension puts on
/// a model id (`face_detection:photos_v1`) and the manifest's grant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Category {
	FaceDetection,
	FaceEmbedding,
	SceneClassification,
	ImageCaption,
	TextEmbedding,
}

impl Category {
	pub const ALL: [Category; 5] = [
		Category::FaceDetection,
		Category::FaceEmbedding,
		Category::SceneClassification,
		Category::ImageCaption,
		Category::TextEmbedding,
	];

	pub fn as_str(self) -> &'static str {
		match self {
			Category::FaceDetection => "face_detection",
			Category::FaceEmbedding => "face_embedding",
			Category::SceneClassification => "scene_classification",
			Category::ImageCaption => "image_caption",
			Category::TextEmbedding => "text_embedding",
		}
	}

	pub fn parse(s: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|c| c.as_str() == s)
	}
}

/// Where an extension allows its inference to run, from the manifest's
/// `use_models[].preference`. A missing preference means local, so an
/// extension has to ask before anything can leave the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
	Local,
	ApiAllowed,
}

impl Preference {
	pub fn from_manifest(preference: Option<&str>) -> Self {
		match preference {
			Some("api" | "api_allowed" | "cloud") => Preference::ApiAllowed,
			_ => Preference::Local,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
	Local,
	Cloud,
}

/// A box in fractions of the oriented image, so it maps onto any rendition
/// of the photo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

/// A detected face as the SDK's `FaceDetection` deserializes it.
/// `embedding` is empty unless the extension also holds a `face_embedding`
/// grant and a face embedding model is installed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
	pub bbox: BoundingBox,
	pub confidence: f32,
	/// Eye, nose and mouth points, in the same fractions as `bbox`.
	pub landmarks: Vec<[f32; 2]>,
	pub embedding: Vec<f32>,
	pub identified_as: Option<uuid::Uuid>,
}

/// A scene label as the SDK's `SceneTag` deserializes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
	pub label: String,
	pub confidence: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
	#[error("no {0} provider is installed")]
	NotInstalled(&'static str),
	#[error("{0}")]
	Input(String),
	#[error("model failed: {0}")]
	Model(String),
	#[error("cloud provider failed: {0}")]
	Cloud(String),
	#[error("interrupted")]
	Interrupted,
}

impl From<::ort::Error> for AiError {
	fn from(e: ::ort::Error) -> Self {
		AiError::Model(e.to_string())
	}
}
