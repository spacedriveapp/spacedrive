//! Input for creating a tag.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateTagInput {
	/// Full path, `Work/Clients/Acme`. A single segment is a root tag.
	pub path: String,
	pub color: Option<String>,
	pub icon: Option<String>,
}

impl CreateTagInput {
	pub fn validate(&self) -> Result<(), String> {
		if self.path.trim().is_empty() {
			return Err("path cannot be empty".to_string());
		}
		if self.path.len() > 255 {
			return Err("path cannot exceed 255 characters".to_string());
		}
		if let Some(color) = &self.color {
			if !color.starts_with('#') || color.len() != 7 {
				return Err("color must be in hex format (#RRGGBB)".to_string());
			}
		}
		Ok(())
	}
}
