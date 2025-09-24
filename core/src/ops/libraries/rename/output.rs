//! Library rename operation output

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryRenameOutput {
	pub library_id: Uuid,
	pub old_name: String,
	pub new_name: String,
}

impl ActionOutputTrait for LibraryRenameOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		format!("Renamed library '{}' to '{}'", self.old_name, self.new_name)
	}

	fn output_type(&self) -> &'static str {
		"library.rename.output"
	}
}
