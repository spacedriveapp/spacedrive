//! Input for removing tags.

use crate::ops::tags::apply::input::TagTargets;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UnapplyTagsInput {
	pub targets: TagTargets,
	pub tag_ids: Vec<Uuid>,
}

impl UnapplyTagsInput {
	pub fn validate(&self) -> Result<(), String> {
		self.targets.validate()?;
		if self.tag_ids.is_empty() {
			return Err("tag_ids cannot be empty".to_string());
		}
		Ok(())
	}
}
