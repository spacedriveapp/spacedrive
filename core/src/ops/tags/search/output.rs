//! Output for searching tags.

use crate::domain::Tag;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchTagsOutput {
	pub tags: Vec<Tag>,
}
