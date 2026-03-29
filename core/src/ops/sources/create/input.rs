//! Source creation input

use serde::{Deserialize, Serialize};
use specta::Type;

/// Input for creating a new archive source
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateSourceInput {
	/// Display name for the source
	pub name: String,
	/// Adapter ID (e.g., "gmail", "obsidian", "chrome-bookmarks")
	pub adapter_id: String,
	/// Adapter-specific configuration
	pub config: serde_json::Value,
}
