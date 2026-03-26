//! Per-source vector store backed by LanceDB (stub implementation).
//!
//! This is a temporary stub until LanceDB is properly integrated.
//! For now, vector operations return empty results.

use std::path::Path;

use crate::embed::EMBEDDING_DIM;
use crate::error::Result;

/// Stub vector store (does nothing).
pub struct VectorStore;

impl VectorStore {
	/// Open or create the embeddings table (no-op for stub).
	pub async fn open_or_create(_lance_dir: &Path) -> Result<Self> {
		Ok(Self)
	}

	/// Store an embedding (no-op for stub).
	pub async fn store(&self, _id: &str, _content: &str, _embedding: &[f32]) -> Result<()> {
		Ok(())
	}

	/// Upsert an embedding (no-op for stub).
	pub async fn upsert(&self, _id: &str, _content: &str, _embedding: &[f32]) -> Result<()> {
		Ok(())
	}

	/// Delete embeddings (no-op for stub).
	pub async fn delete(&self, _id: &str) -> Result<()> {
		Ok(())
	}

	/// Vector similarity search (returns empty for stub).
	pub async fn search(&self, _query_embedding: &[f32], _limit: usize) -> Result<Vec<VectorHit>> {
		Ok(Vec::new())
	}

	/// Get the record count (returns 0 for stub).
	pub async fn count(&self) -> Result<usize> {
		Ok(0)
	}
}

/// A vector search hit.
#[derive(Debug, Clone)]
pub struct VectorHit {
	pub id: String,
	pub distance: f32,
}
