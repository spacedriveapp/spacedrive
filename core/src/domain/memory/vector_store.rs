use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum VectorStoreError {
	#[error("Vector store error: {0}")]
	Store(String),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Serialization error: {0}")]
	Serialization(#[from] rmp_serde::encode::Error),

	#[error("Deserialization error: {0}")]
	Deserialization(#[from] rmp_serde::decode::Error),
}

pub type Result<T> = std::result::Result<T, VectorStoreError>;

/// Simple MessagePack-based vector store
/// TODO: Replace with LanceDB once dependency conflicts resolved
pub struct VectorStore {
	storage_path: std::path::PathBuf,
	embeddings: HashMap<i32, VectorDocument>,
}

/// Document with embedding for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDocument {
	/// Document ID (maps to documents table)
	pub id: i32,

	/// Content UUID (if from Spacedrive)
	pub content_uuid: Option<String>,

	/// Document title
	pub title: String,

	/// Embedding vector
	pub vector: Vec<f32>,

	/// Additional metadata
	pub metadata: Option<serde_json::Value>,
}

impl VectorStore {
	/// Create new vector store in memory directory (old directory format)
	pub async fn create(memory_path: &Path) -> Result<Self> {
		let storage_path = memory_path.join("embeddings.msgpack");
		info!("Creating vector store at: {}", storage_path.display());

		let store = Self {
			storage_path: storage_path.clone(),
			embeddings: HashMap::new(),
		};

		// Write empty embeddings file
		store.persist().await?;

		Ok(store)
	}

	/// Create in-memory vector store (for archive format)
	pub fn create_in_memory() -> Result<Self> {
		Ok(Self {
			storage_path: std::path::PathBuf::new(),
			embeddings: HashMap::new(),
		})
	}

	/// Load from bytes (for archive format)
	pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
		let embeddings = rmp_serde::from_slice(bytes)?;
		Ok(Self {
			storage_path: std::path::PathBuf::new(),
			embeddings,
		})
	}

	/// Serialize to bytes (for archive format)
	pub fn to_bytes(&self) -> Result<Vec<u8>> {
		let bytes = rmp_serde::to_vec(&self.embeddings)?;
		Ok(bytes)
	}

	/// Open existing vector store
	pub async fn open(memory_path: &Path) -> Result<Self> {
		let storage_path = memory_path.join("embeddings.msgpack");
		debug!("Opening vector store at: {}", storage_path.display());

		let embeddings = if storage_path.exists() {
			let bytes = fs::read(&storage_path).await?;
			rmp_serde::from_slice(&bytes)?
		} else {
			HashMap::new()
		};

		Ok(Self {
			storage_path,
			embeddings,
		})
	}

	/// Persist to disk (only for directory-based format)
	async fn persist(&self) -> Result<()> {
		// Skip if in-memory mode (empty path)
		if self.storage_path.as_os_str().is_empty() {
			return Ok(());
		}

		let bytes = rmp_serde::to_vec(&self.embeddings)?;
		fs::write(&self.storage_path, bytes).await?;
		Ok(())
	}

	/// Add embedding for a document
	pub async fn add_embedding(
		&mut self,
		doc_id: i32,
		content_uuid: Option<Uuid>,
		title: String,
		vector: Vec<f32>,
		metadata: Option<serde_json::Value>,
	) -> Result<()> {
		let doc = VectorDocument {
			id: doc_id,
			content_uuid: content_uuid.map(|u| u.to_string()),
			title,
			vector,
			metadata,
		};

		self.embeddings.insert(doc_id, doc);
		self.persist().await?;

		Ok(())
	}

	/// Search for similar documents (simple cosine similarity)
	pub async fn search(
		&self,
		query_vector: Vec<f32>,
		limit: usize,
	) -> Result<Vec<VectorDocument>> {
		let mut results: Vec<(VectorDocument, f32)> = self
			.embeddings
			.values()
			.map(|doc| {
				let similarity = cosine_similarity(&query_vector, &doc.vector);
				(doc.clone(), similarity)
			})
			.collect();

		// Sort by similarity (descending)
		results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

		Ok(results
			.into_iter()
			.take(limit)
			.map(|(doc, _)| doc)
			.collect())
	}

	/// Get embedding count
	pub async fn count(&self) -> Result<usize> {
		Ok(self.embeddings.len())
	}

	/// Remove embedding by document ID
	pub async fn remove_embedding(&mut self, doc_id: i32) -> Result<()> {
		self.embeddings.remove(&doc_id);
		self.persist().await?;
		Ok(())
	}
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
	if a.len() != b.len() {
		return 0.0;
	}

	let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
	let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
	let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

	if mag_a == 0.0 || mag_b == 0.0 {
		return 0.0;
	}

	dot / (mag_a * mag_b)
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;

	#[tokio::test]
	async fn test_create_vector_store() {
		let temp_dir = tempdir().unwrap();
		let memory_path = temp_dir.path().join("test.memory");
		std::fs::create_dir_all(&memory_path).unwrap();

		let _store = VectorStore::create(&memory_path).await.unwrap();

		assert!(memory_path.join("embeddings.msgpack").exists());
	}

	#[tokio::test]
	async fn test_add_and_search() {
		let temp_dir = tempdir().unwrap();
		let memory_path = temp_dir.path().join("test.memory");
		std::fs::create_dir_all(&memory_path).unwrap();

		let mut store = VectorStore::create(&memory_path).await.unwrap();

		// Add test embeddings
		let vector1 = vec![0.1, 0.2, 0.3, 0.4];
		let vector2 = vec![0.2, 0.3, 0.4, 0.5];

		store
			.add_embedding(1, None, "Doc 1".to_string(), vector1.clone(), None)
			.await
			.unwrap();

		store
			.add_embedding(2, None, "Doc 2".to_string(), vector2, None)
			.await
			.unwrap();

		// Search with query similar to vector1
		let results = store.search(vector1, 10).await.unwrap();

		assert_eq!(results.len(), 2);
		assert_eq!(results[0].id, 1); // Most similar should be first
		assert_eq!(results[0].title, "Doc 1");
	}

	#[tokio::test]
	async fn test_count() {
		let temp_dir = tempdir().unwrap();
		let memory_path = temp_dir.path().join("test.memory");
		std::fs::create_dir_all(&memory_path).unwrap();

		let mut store = VectorStore::create(&memory_path).await.unwrap();

		assert_eq!(store.count().await.unwrap(), 0);

		store
			.add_embedding(1, None, "Doc 1".to_string(), vec![0.1, 0.2, 0.3], None)
			.await
			.unwrap();

		assert_eq!(store.count().await.unwrap(), 1);
	}

	#[tokio::test]
	async fn test_remove_embedding() {
		let temp_dir = tempdir().unwrap();
		let memory_path = temp_dir.path().join("test.memory");
		std::fs::create_dir_all(&memory_path).unwrap();

		let mut store = VectorStore::create(&memory_path).await.unwrap();

		store
			.add_embedding(1, None, "Doc 1".to_string(), vec![0.1, 0.2, 0.3], None)
			.await
			.unwrap();

		assert_eq!(store.count().await.unwrap(), 1);

		store.remove_embedding(1).await.unwrap();

		assert_eq!(store.count().await.unwrap(), 0);
	}
}
