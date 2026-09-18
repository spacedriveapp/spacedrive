//! Output for removing tags.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UnapplyTagsOutput {
	/// Targets whose stores now durably carry the removal.
	pub targets_untagged: u32,
	/// Targets on remote-owned sources: the removal delivers to the owner
	/// when it next answers.
	pub targets_pending: u32,
	pub warnings: Vec<String>,
}
