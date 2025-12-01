//! Location import operation output types

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

/// Statistics about what was imported
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImportStats {
	pub entries_imported: u64,
	pub entries_skipped: u64,
	pub content_identities: u64,
	pub user_metadata: u64,
	pub tags: u64,
	pub media_data: u64,
}

/// Output from location import action
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationImportOutput {
	pub location_uuid: Uuid,
	pub location_name: Option<String>,
	pub import_path: PathBuf,
	pub stats: ImportStats,
}

impl ActionOutputTrait for LocationImportOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		let name = self
			.location_name
			.as_deref()
			.unwrap_or("Unnamed location");
		format!(
			"Imported '{}' from {} ({} entries, {} skipped)",
			name,
			self.import_path.display(),
			self.stats.entries_imported,
			self.stats.entries_skipped,
		)
	}

	fn output_type(&self) -> &'static str {
		"location.import.completed"
	}
}
