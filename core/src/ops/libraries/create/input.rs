//! Input types for library creation operations

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Input for creating a new library
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LibraryCreateInput {
	/// Name of the library
	pub name: String,

	/// Optional path for the library (if not provided, will use default location)
	pub path: Option<PathBuf>,
}

impl LibraryCreateInput {
	/// Create a new library creation input
	pub fn new(name: String) -> Self {
		Self { name, path: None }
	}

	/// Create with a specific path
	pub fn with_path(mut self, path: PathBuf) -> Self {
		self.path = Some(path);
		self
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if self.name.trim().is_empty() {
			errors.push("Library name cannot be empty".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
