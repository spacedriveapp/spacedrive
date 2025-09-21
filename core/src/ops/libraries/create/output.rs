//! Library create operation output types

use crate::infra::action::output::ActionOutputTrait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Output from library create action dispatch
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LibraryCreateOutput {
	pub library_id: Uuid,
	pub name: String,
	pub path: PathBuf,
}

impl LibraryCreateOutput {
	pub fn new(library_id: Uuid, name: String, path: PathBuf) -> Self {
		Self {
			library_id,
			name,
			path,
		}
	}
}

impl ActionOutputTrait for LibraryCreateOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		format!(
			"Created library '{}' with ID {} at {}",
			self.name,
			self.library_id,
			self.path.display()
		)
	}

	fn output_type(&self) -> &'static str {
		"library.create.completed"
	}
}
