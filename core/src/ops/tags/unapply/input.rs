//! Input for unapply (remove) tags from entries

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// What to untag — uses entry UUIDs (matching the File.id exposed to frontend)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UnapplyTagsInput {
	/// Entry UUIDs (File.id) to remove tags from
	pub entry_ids: Vec<Uuid>,

	/// Tag UUIDs to remove
	pub tag_ids: Vec<Uuid>,
}

impl UnapplyTagsInput {
	pub fn validate(&self) -> Result<(), String> {
		if self.entry_ids.is_empty() {
			return Err("entry_ids cannot be empty".to_string());
		}
		if self.tag_ids.is_empty() {
			return Err("tag_ids cannot be empty".to_string());
		}
		if self.entry_ids.len() > 1000 {
			return Err("Cannot unapply tags from more than 1000 entries at once".to_string());
		}
		Ok(())
	}
}
