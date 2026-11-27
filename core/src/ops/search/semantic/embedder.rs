//! The text embedder semantic search runs on.
//!
//! Documents and queries must be embedded by the same model, so both sides
//! resolve it here. Today that is a local model from the catalog, loaded
//! through the provider registry when the `ai-local` feature is built and
//! the model is downloaded. A build without ONNX Runtime, such as the Intel
//! Mac daemon, reports semantic search as unavailable and keeps word search.

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

/// The embedder this device uses, or why there is none.
pub async fn resolve(context: &CoreContext) -> Result<Arc<dyn Embedder>, String> {
	#[cfg(feature = "ai-local")]
	{
		use crate::ops::ai::{catalog, Category, Provider};

		let mut model = None;
		for spec in catalog::for_category(Category::TextEmbedding) {
			if context.ai.is_installed(spec).await {
				model = Some(spec.id);
				break;
			}
		}
		let Some(model) = model else {
			return Err(NO_MODEL.to_string());
		};
		match context.ai.local(Category::TextEmbedding).await {
			Ok(Some(Provider::TextEmbedding(inner))) => Ok(Arc::new(Local { model, inner })),
			Ok(_) => Err(NO_MODEL.to_string()),
			Err(error) => Err(format!("the text embedding model did not load: {error}")),
		}
	}
	#[cfg(not(feature = "ai-local"))]
	{
		let _ = context;
		Err(NO_LOCAL_INFERENCE.to_string())
	}
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

#[cfg(feature = "ai-local")]
struct Local {
	model: &'static str,
	inner: Arc<dyn crate::ops::ai::registry::TextEmbedder>,
}

#[cfg(feature = "ai-local")]
impl Embedder for Local {
	fn model(&self) -> &str {
		self.model
	}

	fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
		self.inner.embed_passages(texts).map_err(|e| e.to_string())
	}

	fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
		self.inner
			.embed(&[text])
			.map_err(|e| e.to_string())?
			.pop()
			.ok_or_else(|| "the embedder returned nothing".to_string())
	}
}
