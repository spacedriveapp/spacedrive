//! The providers each category resolves to, and the batched entry point.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use image::RgbImage;
use tokio::sync::Mutex;

use super::catalog::ModelSpec;
#[cfg(feature = "ai-local")]
use super::{catalog, local};
use super::{cloud, image as img, AiError, Category, Face, Label, Locality, Preference};
use crate::library::Library;

/// The long edge images are decoded to before detection. Faces need more
/// pixels than scenes because the embedder aligns from the decoded image.
const FACE_DECODE_SIDE: u32 = 2048;
const SCENE_DECODE_SIDE: u32 = 512;
const SCENE_TOP_K: usize = 5;
/// Local inputs run this many at a time, so a pause or cancel is seen
/// between chunks instead of after the whole batch.
const LOCAL_CHUNK: usize = 16;
/// How often an in-flight cloud request looks for a pause or cancel.
const INTERRUPT_POLL: std::time::Duration = std::time::Duration::from_millis(250);

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
	data_dir: PathBuf,
	loaded: Mutex<HashMap<&'static str, Provider>>,
}

impl ProviderRegistry {
	pub fn new(data_dir: &Path) -> Self {
		Self {
			data_dir: data_dir.to_path_buf(),
			loaded: Mutex::new(HashMap::new()),
		}
	}

	pub fn model_dir(&self, spec: &ModelSpec) -> PathBuf {
		super::model_dir(&self.data_dir, spec)
	}

	pub async fn is_installed(&self, spec: &ModelSpec) -> bool {
		super::is_installed(&self.data_dir, spec).await
	}

	/// Drops a loaded model so its files can be deleted.
	pub async fn unload(&self, spec: &ModelSpec) {
		self.loaded.lock().await.remove(spec.id);
	}

	/// The first installed model of `category`, loading it if needed.
	///
	/// A build without `ai-local` has no ONNX Runtime, so it never loads a
	/// model, even one a full build left on disk.
	pub async fn local(&self, category: Category) -> Result<Option<Provider>, AiError> {
		#[cfg(feature = "ai-local")]
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
		#[cfg(not(feature = "ai-local"))]
		let _ = category;
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
	/// holds a `face_embedding` grant and a model is installed. `interrupted`
	/// is polled between chunks and while a cloud request is in flight, so a
	/// paused or cancelled job stops without waiting for the whole batch.
	pub async fn run(
		&self,
		provider: Provider,
		task: &str,
		items: Vec<Vec<u8>>,
		embedder: Option<Arc<dyn FaceEmbedder>>,
		interrupted: impl Fn() -> bool,
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
		let mut out = Vec::with_capacity(items.len());
		if let Provider::ImageCaption(captioner, _) = provider {
			for item in &items {
				let caption = captioner.caption(item);
				tokio::pin!(caption);
				let result = loop {
					tokio::select! {
						result = &mut caption => break result,
						_ = tokio::time::sleep(INTERRUPT_POLL) => {
							if interrupted() {
								return Err(AiError::Interrupted);
							}
						}
					}
				};
				out.push(result.and_then(to_json));
			}
			return Ok(out);
		}
		for chunk in items.chunks(LOCAL_CHUNK) {
			if interrupted() {
				return Err(AiError::Interrupted);
			}
			let (provider, chunk, embedder) = (provider.clone(), chunk.to_vec(), embedder.clone());
			out.extend(
				tokio::task::spawn_blocking(move || run_local(provider, chunk, embedder))
					.await
					.map_err(|e| AiError::Model(e.to_string()))?,
			);
		}
		Ok(out)
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
