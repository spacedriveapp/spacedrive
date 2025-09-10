//! Core input types for indexing operations

use super::job::{IndexMode, IndexPersistence, IndexScope};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Canonical input for indexing requests from any interface (CLI, API, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexInput {
	/// The library within which the operation runs
	pub library_id: uuid::Uuid,

	/// One or more filesystem paths to index
	pub paths: Vec<PathBuf>,

	/// Indexing scope (current directory only vs recursive)
	pub scope: IndexScope,

	/// Indexing mode (shallow/content/deep)
	pub mode: IndexMode,

	/// Whether to include hidden files/directories
	pub include_hidden: bool,

	/// Where results are stored (ephemeral vs persistent)
	pub persistence: IndexPersistence,
}

impl IndexInput {
	/// Create a new input with sane defaults
	pub fn new<P: IntoIterator<Item = PathBuf>>(library_id: uuid::Uuid, paths: P) -> Self {
		Self {
			library_id,
			paths: paths.into_iter().collect(),
			scope: IndexScope::Recursive,
			mode: IndexMode::Content,
			include_hidden: false,
			persistence: IndexPersistence::Ephemeral,
		}
	}

	/// Convenience for a single path
	pub fn single(library_id: uuid::Uuid, path: PathBuf) -> Self {
		Self::new(library_id, std::iter::once(path))
	}

	pub fn with_scope(mut self, scope: IndexScope) -> Self {
		self.scope = scope;
		self
	}

	pub fn with_mode(mut self, mode: IndexMode) -> Self {
		self.mode = mode;
		self
	}

	pub fn with_include_hidden(mut self, include_hidden: bool) -> Self {
		self.include_hidden = include_hidden;
		self
	}

	pub fn with_persistence(mut self, persistence: IndexPersistence) -> Self {
		self.persistence = persistence;
		self
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if self.paths.is_empty() {
			errors.push("At least one path must be specified".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
