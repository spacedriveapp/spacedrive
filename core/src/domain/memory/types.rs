use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A document reference in a memory file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
	/// Internal ID within memory
	pub id: i32,

	/// Spacedrive content UUID (if file is in VDFS)
	pub content_uuid: Option<Uuid>,

	/// Physical path (for non-VDFS files or reference)
	pub file_path: Option<String>,

	/// Document title
	pub title: String,

	/// AI-generated or user-written summary
	pub summary: Option<String>,

	/// Relevance score (0.0-1.0)
	pub relevance_score: f32,

	/// When document was added to memory
	pub added_at: DateTime<Utc>,

	/// Who added it
	pub added_by: String,

	/// Document type classification
	pub doc_type: DocumentType,

	/// Additional metadata
	pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
	Code,
	Documentation,
	Reference,
	Note,
	Design,
	Test,
	Config,
	Other,
}

impl std::fmt::Display for DocumentType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = match self {
			Self::Code => "Code",
			Self::Documentation => "Documentation",
			Self::Reference => "Reference",
			Self::Note => "Note",
			Self::Design => "Design",
			Self::Test => "Test",
			Self::Config => "Config",
			Self::Other => "Other",
		};
		write!(f, "{}", s)
	}
}

/// A learned fact in a memory file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
	/// Internal ID within memory
	pub id: i32,

	/// The fact text
	pub text: String,

	/// Type of fact
	pub fact_type: FactType,

	/// Confidence score (0.0-1.0)
	pub confidence: f32,

	/// Source document ID (if extracted from document)
	pub source_document_id: Option<i32>,

	/// When fact was created
	pub created_at: DateTime<Utc>,

	/// Whether fact has been verified by user
	pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactType {
	/// Core principle or pattern
	Principle,

	/// Decision made during development
	Decision,

	/// Observed pattern or behavior
	Pattern,

	/// Known issue or limitation
	Issue,

	/// Implementation detail
	Detail,

	/// General knowledge
	General,
}

impl std::fmt::Display for FactType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = match self {
			Self::Principle => "Principle",
			Self::Decision => "Decision",
			Self::Pattern => "Pattern",
			Self::Issue => "Issue",
			Self::Detail => "Detail",
			Self::General => "General",
		};
		write!(f, "{}", s)
	}
}

/// Statistics about a memory file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStatistics {
	/// Number of documents
	pub document_count: usize,

	/// Number of facts
	pub fact_count: usize,

	/// Number of conversation messages (if history enabled)
	pub conversation_message_count: usize,

	/// Number of embeddings in vector store
	pub embedding_count: usize,

	/// Total size on disk (bytes)
	pub file_size_bytes: u64,
}

/// Conversation message (optional history)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
	pub id: i32,
	pub session_id: Uuid,
	pub role: MessageRole,
	pub content: String,
	pub tokens: Option<usize>,
	pub created_at: DateTime<Utc>,
	pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
	User,
	Assistant,
	System,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
	pub id: i32,
	pub action: String,
	pub actor: String,
	pub details: Option<serde_json::Value>,
	pub timestamp: DateTime<Utc>,
}
