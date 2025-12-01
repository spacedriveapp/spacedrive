//! Location export operation output types

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

/// Statistics about what was exported
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExportStats {
	pub entries: u64,
	pub content_identities: u64,
	pub user_metadata: u64,
	pub tags: u64,
	pub media_data: u64,
}

/// Output from location export action
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationExportOutput {
	pub location_uuid: Uuid,
	pub location_name: Option<String>,
	pub export_path: PathBuf,
	pub file_size_bytes: u64,
	pub stats: ExportStats,
}

impl ActionOutputTrait for LocationExportOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		let name = self
			.location_name
			.as_deref()
			.unwrap_or("Unnamed location");
		format!(
			"Exported '{}' to {} ({} entries, {} bytes)",
			name,
			self.export_path.display(),
			self.stats.entries,
			self.file_size_bytes
		)
	}

	fn output_type(&self) -> &'static str {
		"location.export.completed"
	}
}
