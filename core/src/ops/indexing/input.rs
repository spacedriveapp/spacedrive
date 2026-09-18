//! # Indexing Input Types
//!
//! Defines IndexInput, the canonical request shape for all indexing operations regardless
//! of origin (CLI, API, UI). This type is deserialized from external requests, validated,
//! and converted into IndexerJobConfig for internal execution. Separating input from config
//! keeps the public API stable while internal job parameters evolve.

use super::job::IndexScope;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Canonical input for indexing requests from any interface (CLI, API, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct IndexInput {
	/// The library within which the operation runs
	pub library_id: uuid::Uuid,

	/// One or more filesystem paths to index
	pub paths: Vec<PathBuf>,

	/// Indexing scope (current directory only vs recursive)
	pub scope: IndexScope,

	/// Whether to include hidden files/directories
	pub include_hidden: bool,
}

impl IndexInput {
	/// Creates an input with defaults: recursive indexing, excluding hidden files.
	pub fn new<P: IntoIterator<Item = PathBuf>>(library_id: uuid::Uuid, paths: P) -> Self {
		Self {
			library_id,
			paths: paths.into_iter().collect(),
			scope: IndexScope::Recursive,
			include_hidden: false,
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

	pub fn with_include_hidden(mut self, include_hidden: bool) -> Self {
		self.include_hidden = include_hidden;
		self
	}

	/// Checks that at least one path is provided; other fields are structurally valid via types.
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
