//! The providers each category resolves to, and the batched entry point.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use image::RgbImage;
use tokio::sync::Mutex;

use super::catalog::{self, ModelSpec};
use super::{cloud, image as img, local, AiError, Category, Face, Label, Locality, Preference};
use crate::library::Library;

/// The long edge images are decoded to before detection. Faces need more
/// pixels than scenes because the embedder aligns from the decoded image.
const FACE_DECODE_SIDE: u32 = 2048;
const SCENE_DECODE_SIDE: u32 = 512;
const SCENE_TOP_K: usize = 5;

pub trait FaceDetector: Send + Sync {
	fn detect(&self, image: &RgbImage) -> Result<Vec<Face>, AiError>;
}

pub trait FaceEmbedder: Send + Sync {
	/// Embeds the face `face` marks in `image`, aligned on its landmarks.
	fn embed(&self, image: &RgbImage, face: &Face) -> Result<Vec<f32>, AiError>;
	/// Embeds an image that is already a face crop.
	fn embed_crop(&self, image: &RgbImage) -> Result<Vec<f32>, AiError>;
}

pub trait SceneClassifier: Send + Sync {
	fn classify(&self, images: &[RgbImage], top_k: usize) -> Result<Vec<Vec<Label>>, AiError>;
}

pub trait TextEmbedder: Send + Sync {
	fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, AiError>;
}

#[async_trait]
pub trait Captioner: Send + Sync {
	async fn caption(&self, image: &[u8]) -> Result<String, AiError>;
}

#[derive(Clone)]
pub enum Provider {
	FaceDetection(Arc<dyn FaceDetector>),
	FaceEmbedding(Arc<dyn FaceEmbedder>),
	SceneClassification(Arc<dyn SceneClassifier>),
	ImageCaption(Arc<dyn Captioner>, Locality),
	TextEmbedding(Arc<dyn TextEmbedder>),
}

/// Every model this device can run, loaded on first use and kept for the
/// daemon's lifetime.
pub struct ProviderRegistry {
	models_dir: PathBuf,
	loaded: Mutex<HashMap<&'static str, Provider>>,
}

impl ProviderRegistry {
	pub fn new(data_dir: &Path) -> Self {
		Self {
			models_dir: crate::ops::models::get_models_dir(data_dir).join("providers"),
			loaded: Mutex::new(HashMap::new()),
		}
	}

	pub fn model_dir(&self, spec: &ModelSpec) -> PathBuf {
		self.models_dir.join(spec.id)
	}

	/// Every file is on disk at its catalog size. The hash was checked when
	/// the download finished, so it is not recomputed on each lookup.
	pub async fn is_installed(&self, spec: &ModelSpec) -> bool {
		let dir = self.model_dir(spec);
		for file in spec.files {
			match tokio::fs::metadata(dir.join(file.name)).await {
				Ok(meta) if meta.len() == file.size => {}
				_ => return false,
			}
		}
		true
	}

	/// Drops a loaded model so its files can be deleted.
	pub async fn unload(&self, spec: &ModelSpec) {
		self.loaded.lock().await.remove(spec.id);
	}

	/// The first installed model of `category`, loading it if needed.
	pub async fn local(&self, category: Category) -> Result<Option<Provider>, AiError> {
		for spec in catalog::for_category(category) {
			if !self.is_installed(spec).await {
				continue;
			}
			let mut loaded = self.loaded.lock().await;
			if let Some(provider) = loaded.get(spec.id) {
				return Ok(Some(provider.clone()));
			}
			let dir = self.model_dir(spec);
			let provider = tokio::task::spawn_blocking(move || local::load(spec, &dir))
				.await
				.map_err(|e| AiError::Model(e.to_string()))??;
			tracing::info!(model = spec.id, "Loaded inference model");
			loaded.insert(spec.id, provider.clone());
			return Ok(Some(provider));
		}
		Ok(None)
	}

	/// The provider a request runs on: a local model when one is installed,
	/// otherwise a cloud captioner when the extension and the library both
	/// allow it.
	pub async fn resolve(
		&self,
		category: Category,
		preference: Preference,
		library: &Library,
	) -> Result<Provider, AiError> {
		if let Some(provider) = self.local(category).await? {
			return Ok(provider);
		}
		if category == Category::ImageCaption && preference == Preference::ApiAllowed {
			if let Some(captioner) = cloud::captioner(library).await? {
				return Ok(Provider::ImageCaption(Arc::new(captioner), Locality::Cloud));
			}
		}
		Err(AiError::NotInstalled(category.as_str()))
	}

	/// Runs `task` over every item and answers each on its own.
	///
	/// `embedder` fills the embedding of each detected face when the caller
	/// holds a `face_embedding` grant and a model is installed.
	pub async fn run(
		&self,
		provider: Provider,
		task: &str,
		items: Vec<Vec<u8>>,
		embedder: Option<Arc<dyn FaceEmbedder>>,
	) -> Result<Vec<Result<serde_json::Value, AiError>>, AiError> {
		let expected = match &provider {
			Provider::FaceDetection(_) => "detect_faces",
			Provider::FaceEmbedding(_) => "embed_face",
			Provider::SceneClassification(_) => "classify",
			Provider::ImageCaption(..) => "caption",
			Provider::TextEmbedding(_) => "embed_text",
		};
		if task != expected {
			return Err(AiError::Input(format!(
				"unknown task {task}; this category answers {expected}"
			)));
		}
		if let Provider::ImageCaption(captioner, _) = provider {
			let mut out = Vec::with_capacity(items.len());
			for item in &items {
				out.push(captioner.caption(item).await.and_then(to_json));
			}
			return Ok(out);
		}
		tokio::task::spawn_blocking(move || run_local(provider, items, embedder))
			.await
			.map_err(|e| AiError::Model(e.to_string()))
	}
}

fn run_local(
	provider: Provider,
	items: Vec<Vec<u8>>,
	embedder: Option<Arc<dyn FaceEmbedder>>,
) -> Vec<Result<serde_json::Value, AiError>> {
	match provider {
		Provider::FaceDetection(detector) => items
			.iter()
			.map(|bytes| {
				let image = img::decode(bytes, FACE_DECODE_SIDE)?;
				let mut faces = detector.detect(&image)?;
				if let Some(embedder) = &embedder {
					for face in &mut faces {
						face.embedding = embedder.embed(&image, face)?;
					}
				}
				to_json(faces)
			})
			.collect(),
		Provider::FaceEmbedding(embedder) => items
			.iter()
			.map(|bytes| to_json(embedder.embed_crop(&img::decode(bytes, 512)?)?))
			.collect(),
		Provider::SceneClassification(classifier) => {
			let decoded: Vec<_> = items
				.iter()
				.map(|bytes| img::decode(bytes, SCENE_DECODE_SIDE))
				.collect();
			let images: Vec<RgbImage> = decoded
				.iter()
				.filter_map(|r| r.as_ref().ok().cloned())
				.collect();
			let mut labels = match classifier.classify(&images, SCENE_TOP_K) {
				Ok(labels) => labels.into_iter(),
				Err(e) => {
					let message = e.to_string();
					return items
						.iter()
						.map(|_| Err(AiError::Model(message.clone())))
						.collect();
				}
			};
			decoded
				.into_iter()
				.map(|r| r.and_then(|_| to_json(labels.next().unwrap_or_default())))
				.collect()
		}
		Provider::TextEmbedding(embedder) => {
			let texts: Vec<Result<&str, AiError>> = items
				.iter()
				.map(|bytes| {
					std::str::from_utf8(bytes)
						.map_err(|_| AiError::Input("text is not UTF-8".into()))
				})
				.collect();
			let valid: Vec<&str> = texts
				.iter()
				.filter_map(|t| t.as_ref().ok().copied())
				.collect();
			let mut vectors = match embedder.embed(&valid) {
				Ok(vectors) => vectors.into_iter(),
				Err(e) => {
					let message = e.to_string();
					return items
						.iter()
						.map(|_| Err(AiError::Model(message.clone())))
						.collect();
				}
			};
			texts
				.into_iter()
				.map(|t| t.and_then(|_| to_json(vectors.next().unwrap_or_default())))
				.collect()
		}
		Provider::ImageCaption(..) => items
			.iter()
			.map(|_| Err(AiError::Model("captioners run on the async side".into())))
			.collect(),
	}
}

fn to_json<T: serde::Serialize>(value: T) -> Result<serde_json::Value, AiError> {
	serde_json::to_value(value).map_err(|e| AiError::Model(e.to_string()))
}
