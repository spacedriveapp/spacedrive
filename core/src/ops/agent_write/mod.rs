//! # Agent writes
//!
//! `core::ops::agent_write` lets an agent holding a writable grant change
//! files inside the granted directory. Each write goes to disk on the device
//! that owns the source, and the source's watcher ingests it like any other
//! change, so the store never learns about an agent write by a second path.
//!
//! Spacedrive does not merge. A write that changes an existing path names
//! what the agent read, a store revision or a content hash or both, and the
//! write is refused as stale when the path no longer matches. The refusal
//! carries the path's current state so the agent can read again and retry.
//! A write that creates a path never replaces anything: when the name is
//! taken, the new file lands beside it as `name (agent conflict 1).ext` and
//! both are kept.
//!
//! New bytes are staged in a hidden sibling and renamed into place, so a
//! reader sees the old file or the new one and never a partial. Replaced
//! bytes, deleted items and moved items go to the trash or stay on disk, and
//! every change is journaled under the write's id, so `files.undo` with that
//! id reverses it. Instruction files and git internals are refused by the
//! grant's own check. `docs/core/design/agent-writes.md` has the full
//! model and the acceptance matrix.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(context: &sd_core::context::CoreContext) {
//! use sd_core::ops::agent_write::{write_file, Precondition, WriteFileInput};
//!
//! let input = WriteFileInput {
//! 	token: "sdg_...".to_string(),
//! 	path: "notes/today.md".into(),
//! 	content: "hello".to_string(),
//! 	encoding: None,
//! 	offset: None,
//! 	expect: Some(Precondition {
//! 		revision: None,
//! 		content_hash: Some("9f86d0...".to_string()),
//! 	}),
//! };
//! match write_file(context, input).await {
//! 	Ok(outcome) => tracing::info!(undo = ?outcome.undo, "written"),
//! 	Err(refusal) => tracing::warn!(%refusal, "refused"),
//! }
//! # }
//! ```

mod action;
mod conflict;
mod ops;
mod state;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::ops::grants::GrantError;

pub use action::{
	AgentDeleteAction, AgentMakeDirAction, AgentMoveAction, AgentStatQuery, AgentWriteFileAction,
};
pub use conflict::conflict_name;
pub use ops::{delete, make_dir, move_path, stat, write_file};

/// The largest payload one write carries, after decoding.
pub const MAX_WRITE_BYTES: usize = 64 * 1024 * 1024;

/// What the agent read before it asked to change a path.
///
/// At least one field is required for any write that changes an existing
/// path. Every field given must match the path as it is now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Precondition {
	/// The revision of the source's store, as `agent.stat` or an earlier
	/// write reported it. It counts every change the store has ingested
	/// anywhere in the source, so it is the strict form.
	#[serde(default)]
	pub revision: Option<String>,
	/// The BLAKE3 hash of the path's contents, hex. For a directory it is the
	/// hash of its listing. It is computed from the disk on every check.
	#[serde(default)]
	pub content_hash: Option<String>,
}

impl Precondition {
	fn is_empty(&self) -> bool {
		self.revision.is_none() && self.content_hash.is_none()
	}
}

/// How `content` is encoded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ContentEncoding {
	#[default]
	#[serde(rename = "utf8")]
	Utf8,
	#[serde(rename = "base64")]
	Base64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PathKind {
	File,
	Directory,
	Symlink,
	Other,
}

/// A path inside a grant as it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PathState {
	/// Relative to the granted directory, with `/` separators.
	pub path: String,
	/// `None` when nothing is at the path.
	pub kind: Option<PathKind>,
	pub size: Option<u64>,
	/// `None` when absent, or when the file is a cloud placeholder whose
	/// contents are not on this device.
	pub content_hash: Option<String>,
	/// `None` when the source has no store on this device yet.
	pub revision: Option<String>,
}

impl PathState {
	pub fn exists(&self) -> bool {
		self.kind.is_some()
	}
}

/// What a write did.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WriteOutcome {
	/// The path as the request named it, relative to the granted directory.
	pub requested: String,
	/// The result's state after the write. Its path differs from
	/// `requested` when a create found the name taken.
	pub state: PathState,
	/// Whether the name was taken and the result was given a conflict name.
	pub conflict: bool,
	/// Pass this to `files.undo` as `job` to reverse the write. `None` when
	/// the write changed nothing, or when its journal could not be written.
	pub undo: Option<Uuid>,
}

/// Why a write was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteError {
	#[error(transparent)]
	Grant(#[from] GrantError),
	#[error(
		"stale: {path} changed since it was read; it is now {}",
		describe(current)
	)]
	Stale {
		path: String,
		current: Box<PathState>,
	},
	#[error("{0} exists; name the revision or content hash you read to change it")]
	PreconditionRequired(String),
	#[error("not found: {0}")]
	NotFound(String),
	#[error("invalid request: {0}")]
	Invalid(String),
	#[error("{0} is a cloud placeholder whose contents are not on this device")]
	Placeholder(String),
	#[error("{0}")]
	Io(String),
}

impl WriteError {
	/// The path's current state, for a stale refusal.
	pub fn current(&self) -> Option<&PathState> {
		match self {
			Self::Stale { current, .. } => Some(current),
			_ => None,
		}
	}
}

fn describe(state: &PathState) -> String {
	let Some(kind) = state.kind else {
		return format!(
			"absent (revision {})",
			state.revision.as_deref().unwrap_or("none")
		);
	};
	format!(
		"{} at revision {} with content hash {}",
		match kind {
			PathKind::File => "a file",
			PathKind::Directory => "a directory",
			PathKind::Symlink => "a symlink",
			PathKind::Other => "a special file",
		},
		state.revision.as_deref().unwrap_or("none"),
		state.content_hash.as_deref().unwrap_or("none")
	)
}

/// Write a whole file, or a byte range of one.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WriteFileInput {
	/// The grant token.
	pub token: String,
	/// Relative to the granted directory.
	pub path: PathBuf,
	pub content: String,
	#[serde(default)]
	pub encoding: Option<ContentEncoding>,
	/// Write `content` starting at this byte of the existing file, keeping
	/// the bytes around it. Needs `expect`. The whole file when absent.
	#[serde(default)]
	pub offset: Option<u64>,
	/// What was read. Absent for a create, which never replaces anything.
	#[serde(default)]
	pub expect: Option<Precondition>,
}

/// Create a directory and any missing parents.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MakeDirInput {
	pub token: String,
	pub path: PathBuf,
}

/// Move or rename a path inside the grant.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MoveInput {
	pub token: String,
	pub from: PathBuf,
	pub to: PathBuf,
	/// What was read at `from`.
	pub expect: Precondition,
}

/// Move a path to the trash.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteInput {
	pub token: String,
	pub path: PathBuf,
	/// What was read at `path`.
	pub expect: Precondition,
}

/// Read a path's state, the values a precondition names.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StatInput {
	pub token: String,
	pub path: PathBuf,
}
