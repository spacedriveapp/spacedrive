//! Tag domain model.
//!
//! A tag is a small, portable definition: a place in a hierarchy expressed as
//! a path, a color, an icon. Definitions and their applications live in each
//! source's store (`crates/store::tags`), so a drive or a delivered replica
//! arrives carrying named tags rather than foreign keys into a database that
//! stayed home. `docs/core/design/tags-and-assertions.md` carries the model.

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A tag as clients see it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct Tag {
	/// Stable through rename; what assertions reference.
	pub id: Uuid,
	/// Full ancestor chain, `Work/Clients/Acme`. Hierarchy is derived from
	/// it, so a tag travels whole.
	pub path: String,
	/// The leaf segment, for display.
	pub name: String,
	pub color: Option<String>,
	pub icon: Option<String>,
}

impl Tag {
	pub fn from_definition(definition: &sd_store::TagDefinition) -> Self {
		Self {
			id: definition.uuid,
			name: leaf_of(&definition.path),
			path: definition.path.clone(),
			color: definition.color.clone(),
			icon: definition.icon.clone(),
		}
	}

	pub fn from_applied(applied: &sd_store::AppliedTag) -> Self {
		Self {
			id: applied.tag_uuid,
			name: leaf_of(&applied.path),
			path: applied.path.clone(),
			color: applied.color.clone(),
			icon: applied.icon.clone(),
		}
	}
}

/// The display name of a tag path: its last segment.
pub fn leaf_of(path: &str) -> String {
	path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Error types for tag operations.
#[derive(Debug, thiserror::Error)]
pub enum TagError {
	#[error("Tag not found")]
	TagNotFound,

	#[error("Invalid tag path: {0}")]
	InvalidPath(String),

	#[error("Database error: {0}")]
	DatabaseError(String),
}
