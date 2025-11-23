use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{MemoryScope, MemoryStatistics};

/// Metadata for a memory file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
	/// Memory name
	pub name: String,

	/// Optional description
	pub description: Option<String>,

	/// What this memory is scoped to
	pub scope: MemoryScope,

	/// When memory was created
	pub created_at: DateTime<Utc>,

	/// Last modification time
	pub updated_at: DateTime<Utc>,

	/// Last time memory was loaded/used
	pub last_used_at: Option<DateTime<Utc>>,

	/// Format version
	pub version: u32,

	/// Embedding model used
	pub embedding_model: String,

	/// Approximate total tokens
	pub total_tokens: usize,

	/// Tags for categorization
	pub tags: Vec<String>,

	/// Statistics (cached)
	pub statistics: MemoryStatistics,
}

impl MemoryMetadata {
	pub fn new(name: String, scope: MemoryScope) -> Self {
		Self {
			name,
			description: None,
			scope,
			created_at: Utc::now(),
			updated_at: Utc::now(),
			last_used_at: None,
			version: 1,
			embedding_model: "all-MiniLM-L6-v2".to_string(),
			total_tokens: 0,
			tags: Vec::new(),
			statistics: MemoryStatistics::default(),
		}
	}

	/// Mark memory as used (updates last_used_at)
	pub fn touch(&mut self) {
		self.last_used_at = Some(Utc::now());
	}

	/// Update modification time
	pub fn mark_updated(&mut self) {
		self.updated_at = Utc::now();
	}
}
