use std::path::{Path, PathBuf};

use chrono::Utc;
use tracing::{debug, info};
use uuid::Uuid;

use super::{
	archive::MemoryArchive,
	metadata::MemoryMetadata,
	scope::MemoryScope,
	types::{Document, DocumentType, Fact, FactType, MemoryStatistics},
	vector_store::VectorStore,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryFileError {
	#[error("Archive error: {0}")]
	Archive(#[from] super::archive::ArchiveError),

	#[error("Vector store error: {0}")]
	VectorStore(#[from] super::vector_store::VectorStoreError),

	#[error("Serialization error: {0}")]
	Serialization(#[from] rmp_serde::encode::Error),

	#[error("Deserialization error: {0}")]
	Deserialization(#[from] rmp_serde::decode::Error),

	#[error("Document not found: {0}")]
	DocumentNotFound(i32),

	#[error("Fact not found: {0}")]
	FactNotFound(i32),
}

pub type Result<T> = std::result::Result<T, MemoryFileError>;

/// Memory file using custom archive format
/// Single .memory file containing all data
pub struct MemoryFile {
	path: PathBuf,
	archive: MemoryArchive,
	metadata: MemoryMetadata,
	documents: Vec<Document>,
	facts: Vec<Fact>,
	vector_store: VectorStore,
	next_doc_id: i32,
	next_fact_id: i32,
}

impl MemoryFile {
	/// Create new memory file (single file archive)
	pub async fn create(name: String, scope: MemoryScope, output_path: &Path) -> Result<Self> {
		info!("Creating memory archive at: {}", output_path.display());

		// Create archive
		let mut archive = MemoryArchive::create(output_path)?;

		// Initialize metadata
		let metadata = MemoryMetadata::new(name, scope);

		// Write initial files
		let metadata_bytes = rmp_serde::to_vec(&metadata)?;
		archive.add_file("metadata.msgpack", &metadata_bytes)?;

		let documents: Vec<Document> = Vec::new();
		let documents_bytes = rmp_serde::to_vec(&documents)?;
		archive.add_file("documents.msgpack", &documents_bytes)?;

		let facts: Vec<Fact> = Vec::new();
		let facts_bytes = rmp_serde::to_vec(&facts)?;
		archive.add_file("facts.msgpack", &facts_bytes)?;

		// Create in-memory vector store
		let vector_store = VectorStore::create_in_memory()?;

		info!("Memory archive created successfully");

		Ok(Self {
			path: output_path.to_path_buf(),
			archive,
			metadata,
			documents,
			facts,
			vector_store,
			next_doc_id: 1,
			next_fact_id: 1,
		})
	}

	/// Open existing memory file
	pub async fn open(path: PathBuf) -> Result<Self> {
		info!("Opening memory archive at: {}", path.display());

		let mut archive = MemoryArchive::open(&path)?;

		// Load metadata
		let metadata_bytes = archive.read_file("metadata.msgpack")?;
		let metadata: MemoryMetadata = rmp_serde::from_slice(&metadata_bytes)?;

		// Load documents
		let documents: Vec<Document> = if archive.contains("documents.msgpack") {
			let bytes = archive.read_file("documents.msgpack")?;
			rmp_serde::from_slice(&bytes)?
		} else {
			Vec::new()
		};

		// Load facts
		let facts: Vec<Fact> = if archive.contains("facts.msgpack") {
			let bytes = archive.read_file("facts.msgpack")?;
			rmp_serde::from_slice(&bytes)?
		} else {
			Vec::new()
		};

		// Load vector store
		let vector_store = if archive.contains("embeddings.msgpack") {
			let bytes = archive.read_file("embeddings.msgpack")?;
			VectorStore::from_bytes(&bytes)?
		} else {
			VectorStore::create_in_memory()?
		};

		// Compute next IDs
		let next_doc_id = documents
			.iter()
			.map(|d: &Document| d.id)
			.max()
			.unwrap_or(0)
			+ 1;
		let next_fact_id = facts.iter().map(|f: &Fact| f.id).max().unwrap_or(0) + 1;

		debug!("Loaded memory: {} docs, {} facts", documents.len(), facts.len());

		Ok(Self {
			path,
			archive,
			metadata,
			documents,
			facts,
			vector_store,
			next_doc_id,
			next_fact_id,
		})
	}

	/// Add document
	pub async fn add_document(
		&mut self,
		content_uuid: Option<Uuid>,
		title: String,
		summary: Option<String>,
		doc_type: DocumentType,
	) -> Result<i32> {
		let doc = Document {
			id: self.next_doc_id,
			content_uuid,
			file_path: None,
			title,
			summary,
			relevance_score: 1.0,
			added_at: Utc::now(),
			added_by: "user".to_string(),
			doc_type,
			metadata: None,
		};

		self.next_doc_id += 1;
		self.documents.push(doc.clone());

		self.persist_documents().await?;
		self.update_statistics().await?;

		debug!("Added document: {} (id: {})", doc.title, doc.id);

		Ok(doc.id)
	}

	/// Add fact
	pub async fn add_fact(
		&mut self,
		text: String,
		fact_type: FactType,
		confidence: f32,
		source_document_id: Option<i32>,
	) -> Result<i32> {
		let fact = Fact {
			id: self.next_fact_id,
			text,
			fact_type,
			confidence,
			source_document_id,
			created_at: Utc::now(),
			verified: false,
		};

		self.next_fact_id += 1;
		self.facts.push(fact.clone());

		self.persist_facts().await?;
		self.update_statistics().await?;

		debug!("Added fact: {} (id: {})", fact.text, fact.id);

		Ok(fact.id)
	}

	/// Add embedding
	pub async fn add_embedding(&mut self, doc_id: i32, vector: Vec<f32>) -> Result<()> {
		let (content_uuid, title, metadata_val) = {
			let doc = self
				.get_document(doc_id)
				.ok_or(MemoryFileError::DocumentNotFound(doc_id))?;
			(doc.content_uuid, doc.title.clone(), doc.metadata.clone())
		};

		self.vector_store
			.add_embedding(doc_id, content_uuid, title, vector, metadata_val)
			.await?;

		self.persist_vector_store().await?;
		self.update_statistics().await?;

		Ok(())
	}

	/// Search similar documents
	pub async fn search_similar(&self, query_vector: Vec<f32>, limit: usize) -> Result<Vec<i32>> {
		let results = self.vector_store.search(query_vector, limit).await?;
		Ok(results.into_iter().map(|r| r.id).collect())
	}

	/// Get documents
	pub fn get_documents(&self) -> &[Document] {
		&self.documents
	}

	/// Get document by ID
	pub fn get_document(&self, id: i32) -> Option<&Document> {
		self.documents.iter().find(|d| d.id == id)
	}

	/// Get facts
	pub fn get_facts(&self) -> &[Fact] {
		&self.facts
	}

	/// Get metadata
	pub fn metadata(&self) -> &MemoryMetadata {
		&self.metadata
	}

	/// Get path
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Get embedding count
	pub async fn embedding_count(&self) -> Result<usize> {
		self.vector_store.count().await.map_err(Into::into)
	}

	/// Get facts sorted by confidence
	pub fn get_facts_sorted(&self) -> Vec<&Fact> {
		let mut sorted = self.facts.iter().collect::<Vec<_>>();
		sorted.sort_by(|a, b| {
			match (a.verified, b.verified) {
				(true, false) => std::cmp::Ordering::Less,
				(false, true) => std::cmp::Ordering::Greater,
				_ => b
					.confidence
					.partial_cmp(&a.confidence)
					.unwrap_or(std::cmp::Ordering::Equal),
			}
		});
		sorted
	}

	/// Persist documents to archive
	async fn persist_documents(&mut self) -> Result<()> {
		let bytes = rmp_serde::to_vec(&self.documents)?;
		self.archive.update_file("documents.msgpack", &bytes)?;
		Ok(())
	}

	/// Persist facts to archive
	async fn persist_facts(&mut self) -> Result<()> {
		let bytes = rmp_serde::to_vec(&self.facts)?;
		self.archive.update_file("facts.msgpack", &bytes)?;
		Ok(())
	}

	/// Persist vector store to archive
	async fn persist_vector_store(&mut self) -> Result<()> {
		let bytes = self.vector_store.to_bytes()?;
		self.archive.update_file("embeddings.msgpack", &bytes)?;
		Ok(())
	}

	/// Persist metadata to archive
	async fn persist_metadata(&mut self) -> Result<()> {
		let bytes = rmp_serde::to_vec(&self.metadata)?;
		self.archive.update_file("metadata.msgpack", &bytes)?;
		Ok(())
	}

	/// Update statistics
	async fn update_statistics(&mut self) -> Result<()> {
		let embedding_count = self.vector_store.count().await?;
		let file_size = self.archive.size()?;

		self.metadata.statistics = MemoryStatistics {
			document_count: self.documents.len(),
			fact_count: self.facts.len(),
			conversation_message_count: 0,
			embedding_count,
			file_size_bytes: file_size,
		};

		self.persist_metadata().await?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::NamedTempFile;

	#[tokio::test]
	async fn test_create_single_file_memory() {
		let temp_file = NamedTempFile::new().unwrap();

		let memory = MemoryFile::create(
			"test".to_string(),
			MemoryScope::Standalone,
			temp_file.path(),
		)
		.await
		.unwrap();

		// Should be a single file
		assert!(memory.path().exists());
		assert!(memory.path().is_file());
	}

	#[tokio::test]
	async fn test_add_and_retrieve() {
		let temp_file = NamedTempFile::new().unwrap();

		let mut memory = MemoryFile::create(
			"test".to_string(),
			MemoryScope::Standalone,
			temp_file.path(),
		)
		.await
		.unwrap();

		// Add document
		let doc_id = memory
			.add_document(None, "Test Doc".to_string(), None, DocumentType::Note)
			.await
			.unwrap();

		assert_eq!(memory.get_documents().len(), 1);
		assert_eq!(memory.get_document(doc_id).unwrap().title, "Test Doc");

		// Add fact
		memory
			.add_fact("Test fact".to_string(), FactType::General, 1.0, Some(doc_id))
			.await
			.unwrap();

		assert_eq!(memory.get_facts().len(), 1);
	}

	#[tokio::test]
	async fn test_persistence() {
		let temp_file = NamedTempFile::new().unwrap();
		let path = temp_file.path().to_path_buf();

		{
			let mut memory = MemoryFile::create(
				"test".to_string(),
				MemoryScope::Standalone,
				&path,
			)
			.await
			.unwrap();

			memory
				.add_document(None, "Doc".to_string(), None, DocumentType::Note)
				.await
				.unwrap();

			memory
				.add_fact("Fact".to_string(), FactType::General, 1.0, None)
				.await
				.unwrap();
		}

		// Reopen
		let memory = MemoryFile::open(path).await.unwrap();

		assert_eq!(memory.get_documents().len(), 1);
		assert_eq!(memory.get_facts().len(), 1);
	}

	#[tokio::test]
	async fn test_embeddings_in_archive() {
		let temp_file = NamedTempFile::new().unwrap();

		let mut memory = MemoryFile::create(
			"test".to_string(),
			MemoryScope::Standalone,
			temp_file.path(),
		)
		.await
		.unwrap();

		let doc_id = memory
			.add_document(None, "Doc".to_string(), None, DocumentType::Code)
			.await
			.unwrap();

		// Add embedding
		let vector = vec![0.1, 0.2, 0.3, 0.4];
		memory.add_embedding(doc_id, vector.clone()).await.unwrap();

		// Search
		let results = memory.search_similar(vector, 10).await.unwrap();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0], doc_id);
	}
}
