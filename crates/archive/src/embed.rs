//! Embedding model wrapper (stub implementation for now).
//!
//! This is a temporary stub until the heavy FastEmbed/LanceDB dependencies
//! are properly integrated. For now, embeddings return zero vectors.

use std::path::Path;

use crate::error::Result;

/// Number of dimensions (stub - will be 384 for all-MiniLM-L6-v2).
pub const EMBEDDING_DIM: usize = 384;

/// Stub embedding model that returns zero vectors.
pub struct EmbeddingModel;

impl EmbeddingModel {
	/// Create a new stub embedding model.
	pub fn new() -> Result<Self> {
		Ok(Self)
	}

	/// Create from cache dir (no-op for stub).
	pub fn with_cache_dir(_cache_dir: &Path) -> Result<Self> {
		Ok(Self)
	}

	/// Embed a single text string (returns zero vector).
	pub async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
		Ok(vec![0.0; EMBEDDING_DIM])
	}

	/// Embed a batch of text strings (returns zero vectors).
	pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
		Ok(texts
			.into_iter()
			.map(|_| vec![0.0; EMBEDDING_DIM])
			.collect())
	}

	/// Embed a single text string blocking (returns zero vector).
	pub fn embed_blocking(&self, _text: &str) -> Result<Vec<f32>> {
		Ok(vec![0.0; EMBEDDING_DIM])
	}
}
