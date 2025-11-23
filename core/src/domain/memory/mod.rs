//! Memory file format - Modular RAG for AI agents
//!
//! Memory files (.memory) are portable knowledge packages that contain:
//! - Vector embeddings (Chroma vector store)
//! - Document references (files relevant to a task)
//! - Learned facts (extracted knowledge)
//! - Optional conversation history
//!
//! Format: Directory with MessagePack files + embedded Chroma
//! Storage: {name}.memory/ directory containing all components

pub mod archive;
pub mod metadata;
pub mod scope;
pub mod storage;
pub mod types;
pub mod vector_store;

pub use archive::MemoryArchive;
pub use metadata::MemoryMetadata;
pub use scope::MemoryScope;
pub use storage::MemoryFile;
pub use types::{
	AuditEntry, ConversationMessage, Document, DocumentType, Fact, FactType, MemoryStatistics,
	MessageRole,
};
pub use vector_store::{VectorDocument, VectorStore};
