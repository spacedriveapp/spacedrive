//! Output for applying tags.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApplyTagsOutput {
	/// Targets whose stores now durably carry the assertions.
	pub targets_tagged: u32,
	/// Targets on remote-owned sources: authored durably here, delivered to
	/// the owner when it next answers.
	pub targets_pending: u32,
	pub warnings: Vec<String>,
}
