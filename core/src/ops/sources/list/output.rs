//! Source listing output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Information about a source
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceInfo {
	/// Source ID
	pub id: Uuid,
	/// Display name
	pub name: String,
	/// Data type (e.g., "email", "bookmark", "note")
	pub data_type: String,
	/// Adapter ID
	pub adapter_id: String,
	/// Number of items
	pub item_count: i64,
	/// Last sync timestamp
	pub last_synced: Option<String>,
	/// Current status
	pub status: String,
}

impl SourceInfo {
	pub fn new(
		id: Uuid,
		name: String,
		data_type: String,
		adapter_id: String,
		item_count: i64,
		last_synced: Option<String>,
		status: String,
	) -> Self {
		Self {
			id,
			name,
			data_type,
			adapter_id,
			item_count,
			last_synced,
			status,
		}
	}
}
