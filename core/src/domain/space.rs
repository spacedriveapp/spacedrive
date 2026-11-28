//! # Spaces
//!
//! A space is a folder that people and agents work in, plus its policy. Any
//! folder in a source can be a space, and a workspace space is one Spacedrive
//! created empty, usually for an agent. Creating a space on an existing folder
//! changes nothing on disk. `docs/core/design/spaces.md` has the model.
//!
//! The sidebar's saved arrangements were called spaces before this and are now
//! [`crate::domain::Layout`].

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// How a space came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SpaceKind {
	/// An existing folder.
	Folder,
	/// A folder Spacedrive created empty.
	Workspace,
}

impl SpaceKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Folder => "folder",
			Self::Workspace => "workspace",
		}
	}

	/// A kind written by a newer version reads as a folder, which is what
	/// every space is underneath.
	pub fn parse(kind: &str) -> Self {
		match kind {
			"workspace" => Self::Workspace,
			_ => Self::Folder,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Space {
	pub id: Uuid,
	pub name: String,
	pub kind: SpaceKind,
	/// The source whose store holds the space.
	pub source_id: Uuid,
	/// The folder, `/`-separated and relative to the source root. Empty is the
	/// whole source.
	pub root: String,
	/// Where the folder is on this device, when the source is located here.
	pub path: Option<PathBuf>,
	/// Whether file contents inside the space are hashed and read.
	pub content_indexing: bool,
	/// Whether every change inside the space keeps its bytes as a version.
	pub versioning: bool,
	/// Patterns an agent may not write inside the space, on top of the
	/// built-in list. `/`-anchored patterns start at the space root; others
	/// match at any depth.
	pub protected: Vec<String>,
	/// How many times authority over the space has passed between nodes. A
	/// write naming another epoch is refused.
	pub authority_epoch: u64,
	pub created_at: DateTime<Utc>,
}

impl Space {
	pub fn from_row(row: sd_store::SpaceRow, source_id: Uuid, source_root: Option<&Path>) -> Self {
		let path = source_root.map(|root| {
			if row.root.is_empty() {
				root.to_path_buf()
			} else {
				root.join(&row.root)
			}
		});
		Self {
			id: row.uuid,
			name: row.name,
			kind: SpaceKind::parse(&row.kind),
			source_id,
			root: row.root,
			path,
			content_indexing: row.content_indexing,
			versioning: row.versioning,
			protected: serde_json::from_str(&row.protected).unwrap_or_default(),
			authority_epoch: u64::try_from(row.authority_epoch).unwrap_or_default(),
			created_at: DateTime::parse_from_rfc3339(&row.created_at)
				.map(|at| at.with_timezone(&Utc))
				.unwrap_or_default(),
		}
	}
}
