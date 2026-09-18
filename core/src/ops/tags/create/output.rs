//! Output for creating a tag.

use crate::domain::Tag;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateTagOutput {
	pub tag: Tag,
	/// `false` when the path already named a tag, which the caller gets back
	/// instead of a duplicate.
	pub created: bool,
}
