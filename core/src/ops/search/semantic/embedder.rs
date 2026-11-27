//! # Semantic search embedder
//!
//! The text embedder semantic search runs on.
//!
//! Documents and queries must be embedded by the same model, so both sides
//! resolve it here: a local model from the catalog, when the `ai-local`
//! feature is built and the model is downloaded. A build without ONNX
//! Runtime, such as the Intel Mac daemon, reports semantic search as
//! unavailable and keeps word search.
//!
//! Semantic search loads its own instance rather than the one `ai.infer`
//! shares with extensions, because it needs a different thread budget. Its
//! inference runs on [`threads`] threads, one by default. With one thread
//! ONNX Runtime computes on the calling thread, so the background pass's
//! low-priority thread sets the model's priority and an interactive query's
//! normal thread sets the query's.

use std::sync::Arc;

use crate::context::CoreContext;

/// Embeds chunks and queries for one model.
pub trait Embedder: Send + Sync {
	/// The catalog id of the model, recorded beside the vectors it makes.
	fn model(&self) -> &str;
	/// Vectors for document chunks. Blocking; call it off the async runtime.
	fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String>;
	/// The vector for what a person typed. Blocking.
	fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// Why semantic search cannot run on a build without local inference.
pub const NO_LOCAL_INFERENCE: &str =
	"this build has no local inference (ONNX Runtime is not available for this platform), and no cloud embedder is configured";

/// Why semantic search cannot run until a person downloads a model.
pub const NO_MODEL: &str =
	"no text embedding model is installed; download Multilingual E5 small from the Models page";

/// The environment variable that sets how many threads embedding uses.
pub const THREADS_VAR: &str = "SD_SEMANTIC_THREADS";

/// Threads the model runs on: `SD_SEMANTIC_THREADS` when set, from one
/// to the machine's cores, and one otherwise.
pub fn threads() -> usize {
	let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
	std::env::var(THREADS_VAR)
		.ok()
		.and_then(|value| value.trim().parse::<usize>().ok())
		.unwrap_or(1)
		.clamp(1, cores)
}

/// The embedder this device uses, or why there is none. The model is loaded
/// once and kept for the daemon's lifetime.
pub async fn resolve(context: &CoreContext) -> Result<Arc<dyn Embedder>, String> {
	#[cfg(feature = "ai-local")]
	{
		use crate::ops::ai::{catalog, Category};

		static LOADED: tokio::sync::Mutex<Option<Arc<Local>>> = tokio::sync::Mutex::const_new(None);
		let mut spec = None;
		for candidate in catalog::for_category(Category::TextEmbedding) {
			if context.ai.is_installed(candidate).await {
				spec = Some(candidate);
				break;
			}
		}
		let Some(spec) = spec else {
			return Err(NO_MODEL.to_string());
		};
		let mut loaded = LOADED.lock().await;
		if let Some(local) = loaded.as_ref().filter(|local| local.model == spec.id) {
			return Ok(local.clone());
		}
		let dir = context.ai.model_dir(spec);
		let local = load(spec.id, dir, threads()).await?;
		tracing::info!(
			model = spec.id,
			threads = threads(),
			"Loaded the semantic search model"
		);
		*loaded = Some(local.clone());
		Ok(local)
	}
	#[cfg(not(feature = "ai-local"))]
	{
		let _ = context;
		Err(NO_LOCAL_INFERENCE.to_string())
	}
}

/// Load the model in `dir` on `threads` threads. Loaded on the background
/// runtime, so a pool started for more than one thread inherits its
/// priority.
#[cfg(feature = "ai-local")]
pub async fn load(
	model: &'static str,
	dir: std::path::PathBuf,
	threads: usize,
) -> Result<Arc<Local>, String> {
	let task =
		crate::ops::indexing::content_identity::hashing_runtime().spawn_blocking(move || {
			crate::ops::ai::local::E5::with_threads(
				&dir.join("model.onnx"),
				&dir.join("tokenizer.json"),
				threads,
			)
		});
	let inner = task
		.await
		.map_err(|e| e.to_string())?
		.map_err(|e| format!("the text embedding model did not load: {e}"))?;
	Ok(Arc::new(Local {
		model,
		inner: Arc::new(inner),
	}))
}

/// Whether an embedder could be resolved, without loading a model. Used to
/// decide whether a pass is worth a job row.
pub async fn installed(context: &CoreContext) -> bool {
	#[cfg(feature = "ai-local")]
	{
		use crate::ops::ai::{catalog, Category};
		for spec in catalog::for_category(Category::TextEmbedding) {
			if context.ai.is_installed(spec).await {
				return true;
			}
		}
		false
	}
	#[cfg(not(feature = "ai-local"))]
	{
		let _ = context;
		false
	}
}

/// The catalog's E5 model on this device.
#[cfg(feature = "ai-local")]
pub struct Local {
	model: &'static str,
	inner: Arc<crate::ops::ai::local::E5>,
}

#[cfg(feature = "ai-local")]
impl Embedder for Local {
	fn model(&self) -> &str {
		self.model
	}

	fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
		use crate::ops::ai::registry::TextEmbedder;
		self.inner.embed_passages(texts).map_err(|e| e.to_string())
	}

	fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
		use crate::ops::ai::registry::TextEmbedder;
		self.inner
			.embed(&[text])
			.map_err(|e| e.to_string())?
			.pop()
			.ok_or_else(|| "the embedder returned nothing".to_string())
	}
}
