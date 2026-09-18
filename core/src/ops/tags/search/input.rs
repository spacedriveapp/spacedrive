//! Input for searching tags.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchTagsInput {
	/// Case-insensitive substring over the full path. Empty lists every tag.
	pub query: String,
	pub limit: Option<u32>,
}
