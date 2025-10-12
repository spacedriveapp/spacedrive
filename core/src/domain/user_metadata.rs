//! User metadata - notes, favorites, and custom fields
//!
//! This is the key innovation: EVERY Entry has UserMetadata, even if empty.
//! This means any file can be organized immediately without content indexing.
//!
//! Note: Tags are managed through the semantic tagging system (TagApplication)
//! via the user_metadata_tag junction table, not stored directly in UserMetadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// User-applied metadata for any Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetadata {
	/// Unique identifier (matches Entry.metadata_id)
	pub id: Uuid,

	/// Free-form notes
	pub notes: Option<String>,

	/// Whether this entry is marked as favorite
	pub favorite: bool,

	/// Whether this entry should be hidden
	pub hidden: bool,

	/// Custom fields for future extensibility
	pub custom_fields: JsonValue,

	/// When this metadata was created
	pub created_at: DateTime<Utc>,

	/// When this metadata was last updated
	pub updated_at: DateTime<Utc>,
}

impl UserMetadata {
	/// Create new empty metadata
	pub fn new(id: Uuid) -> Self {
		let now = Utc::now();
		Self {
			id,
			notes: None,
			favorite: false,
			hidden: false,
			custom_fields: JsonValue::Object(serde_json::Map::new()),
			created_at: now,
			updated_at: now,
		}
	}

	/// Set notes
	pub fn set_notes(&mut self, notes: Option<String>) {
		self.notes = notes;
		self.updated_at = Utc::now();
	}

	/// Toggle favorite status
	pub fn toggle_favorite(&mut self) {
		self.favorite = !self.favorite;
		self.updated_at = Utc::now();
	}

	/// Set hidden status
	pub fn set_hidden(&mut self, hidden: bool) {
		self.hidden = hidden;
		self.updated_at = Utc::now();
	}

	/// Check if metadata has any user-applied data
	pub fn is_empty(&self) -> bool {
		self.notes.is_none()
			&& !self.favorite
			&& !self.hidden
			&& self.custom_fields == JsonValue::Object(serde_json::Map::new())
	}
}

impl Default for UserMetadata {
	fn default() -> Self {
		Self::new(Uuid::new_v4())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_empty_metadata() {
		let metadata = UserMetadata::new(Uuid::new_v4());
		assert!(metadata.is_empty());
		assert!(!metadata.favorite);
		assert!(!metadata.hidden);
	}
}
