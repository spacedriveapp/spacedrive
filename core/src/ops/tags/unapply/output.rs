//! Output for unapply tags action

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UnapplyTagsOutput {
	pub entries_affected: usize,
	pub tags_removed: usize,
	pub warnings: Vec<String>,
}
