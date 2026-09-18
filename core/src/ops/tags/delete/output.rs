//! Output for deleting a tag definition.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteTagOutput {
	/// Removal assertions written for records that carried the tag.
	pub applications_removed: u64,
	/// Stores the definition was removed from.
	pub sources_updated: u32,
}
