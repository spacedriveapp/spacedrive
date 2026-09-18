//! Input for applying tags.

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// What to tag: the bytes, or one copy of them.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "ids")]
pub enum TagTargets {
	/// Tag the bytes, which reaches every copy of them on every drive. The
	/// preferred form, and what a caller should use whenever the file has
	/// been identified.
	Content(Vec<Uuid>),

	/// Tag one file, by the uuid the volume index gave it. For a file whose
	/// bytes have not been hashed yet, and for the case where someone means
	/// this copy rather than all of them.
	File(Vec<Uuid>),
}

impl TagTargets {
	pub fn len(&self) -> usize {
		match self {
			Self::Content(ids) | Self::File(ids) => ids.len(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// A target list that names nothing, or names a nil uuid, is a caller bug
	/// rather than an empty result.
	pub fn validate(&self) -> Result<(), String> {
		let (ids, what) = match self {
			Self::Content(ids) => (ids, "content"),
			Self::File(ids) => (ids, "file"),
		};

		if ids.is_empty() {
			return Err(format!("{what} UUIDs cannot be empty"));
		}
		if ids.iter().any(Uuid::is_nil) {
			return Err(format!("{what} UUIDs cannot contain nil values"));
		}
		if ids.len() > 1000 {
			return Err(format!("cannot target more than 1000 {what} items at once"));
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApplyTagsInput {
	pub targets: TagTargets,
	pub tag_ids: Vec<Uuid>,
}

impl ApplyTagsInput {
	pub fn validate(&self) -> Result<(), String> {
		self.targets.validate()?;
		if self.tag_ids.is_empty() {
			return Err("tag_ids cannot be empty".to_string());
		}
		if self.tag_ids.len() > 50 {
			return Err("cannot apply more than 50 tags at once".to_string());
		}
		Ok(())
	}
}
