//! Input for delete tag action

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteTagInput {
	pub tag_id: Uuid,
}

impl DeleteTagInput {
	pub fn validate(&self) -> Result<(), String> {
		if self.tag_id.is_nil() {
			return Err("tag_id cannot be nil".to_string());
		}
		Ok(())
	}
}
